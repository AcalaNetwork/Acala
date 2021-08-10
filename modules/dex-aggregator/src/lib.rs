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

//! # Dex-aggregator Module
//!
//! ## Overview
//!
//! Allows Users to input tokens to swap and executes the cheapest path for that pair

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use primitives::{Balance, CurrencyId, TradingPair};
use sp_std::vec;
use support::{AggregatorSuper, AvailableAmm, AvailablePool};

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Max lenghth of trading path
		#[pallet::constant]
		type AggregatorTradingPathLimit: Get<u32>;

		type Aggregator: AggregatorSuper<Self::AccountId, TradingPair, Balance>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	#[pallet::metadata(T::AccountId = "AccountId")]
	pub enum Event<T: Config> {
		/// Use supply currency to swap target currency. \[trader, trading_pair,
		/// supply_currency_amount, target_currency_amount\]
		Swap(T::AccountId, TradingPair, Balance, Balance),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Minimum target was higher than any possible path expected target output
		AboveMinimumTarget,
		/// Aggregator could not find any viable path to perform the swap
		NoPossibleTradingPath,
		/// Invalid CurrencyId
		InvalidCurrencyId,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Trading with DEX-Aggregator, swap with exact supply amount
		///
		/// - `path`: trading path.
		/// - `supply_amount`: exact supply amount.
		/// - `min_target_amount`: acceptable minimum target amount.
		#[pallet::weight(10000)]
		#[transactional]
		pub fn swap_with_exact_supply(
			origin: OriginFor<T>,
			input_token: CurrencyId,
			output_token: CurrencyId,
			#[pallet::compact] supply_amount: Balance,
			#[pallet::compact] min_target_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let pair =
				TradingPair::from_currency_ids(input_token, output_token).ok_or(Error::<T>::InvalidCurrencyId)?;
			let best_path =
				Self::optimal_path_with_exact_supply(pair, supply_amount).ok_or(Error::<T>::NoPossibleTradingPath)?;
			ensure!(best_path.1 > min_target_amount, Error::<T>::AboveMinimumTarget);

			Ok(())
		}

		/*
		/// Trading with DEX-Aggregator, swap with exact target amount
		///
		/// - `path`: trading path.
		/// - `target_amount`: exact target amount.
		/// - `max_supply_amount`: acceptable maximum supply amount.
		#[pallet::weight(10000)]
		#[transactional]
		pub fn swap_with_exact_target(
			origin: OriginFor<T>,
			input_asset: CurrencyId,
			output_asset: CurrencyId,
			#[pallet::compact] target_amount: Balance,
			#[pallet::compact] max_supply_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Ok(())
		}*/
	}
}

impl<T: Config> Pallet<T> {
	/// Retrieves all available pools that can perform swaps of trading pairs
	fn all_active_pairs() -> Vec<AvailablePool> {
		T::Aggregator::all_active_pairs()
	}

	/// Returns supply amount given the trading path and the target amount. Returns None if path
	/// cannot be swapped.
	fn get_supply_amount(path: Vec<AvailablePool>, target_amount: Balance) -> Option<Balance> {
		let mut cache_money = target_amount;
		let mut cache_pool = match path.len() {
			0 => return None,
			n => path[n - 1].1.second(),
		};

		for pool in path.iter().rev() {
			if cache_pool == pool.1.clone().second() {
				cache_money = match T::Aggregator::pallet_get_supply_amount(*pool, cache_money) {
					Some(i) => i,
					None => return None,
				};
				cache_pool = pool.1.clone().first();
			} else {
				return None;
			}
		}
		Some(cache_money)
	}

	/// Returns target amount given the trading path and the supply amount. Returns None if path
	/// cannot be swapped.
	fn get_target_amount(path: Vec<AvailablePool>, supply_amount: Balance) -> Option<Balance> {
		let mut cache_money = supply_amount;
		if path.len() == 0 {
			return None;
		}
		// can panic but above line checks if vec is empty
		let mut cache_pool = path[0].1.first();

		for pool in path.iter() {
			if cache_pool == pool.1.clone().first() {
				cache_money = match T::Aggregator::pallet_get_target_amount(*pool, cache_money) {
					Some(i) => i,
					None => return None,
				};
				cache_pool = pool.1.clone().second()
			} else {
				return None;
			}
		}
		Some(cache_money)
	}

	/// Returns tuple of optimal path with expected target amount. Returns None if trade is not
	/// possible
	fn optimal_path_with_exact_supply(
		pair: TradingPair,
		supply_amount: Balance,
	) -> Option<(Vec<TradingPair>, Balance)> {
		let mut i: u32 = 0;
		let all_pools = Self::all_active_pairs();
		let mut optimal_path: Vec<TradingPair> = Vec::new();
		let mut optimal_balance: Balance = 0;
		while i < T::AggregatorTradingPathLimit::get() {
			if i == 0 {
				for pool in all_pools.clone() {
					if pair == pool.1 {
						if let Some(new_balance) = Self::get_target_amount(vec![pool], supply_amount) {
							if new_balance > optimal_balance {
								optimal_balance = new_balance;
								optimal_path = vec![pool.1];
							}
						}
					}

					if pair == pool.1.swap() {
						if let Some(new_balance) = Self::get_target_amount(vec![pool], supply_amount) {
							if new_balance > optimal_balance {
								optimal_balance = new_balance;
								optimal_path = vec![pool.1.swap()];
							}
						}
					}
				}
			}
			i += 1;
		}

		if optimal_path.is_empty() {
			None
		} else {
			Some((optimal_path, optimal_balance))
		}
	}
}

/*
fn trading_pair_equivilent(path_pair: (AvailableAmm<T>, TradingPair), path_pair2: (AvailableAmm<T>, TradingPair)) -> bool {
	if path_pair.0 == path_pair2.0 {
		if path_pair.1 == path_pair2.1 || path_pair.1 == path_pair2.1.swap() {
			true
		} else {
			false
		}
	} else {
		false
	}
}*/
