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

//! # DEX Oracle Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]

use frame_support::{pallet_prelude::*, traits::Time};
use frame_system::pallet_prelude::*;
use module_support::{DEXManager, DEXPriceProvider, ExchangeRate};
use orml_traits::Happened;
use primitives::{Balance, CurrencyId, TradingPair};
use sp_core::U256;
use sp_runtime::{
	traits::{Saturating, Zero},
	FixedPointNumber, SaturatedConversion,
};
use sp_std::marker::PhantomData;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// DEX provide liquidity info.
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		/// Time provider
		type Time: Time;

		/// The origin which may manage dex oracle.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Already enabled average price for this trading pair.
		AveragePriceAlreadyEnabled,
		/// The trading pair must be enabled average price.
		AveragePriceMustBeEnabled,
		/// The liquidity pool is invalid.
		InvalidPool,
		/// The currency id is invalid.
		InvalidCurrencyId,
		/// The interval is zero.
		IntervalIsZero,
	}

	/// Price cumulatives for TradingPair.
	///
	/// Cumulatives: map TradingPair => (Cumulative0, Cumulative1, LastUpdateTimestamp)
	#[pallet::storage]
	#[pallet::getter(fn cumulatives)]
	pub type Cumulatives<T: Config> = StorageMap<_, Twox64Concat, TradingPair, (U256, U256, MomentOf<T>), ValueQuery>;

	/// Average prices for TradingPair.
	///
	/// AveragePrices: map TradingPair => (AveragePrice0, AveragePrice1, LastCumulative0,
	/// LastCumulative1, LastUpdatePriceTimestamp, InteralToUpdatePrice)
	#[pallet::storage]
	#[pallet::getter(fn average_prices)]
	pub type AveragePrices<T: Config> = StorageMap<
		_,
		Twox64Concat,
		TradingPair,
		(ExchangeRate, ExchangeRate, U256, U256, MomentOf<T>, MomentOf<T>),
		OptionQuery,
	>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			let now = T::Time::now();
			let mut iterate_count: u32 = 0;
			let mut update_count: u32 = 0;

			for (trading_pair, (_, _, last_cumulative_0, last_cumulative_1, last_update_price_time, update_interval)) in
				AveragePrices::<T>::iter()
			{
				iterate_count += 1;
				let elapsed_time = now.saturating_sub(last_update_price_time);

				if elapsed_time >= update_interval {
					// try update cumulative before calculate average price.
					let (pool_0, pool_1) = T::DEX::get_liquidity_pool(trading_pair.first(), trading_pair.second());
					Self::try_update_cumulative(&trading_pair, pool_0, pool_1);

					let (cumulative_0, cumulative_1, _) = Self::cumulatives(trading_pair);
					let u256_elapsed_time: U256 = elapsed_time.saturated_into::<u128>().into();
					let average_price_0 = ExchangeRate::from_inner(
						cumulative_0
							.saturating_sub(last_cumulative_0)
							.checked_div(u256_elapsed_time)
							.expect("shouldn't fail because elapsed_time is not zero")
							.saturated_into::<u128>(),
					);
					let average_price_1 = ExchangeRate::from_inner(
						cumulative_1
							.saturating_sub(last_cumulative_1)
							.checked_div(u256_elapsed_time)
							.expect("shouldn't fail because elapsed_time is not zero")
							.saturated_into::<u128>(),
					);

					AveragePrices::<T>::insert(
						trading_pair,
						(
							average_price_0,
							average_price_1,
							cumulative_0,
							cumulative_1,
							now,
							update_interval,
						),
					);

					update_count += 1;
				}
			}

			<T as Config>::WeightInfo::on_initialize_with_update_average_prices(iterate_count, update_count)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Enabled average price for trading pair.
		///
		/// Requires `UpdateOrigin`
		///
		/// - `currency_id_a`: one currency_id that forms a trading pair
		/// - `currency_id_b`: another currency_id that forms a trading pair
		/// - `interval`: the timestamp interval to update average price.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::enable_average_price())]
		pub fn enable_average_price(
			origin: OriginFor<T>,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
			interval: MomentOf<T>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let trading_pair =
				TradingPair::from_currency_ids(currency_id_a, currency_id_b).ok_or(Error::<T>::InvalidCurrencyId)?;
			ensure!(
				Self::average_prices(trading_pair).is_none(),
				Error::<T>::AveragePriceAlreadyEnabled
			);
			ensure!(!interval.is_zero(), Error::<T>::IntervalIsZero,);

			let (initial_price_0, initial_price_1) =
				Self::get_current_price(&trading_pair).ok_or(Error::<T>::InvalidPool)?;
			let now = T::Time::now();
			let initial_cumulative_0 = U256::zero();
			let initial_cumulative_1 = U256::zero();

			AveragePrices::<T>::insert(
				trading_pair,
				(
					initial_price_0,
					initial_price_1,
					initial_cumulative_0,
					initial_cumulative_1,
					now,
					interval,
				),
			);
			Cumulatives::<T>::insert(trading_pair, (initial_cumulative_0, initial_cumulative_1, now));

			Ok(())
		}

		/// Disable average price for trading pair.
		///
		/// Requires `UpdateOrigin`
		///
		/// - `currency_id_a`: one currency_id that forms a trading pair
		/// - `currency_id_b`: another currency_id that forms a trading pair
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::disable_average_price())]
		pub fn disable_average_price(
			origin: OriginFor<T>,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let trading_pair =
				TradingPair::from_currency_ids(currency_id_a, currency_id_b).ok_or(Error::<T>::InvalidCurrencyId)?;
			AveragePrices::<T>::take(trading_pair).ok_or(Error::<T>::AveragePriceMustBeEnabled)?;
			Cumulatives::<T>::remove(trading_pair);

			Ok(())
		}

		/// Update the interval of the trading pair that enabled average price.
		///
		/// Requires `UpdateOrigin`
		///
		/// - `currency_id_a`: one currency_id that forms a trading pair
		/// - `currency_id_b`: another currency_id that forms a trading pair
		/// - `new_interval`: the new interval.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::update_average_price_interval())]
		pub fn update_average_price_interval(
			origin: OriginFor<T>,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
			new_interval: MomentOf<T>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			let trading_pair =
				TradingPair::from_currency_ids(currency_id_a, currency_id_b).ok_or(Error::<T>::InvalidCurrencyId)?;

			AveragePrices::<T>::try_mutate_exists(trading_pair, |maybe| -> DispatchResult {
				let (_, _, _, _, _, update_interval) = maybe.as_mut().ok_or(Error::<T>::AveragePriceMustBeEnabled)?;
				ensure!(!new_interval.is_zero(), Error::<T>::IntervalIsZero);
				*update_interval = new_interval;
				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// For same trading pair, if now is gt last update cumulative timestamp, update it's
	/// cumulative, otherwise do nothing. It means that in one block, the cumulative of a trading
	/// pair may be updated only once.
	pub fn try_update_cumulative(trading_pair: &TradingPair, pool_0: Balance, pool_1: Balance) {
		// try updating enabled cumulative
		if AveragePrices::<T>::contains_key(trading_pair) {
			Cumulatives::<T>::mutate(
				trading_pair,
				|(cumulative_0, cumulative_1, last_cumulative_timestamp)| {
					let now = T::Time::now();
					// update cumulative only occurs once in one block
					if *last_cumulative_timestamp != now {
						let elapsed_time: U256 = now
							.saturating_sub(*last_cumulative_timestamp)
							.saturated_into::<u128>()
							.into();
						let increased_cumulative_0: U256 = U256::from(
							ExchangeRate::checked_from_rational(pool_1, pool_0)
								.unwrap_or_default()
								.into_inner(),
						)
						.saturating_mul(elapsed_time);
						let increased_cumulative_1: U256 = U256::from(
							ExchangeRate::checked_from_rational(pool_0, pool_1)
								.unwrap_or_default()
								.into_inner(),
						)
						.saturating_mul(elapsed_time);

						*cumulative_0 = cumulative_0.saturating_add(increased_cumulative_0);
						*cumulative_1 = cumulative_1.saturating_add(increased_cumulative_1);
						*last_cumulative_timestamp = now;
					}
				},
			);
		}
	}

	fn get_current_price(trading_pair: &TradingPair) -> Option<(ExchangeRate, ExchangeRate)> {
		let (pool_0, pool_1) = T::DEX::get_liquidity_pool(trading_pair.first(), trading_pair.second());
		ExchangeRate::checked_from_rational(pool_1, pool_0).zip(ExchangeRate::checked_from_rational(pool_0, pool_1))
	}

	fn get_average_price(trading_pair: &TradingPair) -> Option<(ExchangeRate, ExchangeRate)> {
		Self::average_prices(trading_pair).map(|(price_0, price_1, _, _, _, _)| (price_0, price_1))
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

/// AverageDEXPriceProvider that always provider average price.
pub struct AverageDEXPriceProvider<T>(PhantomData<T>);
impl<T: Config> DEXPriceProvider<CurrencyId> for AverageDEXPriceProvider<T> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate> {
		let trading_pair = TradingPair::from_currency_ids(base, quote)?;
		Pallet::<T>::get_average_price(&trading_pair).map(
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

/// PriorityAverageDEXPriceProvider that priority access to the average price, if it is none,
/// will access to real-time price from dex.
pub struct PriorityAverageDEXPriceProvider<T>(PhantomData<T>);
impl<T: Config> DEXPriceProvider<CurrencyId> for PriorityAverageDEXPriceProvider<T> {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<ExchangeRate> {
		let trading_pair = TradingPair::from_currency_ids(base, quote)?;
		Pallet::<T>::get_average_price(&trading_pair)
			.or_else(|| Pallet::<T>::get_current_price(&trading_pair))
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
