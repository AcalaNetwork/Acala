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

//! # Honzon Bridge Module
//! This module provides interface for user to transfer Stablecoin and Bridge Stable coin
//! in and out of the chain.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::AccountIdConversion;

use primitives::{Balance, CurrencyId};

use orml_traits::MultiCurrency;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Currency ID of current chain's stable currency
		#[pallet::constant]
		type StablecoinCurrencyId: Get<CurrencyId>;

		/// Currency ID of the Bridge's Stable currency
		#[pallet::constant]
		type BridgedStableCoinCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// User has exchanged Native stable coin to Bridge's stable coin
		ToBridged { who: T::AccountId, amount: Balance },
		/// User has exchanged Bridge's stable coin to Native's stable coin.
		FromBridged { who: T::AccountId, amount: Balance },
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn integrity_test() {
			assert!(T::StablecoinCurrencyId::get() != T::BridgedStableCoinCurrencyId::get());
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Exchange some amount of Native stable coin into Bridge's stable coin
		///
		/// Parameters:
		/// - `amount`: The amount of stable coin to exchange.
		#[pallet::weight(< T as Config >::WeightInfo::to_bridged())]
		#[transactional]
		pub fn to_bridged(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pallet_account = Self::account_id();

			// transfer amount of StablecoinCurrencyId to PalletId account
			T::Currency::transfer(T::StablecoinCurrencyId::get(), &who, &pallet_account, amount)?;

			// transfer amount of BridgedStableCoinCurrencyId from PalletId account to origin
			T::Currency::transfer(T::BridgedStableCoinCurrencyId::get(), &pallet_account, &who, amount)?;

			Self::deposit_event(Event::<T>::ToBridged { who, amount });
			Ok(())
		}

		/// Exchange some amount of Bridge's stable coin into Native stable coin
		///
		/// Parameters:
		/// - `amount`: The amount of stable coin to exchange.
		#[pallet::weight(< T as Config >::WeightInfo::from_bridged())]
		#[transactional]
		pub fn from_bridged(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pallet_account = Self::account_id();
			// transfer amount of StablecoinCurrencyId to PalletId account
			T::Currency::transfer(T::BridgedStableCoinCurrencyId::get(), &who, &pallet_account, amount)?;

			// transfer amount of BridgedStableCoinCurrencyId from PalletId account to origin
			T::Currency::transfer(T::StablecoinCurrencyId::get(), &pallet_account, &who, amount)?;

			Self::deposit_event(Event::<T>::FromBridged { who, amount });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// Returns the current pallet's account ID.
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}
