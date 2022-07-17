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

//! # Earning Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, OnUnbalanced, WithdrawReasons},
	transactional,
};
use frame_system::pallet_prelude::*;
use orml_traits::Happened;
use primitives::{
	bonding::{self, BondingController},
	Balance,
};
use sp_runtime::{traits::Saturating, Permill};

pub use module::*;

mod mock;
mod tests;
pub mod weights;

pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: LockableCurrency<Self::AccountId, Balance = Balance>;

		type OnBonded: Happened<(Self::AccountId, Balance)>;
		type OnUnbonded: Happened<(Self::AccountId, Balance)>;
		type OnUnstakeFee: OnUnbalanced<NegativeImbalanceOf<Self>>;

		#[pallet::constant]
		type MinBond: Get<Balance>;
		#[pallet::constant]
		type UnbondingPeriod: Get<Self::BlockNumber>;
		#[pallet::constant]
		type InstantUnstakeFee: Get<Permill>;
		#[pallet::constant]
		type MaxUnbondingChunks: Get<u32>;
		#[pallet::constant]
		type LockIdentifier: Get<LockIdentifier>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	pub type BondingLedgerOf<T> = bonding::BondingLedgerOf<Pallet<T>>;
	type NegativeImbalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

	#[pallet::error]
	pub enum Error<T> {
		BelowMinBondThreshold,
		MaxUnlockChunksExceeded,
		NotBonded,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Bonded {
			who: T::AccountId,
			amount: Balance,
		},
		Unbonded {
			who: T::AccountId,
			amount: Balance,
		},
		InstantUnbonded {
			who: T::AccountId,
			amount: Balance,
			fee: Balance,
		},
		Rebonded {
			who: T::AccountId,
			amount: Balance,
		},
		Withdrawn {
			who: T::AccountId,
			amount: Balance,
		},
	}

	/// The earning bonding ledger.
	///
	/// Ledger: map AccountId => Option<BondingLedger>
	#[pallet::storage]
	#[pallet::getter(fn ledger)]
	pub type Ledger<T: Config> = StorageMap<_, Twox64Concat, T::AccountId, BondingLedgerOf<T>, OptionQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Bond tokens by locking them up to `amount`.
		/// If user available balances is less than amount, then all the remaining balances will be
		/// locked.
		#[pallet::weight(T::WeightInfo::bond())]
		#[transactional]
		pub fn bond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::bond(&who, amount)?;

			if let Some(change) = change {
				T::OnBonded::happened(&(who.clone(), change.change));
				Self::deposit_event(Event::Bonded {
					who,
					amount: change.change,
				});
			}
			Ok(())
		}

		/// Start unbonding tokens up to `amount`.
		/// If bonded amount is less than `amount`, then all the remaining bonded tokens will start
		/// unbonding. Token will finish unbonding after `UnbondingPeriod` blocks.
		#[pallet::weight(T::WeightInfo::unbond())]
		#[transactional]
		pub fn unbond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let unbond_at = frame_system::Pallet::<T>::block_number().saturating_add(T::UnbondingPeriod::get());
			let change = <Self as BondingController>::unbond(&who, amount, unbond_at)?;

			if let Some(change) = change {
				T::OnUnbonded::happened(&(who.clone(), change.change));
				Self::deposit_event(Event::Unbonded {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		/// Unbond up to `amount` tokens instantly by paying a `InstantUnstakeFee` fee.
		/// If bonded amount is less than `amount`, then all the remaining bonded tokens will be
		/// unbonded. This will not unbond tokens during unbonding period.
		#[pallet::weight(T::WeightInfo::unbond_instant())]
		#[transactional]
		pub fn unbond_instant(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::unbond_instant(&who, amount)?;

			if let Some(change) = change {
				let amount = change.change;
				let fee = T::InstantUnstakeFee::get().mul_ceil(amount);
				let final_amount = amount.saturating_sub(fee);

				let unbalance =
					T::Currency::withdraw(&who, fee, WithdrawReasons::TRANSFER, ExistenceRequirement::KeepAlive)?;
				T::OnUnstakeFee::on_unbalanced(unbalance);

				T::OnUnbonded::happened(&(who.clone(), final_amount));
				Self::deposit_event(Event::InstantUnbonded {
					who,
					amount: final_amount,
					fee,
				});
			}

			Ok(())
		}

		/// Rebond up to `amount` tokens from unbonding period.
		/// If unbonded amount is less than `amount`, then all the remaining unbonded tokens will be
		/// rebonded.
		#[pallet::weight(T::WeightInfo::rebond())]
		#[transactional]
		pub fn rebond(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change = <Self as BondingController>::rebond(&who, amount)?;

			if let Some(change) = change {
				T::OnBonded::happened(&(who.clone(), change.change));
				Self::deposit_event(Event::Rebonded {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}

		/// Withdraw all unbonded tokens.
		#[pallet::weight(T::WeightInfo::withdraw_unbonded())]
		#[transactional]
		pub fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let change =
				<Self as BondingController>::withdraw_unbonded(&who, frame_system::Pallet::<T>::block_number())?;

			if let Some(change) = change {
				Self::deposit_event(Event::Withdrawn {
					who,
					amount: change.change,
				});
			}

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}

impl<T: Config> BondingController for Pallet<T> {
	type MinBond = T::MinBond;
	type MaxUnbondingChunks = T::MaxUnbondingChunks;
	type Moment = T::BlockNumber;
	type AccountId = T::AccountId;

	type Ledger = Ledger<T>;

	fn available_balance(who: &Self::AccountId, ledger: &BondingLedgerOf<T>) -> Balance {
		let free_balance = T::Currency::free_balance(who);
		free_balance.saturating_sub(ledger.total())
	}

	fn apply_ledger(who: &Self::AccountId, ledger: &BondingLedgerOf<T>) -> DispatchResult {
		if ledger.is_empty() {
			T::Currency::remove_lock(T::LockIdentifier::get(), who);
		} else {
			T::Currency::set_lock(T::LockIdentifier::get(), who, ledger.total(), WithdrawReasons::all());
		}
		Ok(())
	}

	fn convert_error(err: bonding::Error) -> DispatchError {
		match err {
			bonding::Error::BelowMinBondThreshold => Error::<T>::BelowMinBondThreshold.into(),
			bonding::Error::MaxUnlockChunksExceeded => Error::<T>::MaxUnlockChunksExceeded.into(),
			bonding::Error::NotBonded => Error::<T>::NotBonded.into(),
		}
	}
}
