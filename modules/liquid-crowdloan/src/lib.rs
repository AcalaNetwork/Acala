// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

//! # Liquid Crowdloan Module
//!
//! Allow people to redeem lcDOT for DOT.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId};
use sp_runtime::traits::AccountIdConversion;

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		#[pallet::constant]
		type LiquidCrowdloanCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Liquid Crowdloan asset was redeemed.
		Redeemed { amount: Balance },
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(0)]
		pub fn redeem(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			T::Currency::withdraw(T::LiquidCrowdloanCurrencyId::get(), &who, amount)?;

			T::Currency::transfer(T::LiquidCrowdloanCurrencyId::get(), &Self::account_id(), &who, amount)?;

			Self::deposit_event(Event::Redeemed { amount });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}
