// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Collator Selection pallet.
//!
//! A pallet to manage collators in a parachain.
//!
//! ## Overview
//!
//! The Collator Selection pallet manages the collators of a parachain. **Collation is _not_ a
//! secure activity** and this pallet does not implement any game-theoretic mechanisms to meet BFT
//! safety assumptions of the chosen set.
//!
//! ## Terminology
//!
//! - Collator: A parachain block producer.
//! - Bond: An amount of `Balance` _reserved_ for candidate registration.
//! - Invulnerable: An account guaranteed to be in the collator set.
//!
//! ## Implementation
//!
//! The final [`Collators`] are aggregated from two individual lists:
//!
//! 1. [`Invulnerables`]: a set of collators appointed by governance. These accounts will always be
//!    collators.
//! 2. [`Candidates`]: these are *candidates to the collation task* and may or may not be elected as
//!    a final collator.
//!
//! The current implementation resolves congestion of [`Candidates`] in a first-come-first-serve
//! manner.
//!
//! ### Rewards
//!
//! The Collator Selection pallet maintains an on-chain account (the "Pot"). In each block, the
//! collator who authored it receives:
//!
//! - Half the value of the Pot.
//! - Half the value of the transaction fees within the block. The other half of the transaction
//!   fees are deposited into the Pot.
//!
//! Note: Eventually the Pot distribution may be modified as discussed in
//! [this issue](https://github.com/paritytech/asset_hub_polkadot/issues/21#issuecomment-810481073).

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::into_iter_on_ref)]
#![allow(clippy::try_err)]
#![allow(clippy::let_and_return)]

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	pub use crate::weights::WeightInfo;
	use frame_support::{
		dispatch::DispatchClass,
		sp_runtime::{
			traits::{AccountIdConversion, CheckedSub, Zero},
			Permill,
		},
	};
	use frame_support::{
		pallet_prelude::*,
		storage::bounded_btree_set::BoundedBTreeSet,
		traits::{
			Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, NamedReservableCurrency, ValidatorRegistration,
			ValidatorSet,
		},
		BoundedVec, PalletId,
	};
	use frame_system::pallet_prelude::*;
	use frame_system::Config as SystemConfig;
	use pallet_session::SessionManager;
	use primitives::ReserveIdentifier;
	use sp_staking::SessionIndex;
	use sp_std::{ops::Div, prelude::*};

	pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::CollatorSelection;
	pub const POINT_PER_BLOCK: u32 = 10;
	pub const SESSION_DELAY: SessionIndex = 2;

	type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as SystemConfig>::AccountId>>::Balance;

	/// A convertor from collators id. Since this pallet does not have stash/controller, this is
	/// just identity.
	pub struct IdentityCollator;
	impl<T> sp_runtime::traits::Convert<T, Option<T>> for IdentityCollator {
		fn convert(t: T) -> Option<T> {
			Some(t)
		}
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The currency mechanism.
		type Currency: NamedReservableCurrency<Self::AccountId, ReserveIdentifier = ReserveIdentifier>;

		/// A type for retrieving the validators supposed to be online in a session.
		type ValidatorSet: ValidatorSet<Self::AccountId, ValidatorId = Self::AccountId>
			+ ValidatorRegistration<Self::AccountId>;

		/// Origin that can dictate updating parameters of this pallet.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Account Identifier from which the internal Pot is generated.
		#[pallet::constant]
		type PotId: Get<PalletId>;

		/// Minimum number of candidates.
		#[pallet::constant]
		type MinCandidates: Get<u32>;

		/// Maximum number of candidates that we should have. This is used for benchmarking and is
		/// not enforced.
		///
		/// This does not take into account the invulnerables.
		#[pallet::constant]
		type MaxCandidates: Get<u32>;

		/// Maximum number of invulnerables.
		#[pallet::constant]
		type MaxInvulnerables: Get<u32>;

		/// The Kicked candidate cannot register candidate or withdraw bond until
		/// `KickPenaltySessionLength` ends.
		#[pallet::constant]
		type KickPenaltySessionLength: Get<u32>;

		/// Will be kicked if block is not produced in threshold.
		#[pallet::constant]
		type CollatorKickThreshold: Get<Permill>;

		/// Minimum reward to be distributed to the collators.
		#[pallet::constant]
		type MinRewardDistributeAmount: Get<BalanceOf<Self>>;

		/// The weight information of this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// The invulnerable, fixed collators.
	///
	/// Invulnerables: Vec<AccountId>
	#[pallet::storage]
	#[pallet::getter(fn invulnerables)]
	pub type Invulnerables<T: Config> = StorageValue<_, BoundedVec<T::AccountId, T::MaxInvulnerables>, ValueQuery>;

	/// The (community, limited) collation candidates.
	///
	/// Candidates: BTreeSet<AccountId>
	#[pallet::storage]
	#[pallet::getter(fn candidates)]
	pub type Candidates<T: Config> = StorageValue<_, BoundedBTreeSet<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Desired number of candidates.
	///
	/// This should ideally always be less than [`Config::MaxCandidates`] for weights to be correct.
	/// DesiredCandidates: u32
	#[pallet::storage]
	#[pallet::getter(fn desired_candidates)]
	pub type DesiredCandidates<T> = StorageValue<_, u32, ValueQuery>;

	/// Fixed deposit bond for each candidate.
	///
	/// CandidacyBond: Balance
	#[pallet::storage]
	#[pallet::getter(fn candidacy_bond)]
	pub type CandidacyBond<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Session points for each candidate.
	///
	/// SessionPoints: map AccountId => u32
	#[pallet::storage]
	#[pallet::getter(fn session_points)]
	pub type SessionPoints<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u32, ValueQuery>;

	/// Mapping from the kicked candidate or the left candidate to session index.
	///
	/// NonCandidates: map AccountId => SessionIndex
	#[pallet::storage]
	#[pallet::getter(fn non_candidates)]
	pub type NonCandidates<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, SessionIndex, ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub invulnerables: Vec<T::AccountId>,
		pub candidacy_bond: BalanceOf<T>,
		pub desired_candidates: u32,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let duplicate_invulnerables = self
				.invulnerables
				.iter()
				.collect::<sp_std::collections::btree_set::BTreeSet<_>>();
			assert_eq!(
				duplicate_invulnerables.len(),
				self.invulnerables.len(),
				"duplicate invulnerables in genesis."
			);

			let bounded_invulnerables: BoundedVec<T::AccountId, T::MaxInvulnerables> = self
				.invulnerables
				.clone()
				.try_into()
				.expect("genesis invulnerables are more than T::MaxInvulnerables");
			assert!(
				T::MaxCandidates::get() >= self.desired_candidates,
				"genesis desired_candidates are more than T::MaxCandidates",
			);

			<DesiredCandidates<T>>::put(self.desired_candidates);
			<CandidacyBond<T>>::put(self.candidacy_bond);
			<Invulnerables<T>>::put(&bounded_invulnerables);
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Invulnurable was updated.
		NewInvulnerables { new_invulnerables: Vec<T::AccountId> },
		/// Desired candidates was updated.
		NewDesiredCandidates { new_desired_candidates: u32 },
		/// Candidacy bond was updated.
		NewCandidacyBond { new_candidacy_bond: BalanceOf<T> },
		/// A candidate was added.
		CandidateAdded { who: T::AccountId, bond: BalanceOf<T> },
		/// A candidate was removed.
		CandidateRemoved { who: T::AccountId },
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		MaxCandidatesExceeded,
		BelowCandidatesMin,
		StillLocked,
		Unknown,
		Permission,
		AlreadyCandidate,
		NotCandidate,
		NotNonCandidate,
		NothingToWithdraw,
		RequireSessionKey,
		AlreadyInvulnerable,
		InvalidProof,
		MaxInvulnerablesExceeded,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_invulnerables(new.len() as u32))]
		pub fn set_invulnerables(origin: OriginFor<T>, new: Vec<T::AccountId>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			let bounded_new: BoundedVec<T::AccountId, T::MaxInvulnerables> =
				new.try_into().map_err(|_| Error::<T>::MaxInvulnerablesExceeded)?;
			<Invulnerables<T>>::put(&bounded_new);
			Self::deposit_event(Event::NewInvulnerables {
				new_invulnerables: bounded_new.into_inner(),
			});
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_desired_candidates())]
		pub fn set_desired_candidates(origin: OriginFor<T>, #[pallet::compact] max: u32) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if max > T::MaxCandidates::get() {
				Err(Error::<T>::MaxCandidatesExceeded)?;
			}
			<DesiredCandidates<T>>::put(max);
			Self::deposit_event(Event::NewDesiredCandidates {
				new_desired_candidates: max,
			});
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_candidacy_bond())]
		pub fn set_candidacy_bond(origin: OriginFor<T>, #[pallet::compact] bond: BalanceOf<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			<CandidacyBond<T>>::put(bond);
			Self::deposit_event(Event::NewCandidacyBond {
				new_candidacy_bond: bond,
			});
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::register_as_candidate(T::MaxCandidates::get()))]
		pub fn register_as_candidate(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			<NonCandidates<T>>::try_mutate_exists(&who, |maybe_index| -> DispatchResult {
				if let Some(index) = maybe_index.take() {
					ensure!(T::ValidatorSet::session_index() >= index, Error::<T>::StillLocked);
				}
				Ok(())
			})?;

			let deposit = Self::candidacy_bond();
			let bounded_candidates_len = Self::do_register_candidate(&who, deposit)?;
			Self::deposit_event(Event::CandidateAdded { who, bond: deposit });

			Ok(Some(T::WeightInfo::register_as_candidate(bounded_candidates_len as u32)).into())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::register_candidate(T::MaxCandidates::get()))]
		pub fn register_candidate(origin: OriginFor<T>, new_candidate: T::AccountId) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;

			let bounded_candidates_len = Self::do_register_candidate(&new_candidate, Zero::zero())?;

			Self::deposit_event(Event::CandidateAdded {
				who: new_candidate,
				bond: Zero::zero(),
			});
			Ok(Some(T::WeightInfo::register_candidate(bounded_candidates_len as u32)).into())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::leave_intent(T::MaxCandidates::get()))]
		pub fn leave_intent(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let current_count = Self::try_remove_candidate(&who)?;
			<NonCandidates<T>>::insert(who, T::ValidatorSet::session_index().saturating_add(SESSION_DELAY));

			Ok(Some(T::WeightInfo::leave_intent(current_count as u32)).into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::withdraw_bond())]
		pub fn withdraw_bond(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<NonCandidates<T>>::try_mutate_exists(&who, |maybe_index| -> DispatchResult {
				if let Some(index) = maybe_index.take() {
					ensure!(T::ValidatorSet::session_index() >= index, Error::<T>::StillLocked);
					T::Currency::unreserve_all_named(&RESERVE_ID, &who);
					Ok(())
				} else {
					Err(Error::<T>::NothingToWithdraw.into())
				}
			})
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get a unique, inaccessible account id from the `PotId`.
		pub fn account_id() -> T::AccountId {
			T::PotId::get().into_account_truncating()
		}

		/// Removes a candidate if they exist and sends them back their deposit
		fn try_remove_candidate(who: &T::AccountId) -> Result<usize, DispatchError> {
			let current_count = <Candidates<T>>::try_mutate(|candidates| -> Result<usize, DispatchError> {
				// prevent collator count drop below minimal count
				ensure!(
					candidates.len() > T::MinCandidates::get() as usize,
					Error::<T>::BelowCandidatesMin
				);

				candidates.take(who).ok_or(Error::<T>::NotCandidate)?;
				Ok(candidates.len())
			})?;
			Self::deposit_event(Event::CandidateRemoved { who: who.clone() });
			Ok(current_count)
		}

		/// Assemble the current set of candidates and invulnerables into the next collator set.
		///
		/// This is done on the fly, as frequent as we are told to do so, as the session manager.
		pub fn assemble_collators(candidates: Vec<T::AccountId>) -> Vec<T::AccountId> {
			let mut collators = Self::invulnerables().into_inner();
			collators.extend(candidates.into_iter().collect::<Vec<_>>());
			collators
		}

		pub fn do_register_candidate(who: &T::AccountId, deposit: BalanceOf<T>) -> Result<usize, DispatchError> {
			// ensure we are below limit.
			let length = <Candidates<T>>::decode_non_dedup_len().unwrap_or_default();
			ensure!(
				(length as u32) < Self::desired_candidates(),
				Error::<T>::MaxCandidatesExceeded
			);
			ensure!(!Self::invulnerables().contains(who), Error::<T>::AlreadyInvulnerable);
			ensure!(T::ValidatorSet::is_registered(who), Error::<T>::RequireSessionKey);

			<Candidates<T>>::try_mutate(|candidates| -> Result<usize, DispatchError> {
				ensure!(!candidates.contains(who), Error::<T>::AlreadyCandidate);

				candidates
					.try_insert(who.clone())
					.map_err(|_| Error::<T>::MaxCandidatesExceeded)?;
				T::Currency::ensure_reserved_named(&RESERVE_ID, who, deposit)?;
				Ok(candidates.len())
			})
		}
	}

	/// Keep track of number of authored blocks per authority, uncles are counted as well since
	/// they're a valid proof of being online.
	impl<T: Config + pallet_authorship::Config> pallet_authorship::EventHandler<T::AccountId, BlockNumberFor<T>>
		for Pallet<T>
	{
		fn note_author(author: T::AccountId) {
			log::debug!(
				target: "collator-selection",
				"note author {:?} authored a block at #{:?}",
				author,
				<frame_system::Pallet<T>>::block_number(),
			);
			let pot = Self::account_id();
			// assumes an ED will be sent to pot.
			let reward = T::Currency::free_balance(&pot)
				.checked_sub(&T::Currency::minimum_balance())
				.unwrap_or_default()
				.div(2u32.into());

			if reward >= T::MinRewardDistributeAmount::get() {
				// `reward` is half of pot account minus ED, this should never fail.
				let _success = T::Currency::transfer(&pot, &author, reward, KeepAlive);
				debug_assert!(_success.is_ok());
			}

			if <SessionPoints<T>>::contains_key(&author) {
				<SessionPoints<T>>::mutate(author, |point| *point += POINT_PER_BLOCK);
			}

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::note_author(),
				DispatchClass::Mandatory,
			);
		}
	}

	/// Play the role of the session manager.
	impl<T: Config> SessionManager<T::AccountId> for Pallet<T> {
		fn new_session(index: SessionIndex) -> Option<Vec<T::AccountId>> {
			let candidates = Self::candidates().into_iter().collect::<Vec<_>>();
			let result = Self::assemble_collators(candidates);

			log::debug!(
				target: "collator-selection",
				"assembling new collators for new session {:?} at #{:?}, candidates: {:?}",
				index,
				<frame_system::Pallet<T>>::block_number(),
				result,
			);

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::new_session(),
				DispatchClass::Mandatory,
			);

			Some(result)
		}

		fn start_session(index: SessionIndex) {
			let validators = T::ValidatorSet::validators();
			let candidates = Self::candidates();
			let mut collators = vec![];

			candidates.iter().for_each(|candidate| {
				if validators.contains(candidate) {
					collators.push(candidate);
					<SessionPoints<T>>::insert(candidate, 0);
				}
			});

			log::debug!(
				target: "collator-selection",
				"start session {:?} at #{:?}, candidates: {:?}",
				index,
				<frame_system::Pallet<T>>::block_number(),
				collators
			);

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::start_session(candidates.len() as u32, collators.len() as u32),
				DispatchClass::Mandatory,
			);
		}

		fn end_session(index: SessionIndex) {
			let mut removed_len = 0;
			let session_points = <SessionPoints<T>>::drain().collect::<Vec<_>>();
			let candidates_len: u32 = session_points.len() as u32;

			let total_session_point: u32 = session_points.iter().fold(0, |mut sum, (_, point)| {
				sum += point;
				sum
			});
			let average_session_point: u32 = total_session_point.checked_div(candidates_len).unwrap_or_default();
			let required_point: u32 = T::CollatorKickThreshold::get().mul_floor(average_session_point);
			for (who, point) in session_points {
				// required_point maybe is zero
				if point <= required_point {
					log::debug!(
						target: "collator-selection",
						"end session {:?} at #{:?}, remove candidate: {:?}, point: {:?}, required_point: {:?}",
						index,
						<frame_system::Pallet<T>>::block_number(),
						who,
						point,
						required_point,
					);
					removed_len += 1;

					let outcome = Self::try_remove_candidate(&who);
					if let Err(why) = outcome {
						log::warn!(
							target: "collator-selection",
							"Failed to remove candidate {:?}", why);
						debug_assert!(false, "failed to remove candidate {:?}", why);
					} else {
						<NonCandidates<T>>::insert(
							who,
							T::ValidatorSet::session_index().saturating_add(T::KickPenaltySessionLength::get()),
						);
					}
				}
			}

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::end_session(candidates_len, removed_len as u32),
				DispatchClass::Mandatory,
			);
		}
	}
}
