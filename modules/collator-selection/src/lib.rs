// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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
//! [this issue](https://github.com/paritytech/statemint/issues/21#issuecomment-810481073).

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
		dispatch::DispatchResultWithPostInfo,
		inherent::Vec,
		pallet_prelude::*,
		storage::bounded_btree_set::BoundedBTreeSet,
		traits::{Currency, EnsureOrigin, NamedReservableCurrency, ValidatorRegistration, ValidatorSet},
		BoundedVec, PalletId,
	};
	use frame_support::{
		sp_runtime::traits::{AccountIdConversion, Zero},
		weights::DispatchClass,
	};
	use frame_system::pallet_prelude::*;
	use frame_system::Config as SystemConfig;
	use pallet_session::SessionManager;
	use primitives::ReserveIdentifier;
	use sp_staking::SessionIndex;
	use sp_std::{convert::TryInto, vec};

	pub const RESERVE_ID: ReserveIdentifier = ReserveIdentifier::CollatorSelection;

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
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The currency mechanism.
		type Currency: NamedReservableCurrency<Self::AccountId, ReserveIdentifier = ReserveIdentifier>;

		/// A type for retrieving the validators supposed to be online in a session.
		type ValidatorSet: ValidatorSet<Self::AccountId, ValidatorId = Self::AccountId>
			+ ValidatorRegistration<Self::AccountId>;

		/// Origin that can dictate updating parameters of this pallet.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

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

		/// The weight information of this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	/// The invulnerable, fixed collators.
	#[pallet::storage]
	#[pallet::getter(fn invulnerables)]
	pub type Invulnerables<T: Config> = StorageValue<_, BoundedVec<T::AccountId, T::MaxInvulnerables>, ValueQuery>;

	/// The (community, limited) collation candidates.
	#[pallet::storage]
	#[pallet::getter(fn candidates)]
	pub type Candidates<T: Config> = StorageValue<_, BoundedBTreeSet<T::AccountId, T::MaxCandidates>, ValueQuery>;

	/// Desired number of candidates.
	///
	/// This should ideally always be less than [`Config::MaxCandidates`] for weights to be correct.
	#[pallet::storage]
	#[pallet::getter(fn desired_candidates)]
	pub type DesiredCandidates<T> = StorageValue<_, u32, ValueQuery>;

	/// Fixed deposit bond for each candidate.
	#[pallet::storage]
	#[pallet::getter(fn candidacy_bond)]
	pub type CandidacyBond<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// Session points for each candidate.
	#[pallet::storage]
	#[pallet::getter(fn session_points)]
	pub type SessionPoints<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, u32, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub invulnerables: Vec<T::AccountId>,
		pub candidacy_bond: BalanceOf<T>,
		pub desired_candidates: u32,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				invulnerables: Default::default(),
				candidacy_bond: Default::default(),
				desired_candidates: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			let duplicate_invulnerables = self.invulnerables.iter().collect::<std::collections::BTreeSet<_>>();
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

			<DesiredCandidates<T>>::put(&self.desired_candidates);
			<CandidacyBond<T>>::put(&self.candidacy_bond);
			<Invulnerables<T>>::put(&bounded_invulnerables);
		}
	}

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		NewInvulnerables(Vec<T::AccountId>),
		NewDesiredCandidates(u32),
		NewCandidacyBond(BalanceOf<T>),
		CandidateAdded(T::AccountId, BalanceOf<T>),
		CandidateRemoved(T::AccountId),
	}

	// Errors inform users that something went wrong.
	#[pallet::error]
	pub enum Error<T> {
		MaxCandidatesExceeded,
		BelowCandidatesMin,
		Unknown,
		Permission,
		AlreadyCandidate,
		NotCandidate,
		RequireSessionKey,
		AlreadyInvulnerable,
		InvalidProof,
		MaxInvulnerablesExceeded,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::set_invulnerables(new.len() as u32))]
		pub fn set_invulnerables(origin: OriginFor<T>, new: Vec<T::AccountId>) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			let bounded_new: BoundedVec<T::AccountId, T::MaxInvulnerables> =
				new.try_into().map_err(|_| Error::<T>::MaxInvulnerablesExceeded)?;
			<Invulnerables<T>>::put(&bounded_new);
			Self::deposit_event(Event::NewInvulnerables(bounded_new.into_inner()));
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::set_desired_candidates())]
		pub fn set_desired_candidates(origin: OriginFor<T>, max: u32) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			if max > T::MaxCandidates::get() {
				Err(Error::<T>::MaxCandidatesExceeded)?;
			}
			<DesiredCandidates<T>>::put(&max);
			Self::deposit_event(Event::NewDesiredCandidates(max));
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::set_candidacy_bond())]
		pub fn set_candidacy_bond(origin: OriginFor<T>, bond: BalanceOf<T>) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			<CandidacyBond<T>>::put(&bond);
			Self::deposit_event(Event::NewCandidacyBond(bond));
			Ok(().into())
		}

		#[pallet::weight(T::WeightInfo::register_as_candidate(T::MaxCandidates::get()))]
		pub fn register_as_candidate(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let deposit = Self::candidacy_bond();
			let bounded_candidates_len = Self::do_register_candidate(&who, deposit)?;

			Self::deposit_event(Event::CandidateAdded(who, deposit));
			Ok(Some(T::WeightInfo::register_as_candidate(bounded_candidates_len as u32)).into())
		}

		#[pallet::weight(T::WeightInfo::register_candidate(T::MaxCandidates::get()))]
		pub fn register_candidate(origin: OriginFor<T>, new_candidate: T::AccountId) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;

			let bounded_candidates_len = Self::do_register_candidate(&new_candidate, Zero::zero())?;

			Self::deposit_event(Event::CandidateAdded(new_candidate, Zero::zero()));
			Ok(Some(T::WeightInfo::register_candidate(bounded_candidates_len as u32)).into())
		}

		#[pallet::weight(T::WeightInfo::leave_intent(T::MaxCandidates::get()))]
		pub fn leave_intent(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let current_count = Self::try_remove_candidate(&who)?;

			Ok(Some(T::WeightInfo::leave_intent(current_count as u32)).into())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Get a unique, inaccessible account id from the `PotId`.
		pub fn account_id() -> T::AccountId {
			T::PotId::get().into_account()
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
				T::Currency::unreserve_all_named(&RESERVE_ID, &who);
				Ok(candidates.len())
			})?;
			Self::deposit_event(Event::CandidateRemoved(who.clone()));
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
			let length = <Candidates<T>>::decode_len().unwrap_or_default();
			ensure!(
				(length as u32) < Self::desired_candidates(),
				Error::<T>::MaxCandidatesExceeded
			);
			ensure!(!Self::invulnerables().contains(&who), Error::<T>::AlreadyInvulnerable);
			ensure!(T::ValidatorSet::is_registered(&who), Error::<T>::RequireSessionKey);

			<Candidates<T>>::try_mutate(|candidates| -> Result<usize, DispatchError> {
				ensure!(!candidates.contains(who), Error::<T>::AlreadyCandidate);

				candidates
					.try_insert(who.clone())
					.map_err(|_| Error::<T>::MaxCandidatesExceeded)?;
				T::Currency::ensure_reserved_named(&RESERVE_ID, &who, deposit)?;
				Ok(candidates.len())
			})
		}
	}

	/// Keep track of number of authored blocks per authority, uncles are counted as well since
	/// they're a valid proof of being online.
	impl<T: Config + pallet_authorship::Config> pallet_authorship::EventHandler<T::AccountId, T::BlockNumber>
		for Pallet<T>
	{
		fn note_author(author: T::AccountId) {
			log::debug!(
				"note author {:?} authored a block at #{:?}",
				author,
				<frame_system::Pallet<T>>::block_number(),
			);
			if <SessionPoints<T>>::contains_key(&author) {
				<SessionPoints<T>>::mutate(author, |point| *point += 1);
			}

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::note_author(),
				DispatchClass::Mandatory,
			);
		}

		fn note_uncle(_author: T::AccountId, _age: T::BlockNumber) {}
	}

	/// Play the role of the session manager.
	impl<T: Config> SessionManager<T::AccountId> for Pallet<T> {
		fn new_session(index: SessionIndex) -> Option<Vec<T::AccountId>> {
			let candidates = Self::candidates().into_iter().collect::<Vec<_>>();
			let result = Self::assemble_collators(candidates);

			log::debug!(
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
				if validators.contains(&candidate) {
					collators.push(candidate);
					<SessionPoints<T>>::insert(&candidate, 0);
				}
			});

			log::debug!(
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
			let mut candidates_len = 0;
			let mut removed_len = 0;
			<SessionPoints<T>>::drain().for_each(|(who, point)| {
				if point == 0 {
					log::debug!(
						"end session {:?} at #{:?}, remove candidate: {:?}",
						index,
						<frame_system::Pallet<T>>::block_number(),
						who,
					);
					removed_len += 1;

					let outcome = Self::try_remove_candidate(&who);
					if let Err(why) = outcome {
						log::warn!("Failed to remove candidate {:?}", why);
						debug_assert!(false, "failed to remove candidate {:?}", why);
					}
				}
				candidates_len += 1;
			});

			frame_system::Pallet::<T>::register_extra_weight_unchecked(
				T::WeightInfo::end_session(candidates_len as u32, removed_len as u32),
				DispatchClass::Mandatory,
			);
		}
	}
}
