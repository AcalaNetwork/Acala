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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]

use frame_support::{
	pallet_prelude::*,
	traits::{Contains, Get, LockIdentifier},
	BoundedVec,
};
use frame_system::pallet_prelude::*;
use module_support::NomineesProvider;
use orml_traits::{BasicCurrency, BasicLockableCurrency, Handler};
use primitives::{
	bonding::{self, BondingController},
	Balance, EraIndex,
};
use sp_runtime::traits::{MaybeDisplay, MaybeSerializeDeserialize, Member, Zero};
use sp_std::{fmt::Debug, prelude::*};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The token as vote.
		type Currency: BasicLockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>, Balance = Balance>;

		/// Nominee ID
		type NomineeId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord;

		/// LockIdentifier for lock vote token.
		#[pallet::constant]
		type PalletId: Get<LockIdentifier>;

		/// The minimum amount of tokens that can be bonded.
		#[pallet::constant]
		type MinBond: Get<Balance>;

		/// The waiting eras when unbond token.
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;

		/// The maximum number of nominees when voted and picked up.
		#[pallet::constant]
		type MaxNominateesCount: Get<u32>;

		/// The maximum number of simultaneous unbonding chunks that can exist.
		#[pallet::constant]
		type MaxUnbondingChunks: Get<u32>;

		/// The valid nominee filter.
		type NomineeFilter: Contains<Self::NomineeId>;

		/// Origin represented Governance
		type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Callback when an account bonded.
		type OnBonded: Handler<(Self::AccountId, Balance)>;

		/// Callback when an account unbonded.
		type OnUnbonded: Handler<(Self::AccountId, Balance)>;

		/// Current era.
		type CurrentEra: Get<EraIndex>;

		type WeightInfo: WeightInfo;
	}

	pub type BondingLedgerOf<T, I> = bonding::BondingLedgerOf<Pallet<T, I>>;

	#[pallet::error]
	pub enum Error<T, I = ()> {
		BelowMinBondThreshold,
		InvalidTargetsLength,
		MaxUnlockChunksExceeded,
		InvalidNominee,
		NominateesCountExceeded,
		NotBonded,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		Bond {
			who: T::AccountId,
			amount: Balance,
		},
		Unbond {
			who: T::AccountId,
			amount: Balance,
		},
		Rebond {
			who: T::AccountId,
			amount: Balance,
		},
		WithdrawUnbonded {
			who: T::AccountId,
			amount: Balance,
		},
		Nominate {
			who: T::AccountId,
			targets: Vec<T::NomineeId>,
		},
		ResetReservedNominees {
			group_index: u16,
			reserved_nominees: Vec<T::NomineeId>,
		},
	}

	/// The nominations for nominators.
	///
	/// Nominations: map AccountId => Vec<NomineeId>
	#[pallet::storage]
	#[pallet::getter(fn nominations)]
	pub type Nominations<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<<T as Config<I>>::NomineeId, T::MaxNominateesCount>,
		ValueQuery,
	>;

	/// The nomination bonding ledger.
	///
	/// Ledger: map AccountId => BondingLedger
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, BondingLedgerOf<T, I>, OptionQuery>;

	/// The total voting value for nominees.
	///
	/// Votes: map NomineeId => Balance
	#[pallet::storage]
	#[pallet::getter(fn votes)]
	pub type Votes<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, <T as Config<I>>::NomineeId, Balance, ValueQuery>;

	/// Reserved nominees.
	///
	/// ReservedNominees: map u16 => Vec<NomineeId>
	#[pallet::storage]
	#[pallet::getter(fn reserved_nominees)]
	pub type ReservedNominees<T: Config<I>, I: 'static = ()> = StorageMap<
		_,
		Blake2_128Concat,
		u16,
		BoundedVec<<T as Config<I>>::NomineeId, T::MaxNominateesCount>,
		ValueQuery,
	>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::bond())]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::bond(&who, amount)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);

				T::OnBonded::handle(&(who.clone(), change.change))?;

				Self::deposit_event(Event::Bond {
					who,
					amount: change.change,
				});
			}
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::bond())]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let unbond_at = T::CurrentEra::get().saturating_add(T::BondingDuration::get());
			let change = <Self as BondingController>::unbond(&who, amount, unbond_at)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);

				T::OnUnbonded::handle(&(who.clone(), change.change))?;

				Self::deposit_event(Event::Unbond {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::rebond(T::MaxUnbondingChunks::get()))]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::rebond(&who, amount)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);

				T::OnBonded::handle(&(who.clone(), change.change))?;

				Self::deposit_event(Event::Rebond {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::withdraw_unbonded(T::MaxUnbondingChunks::get()))]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::withdraw_unbonded(&who, T::CurrentEra::get())?;

			if let Some(change) = change {
				Self::deposit_event(Event::WithdrawUnbonded {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::nominate(targets.len() as u32))]
		pub fn nominate(origin: OriginFor<T>, targets: Vec<T::NomineeId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ledger = Self::ledger(&who).ok_or(Error::<T, I>::NotBonded)?;

			let bounded_targets: BoundedVec<<T as Config<I>>::NomineeId, <T as Config<I>>::MaxNominateesCount> = {
				if targets.is_empty() {
					Err(Error::<T, I>::InvalidTargetsLength)
				} else {
					targets.try_into().map_err(|_| Error::<T, I>::InvalidTargetsLength)
				}
			}?;

			let bounded_targets = bounded_targets
				.try_mutate(|targets| {
					targets.sort();
					targets.dedup();
				})
				.expect("This only reduce size of the vector; qed");

			for validator in bounded_targets.iter() {
				ensure!(T::NomineeFilter::contains(validator), Error::<T, I>::InvalidNominee);
			}

			let old_nominations = Self::nominations(&who);
			let old_active = ledger.active();

			Self::update_votes(old_active, &old_nominations, old_active, &bounded_targets);
			Nominations::<T, I>::insert(&who, &bounded_targets);

			Self::deposit_event(Event::Nominate {
				who,
				targets: bounded_targets.to_vec(),
			});
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::chill(T::MaxNominateesCount::get()))]
		pub fn chill(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ledger = Self::ledger(&who).ok_or(Error::<T, I>::NotBonded)?;

			let old_nominations = Self::nominations(&who);
			let old_active = ledger.active();

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &[]);
			Nominations::<T, I>::remove(&who);

			Self::deposit_event(Event::Nominate { who, targets: vec![] });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::reset_reserved_nominees(updates.len() as u32))]
		pub fn reset_reserved_nominees(
			origin: OriginFor<T>,
			updates: Vec<(u16, BoundedVec<T::NomineeId, T::MaxNominateesCount>)>,
		) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			for (group_index, reserved_nominees) in updates {
				let mut reserved_nominees: Vec<T::NomineeId> = reserved_nominees.to_vec();
				reserved_nominees.sort();
				reserved_nominees.dedup();

				let reserved: BoundedVec<T::NomineeId, T::MaxNominateesCount> = reserved_nominees
					.clone()
					.try_into()
					.expect("the length has been checked in params; qed");
				ReservedNominees::<T, I>::insert(group_index, reserved);

				Self::deposit_event(Event::ResetReservedNominees {
					group_index,
					reserved_nominees,
				});
			}
			Ok(())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	fn update_votes(
		old_active: Balance,
		old_nominations: &[T::NomineeId],
		new_active: Balance,
		new_nominations: &[T::NomineeId],
	) {
		if !old_active.is_zero() && !old_nominations.is_empty() {
			for account in old_nominations {
				Votes::<T, I>::mutate(account, |balance| *balance = balance.saturating_sub(old_active));
			}
		}

		if !new_active.is_zero() && !new_nominations.is_empty() {
			for account in new_nominations {
				Votes::<T, I>::mutate(account, |balance| *balance = balance.saturating_add(new_active));
			}
		}
	}

	fn sort_voted_nominees() -> Vec<T::NomineeId> {
		let mut voters = Votes::<T, I>::iter()
			.filter(|(id, _)| T::NomineeFilter::contains(id))
			.collect::<Vec<(T::NomineeId, Balance)>>();

		voters.sort_by(|a, b| b.1.cmp(&a.1));

		voters
			.iter()
			.map(|(nomination, _)| nomination.clone())
			.collect::<Vec<T::NomineeId>>()
	}
}

