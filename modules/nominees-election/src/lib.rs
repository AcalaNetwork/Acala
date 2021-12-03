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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::{Encode, MaxEncodedLen};
use frame_support::{
	log,
	pallet_prelude::*,
	traits::{Contains, Get, LockIdentifier},
	transactional, BoundedVec,
};
use frame_system::pallet_prelude::*;
use orml_traits::{BasicCurrency, BasicLockableCurrency};
use primitives::{Balance, EraIndex};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
	RuntimeDebug, SaturatedConversion,
};
use sp_std::{fmt::Debug, prelude::*};
use support::{NomineesProvider, OnNewEra};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be
/// unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct UnlockChunk {
	/// Amount of funds to be unlocked.
	value: Balance,
	/// Era number at which point it'll be unlocked.
	era: EraIndex,
}

/// The ledger of a (bonded) account.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct BondingLedger<T>
where
	T: Get<u32>,
{
	/// The total amount of the account's balance that we are currently
	/// accounting for. It's just `active` plus all the `unlocking`
	/// balances.
	pub total: Balance,
	/// The total amount of the account's balance that will be at stake in
	/// any forthcoming rounds.
	pub active: Balance,
	/// Any balance that is becoming free, which may eventually be
	/// transferred out of the account.
	pub unlocking: BoundedVec<UnlockChunk, T>,
}

impl<T> BondingLedger<T>
where
	T: Get<u32>,
{
	/// Remove entries from `unlocking` that are sufficiently old and reduce
	/// the total by the sum of their balances.
	fn consolidate_unlocked(&mut self, current_era: EraIndex) {
		let mut total = self.total;
		self.unlocking.retain(|chunk| {
			if chunk.era > current_era {
				true
			} else {
				total = total.saturating_sub(chunk.value);
				false
			}
		});

		self.total = total;
	}

	/// Re-bond funds that were scheduled for unlocking.
	fn rebond(mut self, value: Balance) -> Self {
		let mut unlocking_balance: Balance = Zero::zero();
		let mut inner_vec = self.unlocking.into_inner();
		while let Some(last) = inner_vec.last_mut() {
			if unlocking_balance + last.value <= value {
				unlocking_balance += last.value;
				self.active += last.value;
				inner_vec.pop();
			} else {
				let diff = value - unlocking_balance;

				unlocking_balance += diff;
				self.active += diff;
				last.value -= diff;
			}

			if unlocking_balance >= value {
				break;
			}
		}

		self.unlocking = inner_vec.try_into().expect("Only popped elements from inner_vec");
		self
	}
}

