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

//! # DEX Oracle Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::Time, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::Happened;
use primitives::{Balance, CurrencyId, TradingPair};
use sp_core::U256;
use sp_runtime::{traits::Saturating, FixedPointNumber, SaturatedConversion};
use sp_std::marker::PhantomData;
use support::{DEXManager, DEXPriceProvider, ExchangeRate};

mod mock;
mod tests;
//pub mod weights;

pub use module::*;
//pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// DEX provide liquidity info.
		type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

		/// Time provider
		type Time: Time;

		/// The origin which may manage dex oracle.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// The time interval in millisecond for updating the cumulative prices.
		#[pallet::constant]
		type IntervalToUpdateCumulativePrice: Get<MomentOf<Self>>;

		// /// Weight information for the extrinsics in this module.
		// type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		CumulativePricesAlreadyExisted,
		CumulativePricesNotExists,
		InvalidPool,
		InvalidCurrencyId,
	}

	#[pallet::storage]
	#[pallet::getter(fn cumulatives)]
	pub type Cumulatives<T: Config> = StorageMap<_, Twox64Concat, TradingPair, (U256, U256, MomentOf<T>), ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn cumulative_prices)]
	pub type CumulativePrices<T: Config> =
		StorageMap<_, Twox64Concat, TradingPair, (ExchangeRate, ExchangeRate, U256, U256), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn last_price_updated_time)]
	pub type LastPriceUpdatedTime<T: Config> = StorageValue<_, MomentOf<T>, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			let now = T::Time::now();
			let last_price_updated_time = Self::last_price_updated_time();
			let interval = now.saturating_sub(last_price_updated_time);

			if interval >= T::IntervalToUpdateCumulativePrice::get() {
				for (trading_pair, (_, _, last_cumulative_0, last_cumulative_1)) in CumulativePrices::<T>::iter() {
					// update cumulative before calculate cumulative price.
					let (pool_0, pool_1) = T::DEX::get_liquidity_pool(trading_pair.first(), trading_pair.second());
					Self::try_update_cumulative(&trading_pair, pool_0, pool_1);

					let (cumulative_0, cumulative_1, _) = Self::cumulatives(&trading_pair);
					let u256_interval: U256 = interval.saturated_into::<Balance>().into();
					let cumulative_price_0 = ExchangeRate::from_inner(
						cumulative_0
							.saturating_sub(last_cumulative_0)
							.checked_div(u256_interval)
							.expect("shouldn't fail because interval is not zero")
							.saturated_into::<Balance>(),
					);
					let cumulative_price_1 = ExchangeRate::from_inner(
						cumulative_1
							.saturating_sub(last_cumulative_1)
							.checked_div(u256_interval)
							.expect("shouldn't fail because interval is not zero")
							.saturated_into::<Balance>(),
					);

					CumulativePrices::<T>::insert(
						&trading_pair,
						(cumulative_price_0, cumulative_price_1, cumulative_0, cumulative_1),
					);
				}

				LastPriceUpdatedTime::<T>::put(now);
			}

			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000)]
		#[transactional]
		pub fn enable_cumulative(
			origin: OriginFor<T>,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let trading_pair =
				TradingPair::from_currency_ids(currency_id_a, currency_id_b).ok_or(Error::<T>::InvalidCurrencyId)?;
			ensure!(
				Self::cumulative_prices(&trading_pair).is_none(),
				Error::<T>::CumulativePricesAlreadyExisted
			);

			let (initial_price_0, initial_price_1) =
				Self::get_current_price(&trading_pair).ok_or(Error::<T>::InvalidPool)?;
			let initial_cumulative_0 = U256::zero();
			let initial_cumulative_1 = U256::zero();
			let now = T::Time::now();

			CumulativePrices::<T>::insert(
				&trading_pair,
				(
					initial_price_0,
					initial_price_1,
					initial_cumulative_0,
					initial_cumulative_1,
				),
			);
			Cumulatives::<T>::insert(&trading_pair, (initial_cumulative_0, initial_cumulative_1, now));

			Ok(())
		}

		#[pallet::weight(10_000)]
		#[transactional]
		pub fn disable_cumulative(
			origin: OriginFor<T>,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let trading_pair =
				TradingPair::from_currency_ids(currency_id_a, currency_id_b).ok_or(Error::<T>::InvalidCurrencyId)?;
			let _ = CumulativePrices::<T>::take(&trading_pair).ok_or(Error::<T>::CumulativePricesNotExists)?;
			Cumulatives::<T>::remove(&trading_pair);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn try_update_cumulative(trading_pair: &TradingPair, pool_0: Balance, pool_1: Balance) {
		// try updating enabled cumulative
		if CumulativePrices::<T>::contains_key(trading_pair) {
			Cumulatives::<T>::mutate(trading_pair, |(cumulative_0, cumulative_1, last_timestamp)| {
				let now = T::Time::now();
				// update cumulative only occurs once in one block
				if *last_timestamp != now {
					let interval: U256 = now.saturating_sub(*last_timestamp).saturated_into::<Balance>().into();
					let pool_0_cumulative: U256 = U256::from(
						ExchangeRate::checked_from_rational(pool_1, pool_0)
							.unwrap_or_default()
							.into_inner(),
					)
					.saturating_mul(interval);
					let pool_1_cumulative: U256 = U256::from(
						ExchangeRate::checked_from_rational(pool_0, pool_1)
							.unwrap_or_default()
							.into_inner(),
					)
					.saturating_mul(interval);

					*cumulative_0 = cumulative_0.saturating_add(pool_0_cumulative);
					*cumulative_1 = cumulative_1.saturating_add(pool_1_cumulative);
					*last_timestamp = now;
				}
			});
		}
	}

	fn get_current_price(trading_pair: &TradingPair) -> Option<(ExchangeRate, ExchangeRate)> {
		let (pool_0, pool_1) = T::DEX::get_liquidity_pool(trading_pair.first(), trading_pair.second());
		ExchangeRate::checked_from_rational(pool_1, pool_0).zip(ExchangeRate::checked_from_rational(pool_0, pool_1))
	}

	fn get_cumulative_price(trading_pair: &TradingPair) -> Option<(ExchangeRate, ExchangeRate)> {
		Self::cumulative_prices(trading_pair).map(|(price_0, price_1, _, _)| (price_0, price_1))
	}
}