impl<T: Config<I>, I: 'static> NomineesProvider<T::NomineeId> for Pallet<T, I> {
	fn nominees() -> Vec<T::NomineeId> {
		let mut sorted_voted_nominees = Self::sort_voted_nominees();
		sorted_voted_nominees.truncate(T::MaxNominateesCount::get() as usize);
		sorted_voted_nominees
	}

	fn nominees_in_groups(group_index_list: Vec<u16>) -> Vec<(u16, Vec<T::NomineeId>)> {
		let mut nominees_in_groups = group_index_list
			.into_iter()
			.map(|group_index| (group_index, ReservedNominees::<T, I>::get(group_index).to_vec()))
			.collect::<Vec<(u16, Vec<T::NomineeId>)>>();

		let max_nominatees_count = T::MaxNominateesCount::get() as usize;

		for nominee in Self::sort_voted_nominees() {
			if nominees_in_groups
				.iter()
				.all(|(_, nominees)| nominees.len() == max_nominatees_count)
			{
				break;
			}

			let mut distribute_index: Option<(usize, usize)> = None;

			// distribute nominee to the group that does not contain nominee and has the shortest length
			for (index, (_, nominees)) in nominees_in_groups.iter().enumerate() {
				if !nominees.contains(&nominee) && nominees.len() < max_nominatees_count {
					match distribute_index {
						Some((_, len)) => {
							if nominees.len() < len {
								distribute_index = Some((index, nominees.len()));
							}
						}
						None => {
							distribute_index = Some((index, nominees.len()));
						}
					}
				}
			}

			// insert nominee to groups
			if let Some((index, _)) = distribute_index {
				nominees_in_groups[index].1.push(nominee);
			}
		}

		nominees_in_groups
	}
}

