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

		type Aggregator: AggregatorSuper<TradingPair, Balance>;
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
			//T::Aggregator::do_swap_with_exact_supply(&who, input_token, output_token, supply_amount,
			// min_target_amount)?;
			Ok(())
		}

		/*
		/// Trading with DEX-Aggregator, swap with exact target amount
		///
		/// - `path`: trading path.
		/// - `target_amount`: exact target amount.
		/// - `max_supply_amount`: acceptable maximum supply amount.
		#[pallet::weight(0)]
		#[transactional]
		pub fn swap_with_exact_target(
			origin: OriginFor<T>,
			path: Vec<CurrencyId>,
			#[pallet::compact] target_amount: Balance,
			#[pallet::compact] max_supply_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Ok(())
		}*/
	}
}

impl<T: Config> Pallet<T> {
	fn all_active_pairs() -> Vec<AvailablePool> {
		T::Aggregator::all_active_pairs()
	}
}

/*
fn all_possible_paths(trading_pair: TradingPair) -> Vec<Vec<(AvailableAmm<T>, TradingPair)>> {
	let mut i: u32 = 0;
	let mut possible_paths: Vec<Vec<(AvailableAmm<T>, TradingPair)>> = Vec::new();
	let mut cached_two_element_paths: Vec<Vec<(AvailableAmm<T>, TradingPair)>> = Vec::new();
	let all_pairs = Self::all_active_pairs();
	while i < T::AggregatorTradingPathLimit::get() {
		if i == 0 {
			for (amm, active_pair) in all_pairs.clone() {
				if trading_pair == active_pair {
					possible_paths.push(vec![(amm, active_pair)]);
				} else if trading_pair.swap() == active_pair {
					possible_paths.push(vec![(amm, trading_pair.swap())]);
				}
			}
		} else if i == 1 {
			for (amm, active_pair) in all_pairs.clone() {
				if trading_pair.first() == active_pair.first() {

				} else if trading_pair == active_pair.swap() {

				}
			}

		}
		i += 1;
	}
	possible_paths
}

fn best_path_with_exact_target(target: Balance, trading_pair: TradingPair) -> Vec<(AvailableAmm<T>, Vec<CurrencyId>)> {
	vec![]
}

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
