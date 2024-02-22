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

//! # Honzon Bridge Module
//! This module provides interface for user to transfer Stablecoin and Bridge Stable coin
//! in and out of the chain.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

use primitives::{currency::KUSD, evm::EvmAddress, Balance, CurrencyId};

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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi-currency support for asset management
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Currency ID of current chain's stable currency
		#[pallet::constant]
		type StableCoinCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		type HonzonBridgeAccount: Get<Self::AccountId>;

		/// The origin which set the Currency ID of the Bridge's Stable currency.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	/// Currency ID of the Bridge's Stable currency
	///
	/// BridgedStableCoinCurrencyId: CurrencyId
	#[pallet::storage]
	#[pallet::getter(fn bridged_stable_coin_currency_id)]
	pub type BridgedStableCoinCurrencyId<T: Config> = StorageValue<_, CurrencyId, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The Bridge's stable coin currency doesn't set.
		BridgedStableCoinCurrencyIdNotSet,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Set the Bridge's stable coin currency id.
		BridgedStableCoinCurrencyIdSet {
			bridged_stable_coin_currency_id: CurrencyId,
		},
		/// User has exchanged Native stable coin to Bridge's stable coin.
		ToBridged { who: T::AccountId, amount: Balance },
		/// User has exchanged Bridge's stable coin to Native's stable coin.
		FromBridged { who: T::AccountId, amount: Balance },
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			assert!(T::StableCoinCurrencyId::get() == KUSD);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the Bridge's stable coin currency id.
		///
		/// Parameters:
		/// - `address`: The address of the Bridge's stable coin currency id.
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::set_bridged_stable_coin_address())]
		pub fn set_bridged_stable_coin_address(origin: OriginFor<T>, address: EvmAddress) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let currency_id = CurrencyId::Erc20(address);

			BridgedStableCoinCurrencyId::<T>::put(currency_id);

			Self::deposit_event(Event::<T>::BridgedStableCoinCurrencyIdSet {
				bridged_stable_coin_currency_id: currency_id,
			});
			Ok(())
		}

		/// Exchange some amount of Native stable coin into Bridge's stable coin
		///
		/// Parameters:
		/// - `amount`: The amount of stable coin to exchange.
		#[pallet::call_index(1)]
		#[pallet::weight(< T as Config >::WeightInfo::to_bridged())]
		pub fn to_bridged(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pallet_account = T::HonzonBridgeAccount::get();
			let bridged_stable_coin_currency_id =
				Self::bridged_stable_coin_currency_id().ok_or(Error::<T>::BridgedStableCoinCurrencyIdNotSet)?;

			// transfer amount of StableCoinCurrencyId to PalletId account
			T::Currency::transfer(T::StableCoinCurrencyId::get(), &who, &pallet_account, amount)?;

			// transfer amount of BridgedStableCoinCurrencyId from PalletId account to origin
			T::Currency::transfer(bridged_stable_coin_currency_id, &pallet_account, &who, amount)?;

			Self::deposit_event(Event::<T>::ToBridged { who, amount });
			Ok(())
		}

		/// Exchange some amount of Bridge's stable coin into Native stable coin
		///
		/// Parameters:
		/// - `amount`: The amount of stable coin to exchange.
		#[pallet::call_index(2)]
		#[pallet::weight(< T as Config >::WeightInfo::from_bridged())]
		pub fn from_bridged(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pallet_account = T::HonzonBridgeAccount::get();
			let bridged_stable_coin_currency_id =
				Self::bridged_stable_coin_currency_id().ok_or(Error::<T>::BridgedStableCoinCurrencyIdNotSet)?;

			// transfer amount of BridgedStableCoinCurrencyId to PalletId account
			T::Currency::transfer(bridged_stable_coin_currency_id, &who, &pallet_account, amount)?;

			// transfer amount of StableCoinCurrencyId from PalletId account to origin
			T::Currency::transfer(T::StableCoinCurrencyId::get(), &pallet_account, &who, amount)?;

			Self::deposit_event(Event::<T>::FromBridged { who, amount });
			Ok(())
		}
	}
}
