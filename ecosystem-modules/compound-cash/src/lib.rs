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

//! # Compound Cash module
//!
//! This module provide support functions that handles business logic related to Compound Cash
//! tokens.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::UnixTime};
use module_support::CompoundCashTrait;
use primitives::{Balance, CashYieldIndex, CurrencyId, Moment, TokenSymbol};

mod mock;
mod tests;

pub const CASH_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::CASH);

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Time used for find which yield rate would apply.
		type UnixTime: UnixTime;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The yield has a timestamp older than the current value, so it will never be effective
		YieldIsOlderThanCurrent,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// Set the future yield for the Cash asset.
		FutureYieldSet {
			yield_amount: Balance,
			index: CashYieldIndex,
			timestamp: Moment,
		},
	}

	/// Stores a history of yields that have already been consumed.
	#[pallet::storage]
	#[pallet::getter(fn past_yield)]
	pub type PastYield<T: Config> = StorageMap<_, Blake2_128Concat, CashYieldIndex, (Balance, Moment), ValueQuery>;

	/// Stores a list of future-yields.
	#[pallet::storage]
	#[pallet::getter(fn future_yield)]
	pub type FutureYield<T: Config> = StorageMap<_, Blake2_128Concat, CashYieldIndex, (Balance, Moment), ValueQuery>;

	/// Stores the current yield used for CASH interest calculation.
	#[pallet::storage]
	#[pallet::getter(fn current_yield)]
	pub type CurrentYield<T: Config> = StorageValue<_, (CashYieldIndex, Balance, Moment), ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			// Use timestamp to check if the current Yield rate needs to be updated
			// To be completed once the spec is confirmed.
			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	pub fn set_future_yield(
		next_cash_yield: Balance,
		yield_index: CashYieldIndex,
		timestamp_effective: Moment,
	) -> DispatchResult {
		ensure!(
			timestamp_effective >= Self::current_yield().2,
			Error::<T>::YieldIsOlderThanCurrent
		);

		FutureYield::<T>::insert(yield_index, (next_cash_yield, timestamp_effective));
		Self::deposit_event(Event::FutureYieldSet {
			yield_amount: next_cash_yield,
			index: yield_index,
			timestamp: timestamp_effective,
		});
		Ok(())
	}
}

impl<T: Config> CompoundCashTrait<Balance, Moment> for Pallet<T> {
	fn set_future_yield(next_cash_yield: Balance, yield_index: u128, timestamp_effective: Moment) -> DispatchResult {
		Self::set_future_yield(next_cash_yield, yield_index, timestamp_effective)?;
		Ok(())
	}
}