impl<T> Default for BondingLedger<T>
where
	T: Get<u32>,
{
	fn default() -> Self {
		Self {
			unlocking: Default::default(),
			total: Default::default(),
			active: Default::default(),
		}
	}
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config {
		type Event: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::Event>;
		type Currency: BasicLockableCurrency<Self::AccountId, Moment = Self::BlockNumber, Balance = Balance>;
		type NomineeId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
		#[pallet::constant]
		type PalletId: Get<LockIdentifier>;
		#[pallet::constant]
		type MinBondThreshold: Get<Balance>;
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;
		#[pallet::constant]
		type NominateesCount: Get<u32>;
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;
		type NomineeFilter: Contains<Self::NomineeId>;
		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		BelowMinBondThreshold,
		InvalidTargetsLength,
		MaxUnlockChunksExceeded,
		NoBonded,
		NoUnlockChunk,
		InvalidNominee,
		NominateesCountExceeded,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// rebond. \[who, amount\]
		Rebond(T::AccountId, Balance),
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
		StorageMap<_, Twox64Concat, T::AccountId, BondingLedger<T::MaxUnlockingChunks>, ValueQuery>;

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
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<T::BlockNumber> for Pallet<T, I> {}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {
		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			let free_balance = T::Currency::free_balance(&who);
			if let Some(extra) = free_balance.checked_sub(ledger.total) {
				let extra = extra.min(amount);
				let old_active = ledger.active;
				ledger.active += extra;
				ensure!(
					ledger.active >= T::MinBondThreshold::get(),
					Error::<T, I>::BelowMinBondThreshold
				);
				ledger.total += extra;
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);

			let amount = amount.min(ledger.active);

			if !amount.is_zero() {
				let old_active = ledger.active;
				ledger.active -= amount;

				ensure!(
					ledger.active.is_zero() || ledger.active >= T::MinBondThreshold::get(),
					Error::<T, I>::BelowMinBondThreshold,
				);

				// Note: in case there is no current era it is fine to bond one era more.
				let era = Self::current_era() + T::BondingDuration::get();
				ledger
					.unlocking
					.try_push(UnlockChunk { value: amount, era })
					.map_err(|_| Error::<T, I>::MaxUnlockChunksExceeded)?;
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::rebond(T::MaxUnlockingChunks::get()))]
		#[transactional]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who);
			ensure!(!ledger.unlocking.is_empty(), Error::<T, I>::NoUnlockChunk);
			let old_active = ledger.active;
			let old_ledger_unlocking = ledger.unlocking.len();
			let old_nominations = Self::nominations(&who);
			let ledger = ledger.rebond(amount);

			Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
			Self::update_ledger(&who, &ledger);
			Self::deposit_event(Event::Rebond(who, amount));
			let removed_len = old_ledger_unlocking - ledger.unlocking.len();
			Ok(Some(T::WeightInfo::rebond(removed_len as u32)).into())
		}

		#[pallet::weight(T::WeightInfo::withdraw_unbonded(T::MaxUnlockingChunks::get()))]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let mut ledger = Self::ledger(&who);
			let old_ledger_unlocking = ledger.unlocking.len();
			ledger.consolidate_unlocked(Self::current_era());

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				Self::remove_ledger(&who);
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move
				// on.
				Self::update_ledger(&who, &ledger);
			}
			let removed_len = old_ledger_unlocking - ledger.unlocking.len();
			Ok(Some(T::WeightInfo::withdraw_unbonded(removed_len as u32)).into())
		}

		#[pallet::weight(T::WeightInfo::nominate(targets.len() as u32))]
		#[transactional]
		pub fn nominate(origin: OriginFor<T>, targets: Vec<T::NomineeId>) -> DispatchResult {
			let who = ensure_signed(origin)?;

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
				.ok_or(Error::<T, I>::InvalidTargetsLength)?;

			let ledger = Self::ledger(&who);
			ensure!(!ledger.total.is_zero(), Error::<T, I>::NoBonded);

			for validator in bounded_targets.iter() {
				ensure!(T::NomineeFilter::contains(validator), Error::<T, I>::InvalidNominee);
			}

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, old_active, &bounded_targets);
			Nominations::<T, I>::insert(&who, &bounded_targets);
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::chill(T::NominateesCount::get()))]
		#[transactional]
		pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &[]);
			Nominations::<T, I>::remove(&who);
			Ok(Some(T::WeightInfo::chill(old_nominations.len() as u32)).into())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	fn update_ledger(who: &T::AccountId, ledger: &BondingLedger<T::MaxUnlockingChunks>) {
		let res = T::Currency::set_lock(T::PalletId::get(), who, ledger.total);
		if let Err(e) = res {
			log::warn!(
				target: "nominees-election",
				"set_lock: failed to lock {:?} for {:?}: {:?}. \
				This is unexpected but should be safe",
				ledger.total, who.clone(), e
			);
			debug_assert!(false);
		}

		Ledger::<T, I>::insert(who, ledger);
	}

	fn remove_ledger(who: &T::AccountId) {
		let res = T::Currency::remove_lock(T::PalletId::get(), who);
		if let Err(e) = res {
			log::warn!(
				target: "nominees-election",
				"remove_lock: failed to remove lock for {:?}: {:?}. \
				This is unexpected but should be safe",
				who.clone(), e
			);
			debug_assert!(false);
		}

		Ledger::<T, I>::remove(who);
		Nominations::<T, I>::remove(who);
	}

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
