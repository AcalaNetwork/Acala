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

//! # Prices Module
//!
//! ## Overview
//!
//! The data from Oracle cannot be used in business, prices module will do some
//! process and feed prices for Acala. Process include:
//!   - specify a fixed price for stable currency
//!   - feed price in USD or related price bewteen two currencies
//!   - lock/unlock the price data get from oracle

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::{DataFeeder, DataProvider, MultiCurrency};
use primitives::{Balance, CurrencyId};
use sp_core::U256;
use sp_runtime::{
	traits::{CheckedDiv, CheckedMul},
	FixedPointNumber,
};
use sp_std::convert::TryInto;
use support::{CurrencyIdMapping, DEXManager, ExchangeRateProvider, Price, PriceProvider};

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

		/// The data source, such as Oracle.
		type Source: DataProvider<CurrencyId, Price> + DataFeeder<CurrencyId, Price, Self::AccountId>;

		/// The stable currency id, it should be AUSD in Acala.
		#[pallet::constant]
		type GetStableCurrencyId: Get<CurrencyId>;

		/// The fixed prices of stable currency, it should be 1 USD in Acala.
		#[pallet::constant]
		type StableCurrencyFixedPrice: Get<Price>;

		/// The staking currency id, it should be DOT in Acala.
		#[pallet::constant]
		type GetStakingCurrencyId: Get<CurrencyId>;

		/// The liquid currency id, it should be LDOT in Acala.
		#[pallet::constant]
		type GetLiquidCurrencyId: Get<CurrencyId>;

		/// The origin which may lock and unlock prices feed to system.
		type LockOrigin: EnsureOrigin<Self::Origin>;

		/// The provider of the exchange rate between liquid currency and
		/// staking currency.
		type LiquidStakingExchangeRateProvider: ExchangeRateProvider;

		/// DEX provide liquidity info.
		type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

		/// Currency provide the total insurance of LPToken.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Mapping between CurrencyId and ERC20 address so user can use Erc20.
		type CurrencyIdMapping: CurrencyIdMapping;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Lock price. \[currency_id, locked_price\]
		LockPrice(CurrencyId, Price),
		/// Unlock price. \[currency_id\]
		UnlockPrice(CurrencyId),
	}

	/// Mapping from currency id to it's locked price
	///
	/// map CurrencyId => Option<Price>
	#[pallet::storage]
	#[pallet::getter(fn locked_price)]
	pub type LockedPrice<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Price, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock the price and feed it to system.
		///
		/// The dispatch origin of this call must be `LockOrigin`.
		///
		/// - `currency_id`: currency type.
		#[pallet::weight((T::WeightInfo::lock_price(), DispatchClass::Operational))]
		#[transactional]
		pub fn lock_price(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResult {
			T::LockOrigin::ensure_origin(origin)?;
			<Pallet<T> as PriceProvider<CurrencyId>>::lock_price(currency_id);
			Ok(())
		}

		/// Unlock the price and get the price from `PriceProvider` again
		///
		/// The dispatch origin of this call must be `LockOrigin`.
		///
		/// - `currency_id`: currency type.
		#[pallet::weight((T::WeightInfo::unlock_price(), DispatchClass::Operational))]
		#[transactional]
		pub fn unlock_price(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResult {
			T::LockOrigin::ensure_origin(origin)?;
			<Pallet<T> as PriceProvider<CurrencyId>>::unlock_price(currency_id);
			Ok(())
		}
	}
}

impl<T: Config> PriceProvider<CurrencyId> for Pallet<T> {
	/// Get exchange rate between two currency,
	/// if priority_locked is true, will try to get the frozen price first
	/// instead of get the real-time price directly.
	///
	/// Note: this returns the price for 1 basic unit
	fn get_relative_price(
		base_currency_id: CurrencyId,
		priority_locked_for_base: bool,
		quote_currency_id: CurrencyId,
		priority_locked_for_quote: bool,
	) -> Option<Price> {
		if let (Some(base_price), Some(quote_price)) = (
			Self::get_price(base_currency_id, priority_locked_for_base),
			Self::get_price(quote_currency_id, priority_locked_for_quote),
		) {
			base_price.checked_div(&quote_price)
		} else {
			None
		}
	}

