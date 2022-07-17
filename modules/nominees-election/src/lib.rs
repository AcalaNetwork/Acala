// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

use frame_support::{
	log,
	pallet_prelude::*,
	traits::{Contains, Get, LockIdentifier},
	transactional, BoundedVec,
};
use frame_system::pallet_prelude::*;
use orml_traits::{BasicCurrency, BasicLockableCurrency};
use primitives::{
	bonding::{self, BondingController},
	Balance, EraIndex,
};
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
	SaturatedConversion,
};
use sp_std::{fmt::Debug, prelude::*};
use support::{NomineesProvider, OnNewEra};

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
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: BasicLockableCurrency<Self::AccountId, Moment = Self::BlockNumber, Balance = Balance>;
		type NomineeId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord;
		#[pallet::constant]
		type PalletId: Get<LockIdentifier>;
		#[pallet::constant]
		type MinBond: Get<Balance>;
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;
		#[pallet::constant]
		type NominateesCount: Get<u32>;
		#[pallet::constant]
		type MaxUnbondingChunks: Get<u32>;
		type NomineeFilter: Contains<Self::NomineeId>;
		/// Weight information for the extrinsics in this module.
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
		Rebond { who: T::AccountId, amount: Balance },
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
		BoundedVec<<T as Config<I>>::NomineeId, T::NominateesCount>,
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

	/// The elected nominees.
	///
	/// Nominees: Vec<NomineeId>
	#[pallet::storage]
	#[pallet::getter(fn nominees)]
	pub type Nominees<T: Config<I>, I: 'static = ()> =
		StorageValue<_, BoundedVec<<T as Config<I>>::NomineeId, T::NominateesCount>, ValueQuery>;

	/// Current era index.
	///
	/// CurrentEra: EraIndex
	#[pallet::storage]
	#[pallet::getter(fn current_era)]
	pub type CurrentEra<T: Config<I>, I: 'static = ()> = StorageValue<_, EraIndex, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<T::BlockNumber> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::bond(&who, amount)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);
			}
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let unbond_at = Self::current_era().saturating_add(T::BondingDuration::get());
			let change = <Self as BondingController>::unbond(&who, amount, unbond_at)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);
			}

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::rebond(T::MaxUnbondingChunks::get()))]
		#[transactional]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::rebond(&who, amount)?;

			if let Some(change) = change {
				let old_nominations = Self::nominations(&who);

				Self::update_votes(change.old, &old_nominations, change.new, &old_nominations);
				Self::deposit_event(Event::Rebond {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::withdraw_unbonded(T::MaxUnbondingChunks::get()))]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as BondingController>::withdraw_unbonded(&who, Self::current_era())?;

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::nominate(targets.len() as u32))]
		#[transactional]
		pub fn nominate(origin: OriginFor<T>, targets: Vec<T::NomineeId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ledger = Self::ledger(&who).ok_or(Error::<T, I>::NotBonded)?;

			let bounded_targets: BoundedVec<<T as Config<I>>::NomineeId, <T as Config<I>>::NominateesCount> = {
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
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::chill(T::NominateesCount::get()))]
		#[transactional]
		pub fn chill(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let ledger = Self::ledger(&who).ok_or(Error::<T, I>::NotBonded)?;

			let old_nominations = Self::nominations(&who);
			let old_active = ledger.active();

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &[]);
			Nominations::<T, I>::remove(&who);

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

	fn rebalance() {
		let mut voters = Votes::<T, I>::iter().collect::<Vec<(T::NomineeId, Balance)>>();

		voters.sort_by(|a, b| b.1.cmp(&a.1));

		let new_nominees: BoundedVec<<T as Config<I>>::NomineeId, <T as Config<I>>::NominateesCount> = voters
			.into_iter()
			.take(T::NominateesCount::get().saturated_into())
			.map(|(nominee, _)| nominee)
			.collect::<Vec<_>>()
			.try_into()
			.expect("Only took from voters");

		Nominees::<T, I>::put(new_nominees);
	}
}

impl<T: Config<I>, I: 'static> NomineesProvider<T::NomineeId> for Pallet<T, I> {
	fn nominees() -> Vec<T::NomineeId> {
		Nominees::<T, I>::get().into_inner()
	}
}

impl<T: Config<I>, I: 'static> OnNewEra<EraIndex> for Pallet<T, I> {
	fn on_new_era(era: EraIndex) {
		CurrentEra::<T, I>::put(era);
		Self::rebalance();
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
