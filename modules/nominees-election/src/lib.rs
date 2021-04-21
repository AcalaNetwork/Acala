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

use codec::Encode;
use frame_support::{log, pallet_prelude::*, traits::LockIdentifier, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::{BasicCurrency, BasicLockableCurrency, Contains};
use primitives::{Balance, EraIndex};
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member, Zero},
	RuntimeDebug, SaturatedConversion,
};
use sp_std::{fmt::Debug, prelude::*};
use support::{NomineesProvider, OnNewEra};

mod mock;
mod tests;

pub use module::*;

pub const NOMINEES_ELECTION_ID: LockIdentifier = *b"nomelect";

/// Just a Balance/BlockNumber tuple to encode when a chunk of funds will be
/// unlocked.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk {
	/// Amount of funds to be unlocked.
	value: Balance,
	/// Era number at which point it'll be unlocked.
	era: EraIndex,
}

/// The ledger of a (bonded) account.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct BondingLedger {
	/// The total amount of the account's balance that we are currently
	/// accounting for. It's just `active` plus all the `unlocking`
	/// balances.
	pub total: Balance,
	/// The total amount of the account's balance that will be at stake in
	/// any forthcoming rounds.
	pub active: Balance,
	/// Any balance that is becoming free, which may eventually be
	/// transferred out of the account.
	pub unlocking: Vec<UnlockChunk>,
}

impl BondingLedger {
	/// Remove entries from `unlocking` that are sufficiently old and reduce
	/// the total by the sum of their balances.
	fn consolidate_unlocked(self, current_era: EraIndex) -> Self {
		let mut total = self.total;
		let unlocking = self
			.unlocking
			.into_iter()
			.filter(|chunk| {
				if chunk.era > current_era {
					true
				} else {
					total = total.saturating_sub(chunk.value);
					false
				}
			})
			.collect();

		Self {
			total,
			active: self.active,
			unlocking,
		}
	}

	/// Re-bond funds that were scheduled for unlocking.
	fn rebond(mut self, value: Balance) -> Self {
		let mut unlocking_balance: Balance = Zero::zero();

		while let Some(last) = self.unlocking.last_mut() {
			if unlocking_balance + last.value <= value {
				unlocking_balance += last.value;
				self.active += last.value;
				self.unlocking.pop();
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

		self
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
		type MinBondThreshold: Get<Balance>;
		#[pallet::constant]
		type BondingDuration: Get<EraIndex>;
		#[pallet::constant]
		type NominateesCount: Get<u32>;
		#[pallet::constant]
		type MaxUnlockingChunks: Get<u32>;
		type RelaychainValidatorFilter: Contains<Self::NomineeId>;
	}

	#[pallet::error]
	pub enum Error<T, I = ()> {
		BelowMinBondThreshold,
		InvalidTargetsLength,
		TooManyChunks,
		NoBonded,
		NoUnlockChunk,
		InvalidRelaychainValidator,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// rebond. [who, amount]
		Rebond(T::AccountId, Balance),
	}

	/// The nominations for nominators.
	///
	/// Nominations: map AccountId => Vec<NomineeId>
	#[pallet::storage]
	#[pallet::getter(fn nominations)]
	pub type Nominations<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, Vec<<T as Config<I>>::NomineeId>, ValueQuery>;

	/// The nomination bonding ledger.
	///
	/// Ledger: map => AccountId, BondingLedger
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, T::AccountId, BondingLedger, ValueQuery>;

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
	pub type Nominees<T: Config<I>, I: 'static = ()> = StorageValue<_, Vec<<T as Config<I>>::NomineeId>, ValueQuery>;

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
		#[pallet::weight(10000)]
		#[transactional]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
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
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let mut ledger = Self::ledger(&who);
			ensure!(
				ledger.unlocking.len() < T::MaxUnlockingChunks::get().saturated_into(),
				Error::<T, I>::TooManyChunks,
			);

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
				ledger.unlocking.push(UnlockChunk { value: amount, era });
				let old_nominations = Self::nominations(&who);

				Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
				Self::update_ledger(&who, &ledger);
			}
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who);
			ensure!(!ledger.unlocking.is_empty(), Error::<T, I>::NoUnlockChunk);
			let old_active = ledger.active;
			let old_nominations = Self::nominations(&who);
			let ledger = ledger.rebond(amount);

			Self::update_votes(old_active, &old_nominations, ledger.active, &old_nominations);
			Self::update_ledger(&who, &ledger);
			Self::deposit_event(Event::Rebond(who, amount));
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			let ledger = Self::ledger(&who).consolidate_unlocked(Self::current_era());

			if ledger.unlocking.is_empty() && ledger.active.is_zero() {
				Self::remove_ledger(&who);
			} else {
				// This was the consequence of a partial unbond. just update the ledger and move
				// on.
				Self::update_ledger(&who, &ledger);
			}
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn nominate(origin: OriginFor<T>, targets: Vec<T::NomineeId>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				!targets.is_empty() && targets.len() <= T::NominateesCount::get().saturated_into(),
				Error::<T, I>::InvalidTargetsLength,
			);

			let ledger = Self::ledger(&who);
			ensure!(!ledger.total.is_zero(), Error::<T, I>::NoBonded);

			let mut targets = targets;
			targets.sort();
			targets.dedup();

			for validator in &targets {
				ensure!(
					T::RelaychainValidatorFilter::contains(&validator),
					Error::<T, I>::InvalidRelaychainValidator
				);
			}

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, old_active, &targets);
			Nominations::<T, I>::insert(&who, &targets);
			Ok(().into())
		}

		#[pallet::weight(10000)]
		#[transactional]
		pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let old_nominations = Self::nominations(&who);
			let old_active = Self::ledger(&who).active;

			Self::update_votes(old_active, &old_nominations, Zero::zero(), &[]);
			Nominations::<T, I>::remove(&who);
			Ok(().into())
		}
	}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	fn update_ledger(who: &T::AccountId, ledger: &BondingLedger) {
		let res = T::Currency::set_lock(NOMINEES_ELECTION_ID, who, ledger.total);
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
		let res = T::Currency::remove_lock(NOMINEES_ELECTION_ID, who);
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

		let new_nominees = voters
			.into_iter()
			.take(T::NominateesCount::get().saturated_into())
			.map(|(nominee, _)| nominee)
			.collect::<Vec<_>>();

		Nominees::<T, I>::put(new_nominees);
	}
}

impl<T: Config<I>, I: 'static> NomineesProvider<T::NomineeId> for Pallet<T, I> {
	fn nominees() -> Vec<T::NomineeId> {
		Nominees::<T, I>::get()
	}
}

impl<T: Config<I>, I: 'static> OnNewEra<EraIndex> for Pallet<T, I> {
	fn on_new_era(era: EraIndex) {
		CurrentEra::<T, I>::put(era);
		Self::rebalance();
	}
}
