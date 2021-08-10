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

//! Tests for dex-aggregator

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	AUSDBTCPair, AUSDDOTPair, DOTBTCPair, DexAggregator, DexModule, Event, ExtBuilder, ListingOrigin, Origin, Runtime,
	System, Tokens, ACA, ALICE, AUSD, BOB, BTC, DOT,
};
use support::{AggregatorManager, DEXManager};

fn sorted_vec<T: Ord>(mut vec: Vec<T>) -> Vec<T> {
	vec.sort();
	vec
}

#[test]
fn test_all_active_pairs() {
	ExtBuilder::default()
		.initialize_enabled_trading_pairs()
		.build()
		.execute_with(|| {
			let all_pairs = vec![
				AvailablePool(AvailableAmm::Dex, AUSDBTCPair::get()),
				AvailablePool(AvailableAmm::Dex, AUSDDOTPair::get()),
				AvailablePool(AvailableAmm::Dex, DOTBTCPair::get()),
			];
			assert_eq!(
				sorted_vec(DexAggregator::all_active_pairs()),
				sorted_vec(all_pairs.clone())
			);

			assert_ok!(DexModule::disable_trading_pair(
				Origin::signed(ListingOrigin::get()),
				AUSD,
				DOT
			));
			assert_eq!(
				sorted_vec(DexAggregator::all_active_pairs()),
				sorted_vec(vec![
					AvailablePool(AvailableAmm::Dex, AUSDBTCPair::get()),
					AvailablePool(AvailableAmm::Dex, DOTBTCPair::get())
				])
			);
		});
}

#[test]
fn test_swap_amounts() {
	ExtBuilder::default()
		.initialize_enabled_trading_pairs()
		.initialize_added_liquidity_pools(ALICE)
		.build()
		.execute_with(|| {
			let path1 = vec![AvailablePool(AvailableAmm::Dex, AUSDDOTPair::get())];
			let path2 = vec![
				AvailablePool(AvailableAmm::Dex, AUSDDOTPair::get().swap()),
				AvailablePool(AvailableAmm::Dex, AUSDBTCPair::get()),
			];
			let path2_slice: [CurrencyId; 3] = [DOT, AUSD, BTC];
			let invalid_path = vec![
				AvailablePool(AvailableAmm::Dex, AUSDBTCPair::get()),
				AvailablePool(AvailableAmm::Dex, DOTBTCPair::get()),
			];

			let amount: Balance = 10;

			assert_eq!(
				DexAggregator::get_target_amount(path1.clone(), amount),
				DexModule::aggregator_target_amount(path1.clone()[0].1, amount)
			);
			assert_ne!(DexAggregator::get_target_amount(path1.clone(), amount), Some(10));

			assert_eq!(
				DexAggregator::get_supply_amount(path1.clone(), amount),
				DexModule::aggregator_supply_amount(path1.clone()[0].1, amount)
			);

			assert_eq!(
				DexAggregator::get_target_amount(path2.clone(), amount),
				DexModule::get_swap_target_amount(&path2_slice, amount)
			);
			assert_eq!(
				DexAggregator::get_supply_amount(path2.clone(), amount),
				DexModule::get_swap_supply_amount(&path2_slice, amount)
			);

			assert_eq!(DexAggregator::get_target_amount(invalid_path.clone(), amount), None);
			assert_eq!(DexAggregator::get_supply_amount(invalid_path.clone(), amount), None);

			assert_eq!(DexAggregator::get_supply_amount(Vec::new(), amount), None);
			assert_eq!(DexAggregator::get_target_amount(Vec::new(), amount), None);
		});
}