impl<T: Config> Happened<(TradingPair, Balance, Balance)> for Pallet<T> {
	fn happened(info: &(TradingPair, Balance, Balance)) {
		let (trading_pair, pool_0, pool_1) = *info;
		Self::try_update_cumulative(&trading_pair, pool_0, pool_1);
	}
}

/// CurrentDEXPriceProvider that always provider real-time prices from dex.
pub struct CurrentDEXPriceProvider<T>(PhantomData<T>);
impl<T: Config> DEXPriceProvider<CurrencyId> for CurrentDEXPriceProvider<T> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate> {
		let trading_pair = TradingPair::from_currency_ids(base, quote)?;
		Pallet::<T>::get_current_price(&trading_pair).map(
			|(price_0, price_1)| {
				if base == trading_pair.first() {
					price_0
				} else {
					price_1
				}
			},
		)
	}
}

/// CumulativeDEXPriceProvider that always provider cumulative prices.
pub struct CumulativeDEXPriceProvider<T>(PhantomData<T>);
impl<T: Config> DEXPriceProvider<CurrencyId> for CumulativeDEXPriceProvider<T> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate> {
		let trading_pair = TradingPair::from_currency_ids(base, quote)?;
		Pallet::<T>::get_cumulative_price(&trading_pair).map(|(price_0, price_1)| {
			if base == trading_pair.first() {
				price_0
			} else {
				price_1
			}
		})
	}
}

/// PriorityCumulativeDEXPriceProvider that priority access to the cumulative price, if it is none,
/// will access to real-time price from dex.
pub struct PriorityCumulativeDEXPriceProvider<T>(PhantomData<T>);
impl<T: Config> DEXPriceProvider<CurrencyId> for PriorityCumulativeDEXPriceProvider<T> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate> {
		let trading_pair = TradingPair::from_currency_ids(base, quote)?;
		Pallet::<T>::get_cumulative_price(&trading_pair)
			.or(Pallet::<T>::get_current_price(&trading_pair))
			.map(
				|(price_0, price_1)| {
					if base == trading_pair.first() {
						price_0
					} else {
						price_1
					}
				},
			)
	}
}