impl<T: Config<I>, I: 'static> BondingController for Pallet<T, I> {
	type MinBond = T::MinBond;
	type MaxUnbondingChunks = T::MaxUnbondingChunks;
	type Moment = EraIndex;
	type AccountId = T::AccountId;

	type Ledger = Ledger<T, I>;

	fn available_balance(who: &Self::AccountId, ledger: &BondingLedgerOf<T, I>) -> Balance {
		let free_balance = T::Currency::free_balance(who);
		free_balance.saturating_sub(ledger.total())
	}

	fn apply_ledger(who: &Self::AccountId, ledger: &BondingLedgerOf<T, I>) -> DispatchResult {
		if ledger.is_empty() {
			let res = T::Currency::remove_lock(T::PalletId::get(), who);
			if let Err(e) = res {
				log::warn!(
					target: "nominees-election",
					"remove_lock: failed to remove lock for {:?}: {:?}. \
					This is unexpected but should be safe",
					&who, e
				);
				debug_assert!(false);
			}

			Nominations::<T, I>::remove(who);

			res
		} else {
			let res = T::Currency::set_lock(T::PalletId::get(), who, ledger.total());
			if let Err(e) = res {
				log::warn!(
					target: "nominees-election",
					"set_lock: failed to lock {:?} for {:?}: {:?}. \
					This is unexpected but should be safe",
					ledger.total(), &who, e
				);
				debug_assert!(false);
			}
			res
		}
	}

	fn convert_error(err: bonding::Error) -> DispatchError {
		match err {
			bonding::Error::BelowMinBondThreshold => Error::<T, I>::BelowMinBondThreshold.into(),
			bonding::Error::MaxUnlockChunksExceeded => Error::<T, I>::MaxUnlockChunksExceeded.into(),
			bonding::Error::NotBonded => Error::<T, I>::NotBonded.into(),
		}
	}
}
