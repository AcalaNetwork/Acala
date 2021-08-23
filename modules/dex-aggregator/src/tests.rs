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
use orml_traits::MultiCurrency;
use support::{AggregatorManager, AvailableAmm, DEXManager};

/// helper function to sort vecs, useful in unit tests for dex aggregator
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
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, AUSDBTCPair::get().into()),
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, AUSDDOTPair::get().into()),
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, DOTBTCPair::get().into()),
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
					AvailablePool::from_pallet_pair(AvailableAmm::Dex, AUSDBTCPair::get().into()),
					AvailablePool::from_pallet_pair(AvailableAmm::Dex, DOTBTCPair::get().into())
				])
			);
		});
}

#[test]
fn test_get_swap_amounts() {
	ExtBuilder::default()
		.initialize_enabled_trading_pairs()
		.initialize_added_liquidity_pools(ALICE)
		.build()
		.execute_with(|| {
			let path1 = vec![AvailablePool::from_pallet_pair(
				AvailableAmm::Dex,
				AUSDDOTPair::get().into(),
			)];
			let swap_ausd_dot: TradingDirection = AUSDDOTPair::get().into();
			let path2 = vec![
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, swap_ausd_dot.swap()),
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, AUSDBTCPair::get().into()),
			];
			let path2_slice: [CurrencyId; 3] = [DOT, AUSD, BTC];
			let invalid_path = vec![
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, AUSDBTCPair::get().into()),
				AvailablePool::from_pallet_pair(AvailableAmm::Dex, DOTBTCPair::get().into()),
			];

			let amount: Balance = 10;

			assert_eq!(
				DexAggregator::get_target_amount(path1.clone(), amount),
				DexModule::aggregator_target_amount(path1.clone()[0].pair, amount)
			);
			assert_ne!(DexAggregator::get_target_amount(path1.clone(), amount), Some(0));

			assert_eq!(
				DexAggregator::get_supply_amount(path1.clone(), amount),
				DexModule::aggregator_supply_amount(path1.clone()[0].pair, amount)
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

#[test]
fn test_swap_supply_extrinisc() {
	ExtBuilder::default()
		.initialize_enabled_trading_pairs()
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			let bob_signed = Origin::signed(BOB);
			assert_eq!(Tokens::free_balance(AUSD, &BOB), 1_000_000_000_000_000_000);
			assert_eq!(Tokens::free_balance(DOT, &BOB), 1_000_000_000_000_000_000);
			assert_eq!(Tokens::free_balance(BTC, &BOB), 1_000_000_000_000_000_000);

			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				AUSD,
				DOT,
				500_000_000_000_000,
				100_000_000_000_000,
				0,
				false,
			));
			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				AUSD,
				BTC,
				100_000_000_000_000,
				10_000_000_000,
				0,
				false,
			));
			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				DOT,
				BTC,
				100_000_000_000_000,
				10_000_000_000,
				0,
				false,
			));

			assert_noop!(
				DexAggregator::swap_with_exact_supply(
					bob_signed.clone(),
					AUSD,
					DOT,
					100_000_000_000_000,
					250_000_000_000_000
				),
				Error::<Runtime>::BelowMinimumTarget
			);
			assert_noop!(
				DexAggregator::swap_with_exact_supply(bob_signed.clone(), ACA, DOT, 100_000, 0),
				Error::<Runtime>::NoPossibleTradingPath
			);

			// the aggregator does not use the same path as DexModule, it will use the more advantageous AUSD =>
			// BTC => DOT path rather than AUSD => DOT, giving the user far more DOT!
			assert_ok!(DexModule::swap_with_exact_supply(
				bob_signed.clone(),
				vec![AUSD, DOT],
				100_000,
				10
			));
			System::assert_last_event(Event::DexModule(dex::Event::Swap(
				BOB,
				vec![AUSD, DOT],
				100_000,
				19_799,
			)));
			assert_ok!(DexAggregator::swap_with_exact_supply(
				bob_signed.clone(),
				AUSD,
				DOT,
				100_000,
				10
			));
			System::assert_last_event(Event::DexAggregator(crate::Event::Swap(
				BOB, AUSD, DOT, 100_000, 89_099,
			)));

			// In this case DexModules and DexAggregator use the same trading path
			assert_ok!(DexModule::swap_with_exact_supply(
				bob_signed.clone(),
				vec![DOT, AUSD],
				100_000,
				10
			));
			System::assert_last_event(Event::DexModule(dex::Event::Swap(
				BOB,
				vec![DOT, AUSD],
				100_000,
				494_999,
			)));
			assert_ok!(DexAggregator::swap_with_exact_supply(
				bob_signed.clone(),
				DOT,
				AUSD,
				100_000,
				10
			));
			System::assert_last_event(Event::DexAggregator(crate::Event::Swap(
				BOB, DOT, AUSD, 100_000, 494_999,
			)));
		});
}

#[test]
fn test_swap_target_extrinisc() {
	ExtBuilder::default()
		.initialize_enabled_trading_pairs()
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			let bob_signed = Origin::signed(BOB);
			assert_eq!(Tokens::free_balance(AUSD, &BOB), 1_000_000_000_000_000_000);
			assert_eq!(Tokens::free_balance(DOT, &BOB), 1_000_000_000_000_000_000);
			assert_eq!(Tokens::free_balance(BTC, &BOB), 1_000_000_000_000_000_000);

			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				AUSD,
				DOT,
				500_000_000_000_000,
				100_000_000_000_000,
				0,
				false,
			));
			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				AUSD,
				BTC,
				100_000_000_000_000,
				10_000_000_000,
				0,
				false,
			));
			assert_ok!(DexModule::add_liquidity(
				Origin::signed(ALICE),
				DOT,
				BTC,
				100_000_000_000_000,
				10_000_000_000,
				0,
				false,
			));

			assert_noop!(
				DexAggregator::swap_with_exact_target(bob_signed.clone(), AUSD, DOT, 100_000_000_000, 250_000_000),
				Error::<Runtime>::AboveMaximumSupply
			);
			assert_noop!(
				DexAggregator::swap_with_exact_target(bob_signed.clone(), ACA, DOT, 100_000, 0),
				Error::<Runtime>::NoPossibleTradingPath
			);

			// the aggregator does not use the same path as DexModule, it will use the more advantageous AUSD =>
			// BTC => DOT path rather than AUSD => DOT, saving the user far more AUSD!
			assert_ok!(DexModule::swap_with_exact_target(
				bob_signed.clone(),
				vec![AUSD, DOT],
				20_000,
				200_000
			));
			System::assert_last_event(Event::DexModule(dex::Event::Swap(
				BOB,
				vec![AUSD, DOT],
				101_011,
				20_000,
			)));
			assert_ok!(DexAggregator::swap_with_exact_target(
				bob_signed.clone(),
				AUSD,
				DOT,
				20_000,
				200_000_000
			));
			System::assert_last_event(Event::DexAggregator(crate::Event::Swap(BOB, AUSD, DOT, 30_304, 20_000)));
		});
}