	/// Get the exchange rate of specific currency to USD,
	/// if priority_locked is true, will try to get the frozen price first
	/// instead of get the real-time price directly.
	///
	/// Note: this returns the price for 1 basic unit
	fn get_price(currency_id: CurrencyId, priority_locked: bool) -> Option<Price> {
		let maybe_price = if currency_id == T::GetStableCurrencyId::get() {
			// if is stable currency, use fixed price
			Some(T::StableCurrencyFixedPrice::get())
		} else if let (true, Some(locked_price)) = (priority_locked, Self::locked_price(currency_id)) {
			// if priority_locked and locked price is some, directly return locked price
			return Some(locked_price);
		} else if currency_id == T::GetLiquidCurrencyId::get() {
			// directly return real-time the multiple of the price of StakingCurrencyId and the exchange rate
			return Self::get_price(T::GetStakingCurrencyId::get(), false)
				.and_then(|n| n.checked_mul(&T::LiquidStakingExchangeRateProvider::get_exchange_rate()));
		} else if let CurrencyId::DexShare(symbol_0, symbol_1) = currency_id {
			let token_0: CurrencyId = symbol_0.into();
			let token_1: CurrencyId = symbol_1.into();

			// directly return the fair price
			return {
				if let (Some(price_0), Some(price_1)) =
					(Self::get_price(token_0, false), Self::get_price(token_1, false))
				{
					let (pool_0, pool_1) = T::DEX::get_liquidity_pool(token_0, token_1);
					let total_shares = T::Currency::total_issuance(currency_id);
					lp_token_fair_price(total_shares, pool_0, pool_1, price_0, price_1)
				} else {
					None
				}
			};
		} else {
			// get real-time price from oracle
			T::Source::get(&currency_id)
		};

		let maybe_adjustment_multiplier = 10u128.checked_pow(T::CurrencyIdMapping::decimals(currency_id)?.into());

		if let (Some(price), Some(adjustment_multiplier)) = (maybe_price, maybe_adjustment_multiplier) {
			// return the price for 1 basic unit
			Price::checked_from_rational(price.into_inner(), adjustment_multiplier)
		} else {
			None
		}
	}

	fn lock_price(currency_id: CurrencyId) {
		// lock real-time price
		if let Some(val) = Self::get_price(currency_id, false) {
			LockedPrice::<T>::insert(currency_id, val);
			<Pallet<T>>::deposit_event(Event::LockPrice(currency_id, val));
		}
	}

	fn unlock_price(currency_id: CurrencyId) {
		if LockedPrice::<T>::take(currency_id).is_some() {
			<Pallet<T>>::deposit_event(Event::UnlockPrice(currency_id));
		}
	}
}

/// The fair price is determined by the external feed price and the size of the liquidity pool:
/// https://blog.alphafinance.io/fair-lp-token-pricing/
/// fair_price = (pool_0 * pool_1)^0.5 * (price_0 * price_1)^0.5 / total_shares * 2
fn lp_token_fair_price(
	total_shares: Balance,
	pool_a: Balance,
	pool_b: Balance,
	price_a: Price,
	price_b: Price,
) -> Option<Price> {
	U256::from(pool_a)
		.saturating_mul(U256::from(pool_b))
		.integer_sqrt()
		.saturating_mul(
			U256::from(price_a.into_inner())
				.saturating_mul(U256::from(price_b.into_inner()))
				.integer_sqrt(),
		)
		.checked_div(U256::from(total_shares))
		.and_then(|n| n.checked_mul(U256::from(2)))
		.and_then(|r| TryInto::<u128>::try_into(r).ok())
		.map(Price::from_inner)
}
