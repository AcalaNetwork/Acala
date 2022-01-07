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

//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Call as MockCall, Event, *};
use orml_traits::MultiCurrency;
use sp_core::offchain::{testing, OffchainDbExt, OffchainWorkerExt, TransactionPoolExt};
use sp_io::offchain;
use sp_runtime::{
	offchain::{DbExternalities, StorageKind},
	traits::BadOrigin,
};
use support::DEXManager;

pub const INIT_TIMESTAMP: u64 = 30_000;
pub const BLOCK_TIME: u64 = 1000;

fn run_to_block_offchain(n: u64) {
	while System::block_number() < n {
		System::set_block_number(System::block_number() + 1);
		Timestamp::set_timestamp((System::block_number() as u64 * BLOCK_TIME) + INIT_TIMESTAMP);
		CDPEngineModule::on_initialize(System::block_number());
		CDPEngineModule::offchain_worker(System::block_number());
		// this unlocks the concurrency storage lock so offchain_worker will fire next block
		offchain::sleep_until(offchain::timestamp().add(Duration::from_millis(LOCK_DURATION + 200)));
	}
}

#[test]
fn check_cdp_status_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(CDPEngineModule::check_cdp_status(BTC, 100, 500), CDPStatus::Safe);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_eq!(CDPEngineModule::check_cdp_status(BTC, 100, 500), CDPStatus::Unsafe);

		MockPriceSource::set_relative_price(None);
		assert_eq!(
			CDPEngineModule::check_cdp_status(BTC, 100, 500),
			CDPStatus::ChecksFailed(Error::<Runtime>::InvalidFeedPrice.into())
		);
	});
}

#[test]
fn get_debit_exchange_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPEngineModule::debit_exchange_rate(BTC), None);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(BTC),
			ExchangeRate::saturating_from_rational(1, 10)
		);

		DebitExchangeRate::<Runtime>::insert(BTC, ExchangeRate::one());
		assert_eq!(CDPEngineModule::debit_exchange_rate(BTC), Some(ExchangeRate::one()));
		assert_eq!(CDPEngineModule::get_debit_exchange_rate(BTC), ExchangeRate::one());
	});
}

#[test]
fn get_liquidation_penalty_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CDPEngineModule::get_liquidation_penalty(BTC),
			DefaultLiquidationPenalty::get()
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::get_liquidation_penalty(BTC),
			Rate::saturating_from_rational(2, 10)
		);
	});
}

#[test]
fn get_liquidation_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CDPEngineModule::get_liquidation_ratio(BTC),
			DefaultLiquidationRatio::get()
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::get_liquidation_ratio(BTC),
			Ratio::saturating_from_rational(5, 2)
		);
	});
}

#[test]
fn set_global_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			CDPEngineModule::set_global_params(Origin::signed(5), Rate::saturating_from_rational(1, 10000)),
			BadOrigin
		);
		assert_ok!(CDPEngineModule::set_global_params(
			Origin::signed(1),
			Rate::saturating_from_rational(1, 10000),
		));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::GlobalInterestRatePerSecUpdated {
			new_global_interest_rate_per_sec: Rate::saturating_from_rational(1, 10000),
		}));
		assert_eq!(
			CDPEngineModule::global_interest_rate_per_sec(),
			Rate::saturating_from_rational(1, 10000)
		);
	});
}

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CDPEngineModule::set_collateral_params(
				Origin::signed(1),
				LDOT,
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
			),
			Error::<Runtime>::InvalidCollateralType
		);

		System::set_block_number(1);
		assert_noop!(
			CDPEngineModule::set_collateral_params(
				Origin::signed(5),
				BTC,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10000),
			),
			BadOrigin
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::InterestRatePerSecUpdated {
			collateral_type: BTC,
			new_interest_rate_per_sec: Some(Rate::saturating_from_rational(1, 100000)),
		}));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::LiquidationRatioUpdated {
			collateral_type: BTC,
			new_liquidation_ratio: Some(Ratio::saturating_from_rational(3, 2)),
		}));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::LiquidationPenaltyUpdated {
			collateral_type: BTC,
			new_liquidation_penalty: Some(Rate::saturating_from_rational(2, 10)),
		}));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::RequiredCollateralRatioUpdated {
			collateral_type: BTC,
			new_required_collateral_ratio: Some(Ratio::saturating_from_rational(9, 5)),
		}));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::MaximumTotalDebitValueUpdated {
			collateral_type: BTC,
			new_total_debit_value: 10000,
		}));

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		let new_collateral_params = CDPEngineModule::collateral_params(BTC);

		assert_eq!(
			new_collateral_params.interest_rate_per_sec,
			Some(Rate::saturating_from_rational(1, 100000))
		);
		assert_eq!(
			new_collateral_params.liquidation_ratio,
			Some(Ratio::saturating_from_rational(3, 2))
		);
		assert_eq!(
			new_collateral_params.liquidation_penalty,
			Some(Rate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			new_collateral_params.required_collateral_ratio,
			Some(Ratio::saturating_from_rational(9, 5))
		);
		assert_eq!(new_collateral_params.maximum_total_debit_value, 10000);
	});
}

#[test]
fn calculate_collateral_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::calculate_collateral_ratio(BTC, 100, 500, Price::saturating_from_rational(1, 1)),
			Ratio::saturating_from_rational(100, 50)
		);
	});
}

#[test]
fn check_debit_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::check_debit_cap(BTC, 100000));
		assert_noop!(
			CDPEngineModule::check_debit_cap(BTC, 100010),
			Error::<Runtime>::ExceedDebitValueHardCap,
		);
	});
}

#[test]
fn check_position_valid_failed_when_invalid_feed_price() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(1, 1))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(10000),
		));

		MockPriceSource::set_relative_price(None);
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 100, 500, true),
			Error::<Runtime>::InvalidFeedPrice
		);

		MockPriceSource::set_relative_price(Some(Price::one()));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 100, 500, true));
	});
}

#[test]
fn check_position_valid_failed_when_remain_debit_value_too_small() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(1, 1))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(10000),
		));
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 2, 10, true),
			Error::<Runtime>::RemainDebitValueTooSmall,
		);
	});
}

#[test]
fn check_position_valid_ratio_below_liquidate_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(10, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 91, 500, true),
			Error::<Runtime>::BelowLiquidationRatio,
		);
	});
}

#[test]
fn check_position_valid_ratio_below_required_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 89, 500, false));
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 89, 500, true),
			Error::<Runtime>::BelowRequiredCollateralRatio
		);
	});
}

#[test]
fn adjust_position_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, ACA, 100, 500),
			Error::<Runtime>::InvalidCollateralType,
		);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert!(!CDPEngineModule::adjust_position(&ALICE, BTC, 0, 200).is_ok());
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -200));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 300);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
	});
}

#[test]
fn remain_debit_value_too_small_check() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert!(!CDPEngineModule::adjust_position(&ALICE, BTC, 0, -490).is_ok());
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, -100, -500));
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_noop!(
			CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::<Runtime>::MustBeUnsafe,
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));

		System::assert_last_event(Event::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			liquidation_strategy: LiquidationStrategy::Auction { auction_count: 1 },
		}));
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);

		mock_shutdown();
		assert_noop!(
			CDPEngineModule::liquidate(Origin::none(), BTC, ALICE),
			Error::<Runtime>::AlreadyShutdown
		);
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction_when_limited_by_slippage() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			BTC,
			AUSD,
			100,
			121,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 121));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		// pool is enough, but slippage limit the swap
		MockPriceSource::set_relative_price(Some(Price::saturating_from_rational(1, 2)));
		assert_eq!(
			DEXModule::get_swap_amount(&vec![BTC, AUSD], SwapLimit::ExactTarget(Balance::MAX, 60)),
			Some((99, 60))
		);
		assert_eq!(
			DEXModule::get_swap_amount(&vec![BTC, AUSD], SwapLimit::ExactSupply(100, 0)),
			Some((100, 60))
		);
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			liquidation_strategy: LiquidationStrategy::Auction { auction_count: 1 },
		}));

		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 121));
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
	});
}

#[test]
fn liquidate_unsafe_cdp_by_swap() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			BTC,
			AUSD,
			100,
			121,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 121));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			liquidation_strategy: LiquidationStrategy::Exchange,
		}));

		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (199, 61));
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 901);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
	});
}

#[test]
fn get_interest_rate_per_sec_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPEngineModule::get_interest_rate_per_sec(BTC), Rate::zero());
		assert_eq!(CDPEngineModule::get_interest_rate_per_sec(DOT), Rate::zero());

		assert_ok!(CDPEngineModule::set_global_params(
			Origin::signed(1),
			Rate::saturating_from_rational(1, 10000),
		));
		assert_eq!(
			CDPEngineModule::get_interest_rate_per_sec(BTC),
			Rate::saturating_from_rational(1, 10000)
		);
		assert_eq!(
			CDPEngineModule::get_interest_rate_per_sec(DOT),
			Rate::saturating_from_rational(1, 10000)
		);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(2, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::get_interest_rate_per_sec(BTC),
			Rate::saturating_from_rational(12, 100000)
		);
		assert_eq!(
			CDPEngineModule::get_interest_rate_per_sec(DOT),
			Rate::saturating_from_rational(1, 10000)
		);
	});
}

#[test]
fn compound_interest_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPEngineModule::compound_interest_rate(Rate::zero(), 10), Rate::zero());
		assert_eq!(
			CDPEngineModule::compound_interest_rate(Rate::saturating_from_rational(1, 10000), 0),
			Rate::zero()
		);
		assert_eq!(
			CDPEngineModule::compound_interest_rate(Rate::saturating_from_rational(1, 10000), 1),
			Rate::saturating_from_rational(1, 10000)
		);
		assert_eq!(
			CDPEngineModule::compound_interest_rate(Rate::saturating_from_rational(1, 10000), 2),
			Rate::saturating_from_rational(20001, 100000000)
		);

		// 1% APY
		assert_eq!(
			CDPEngineModule::compound_interest_rate(
				Rate::saturating_from_rational(315_523_000u128, 1_000_000_000_000_000_000u128),
				6
			),
			Rate::saturating_from_rational(1_893_138_000u128, 1_000_000_000_000_000_000u128)
		);
		assert_eq!(
			CDPEngineModule::compound_interest_rate(
				Rate::saturating_from_rational(315_523_000u128, 1_000_000_000_000_000_000u128),
				12
			),
			Rate::saturating_from_rational(3_786_276_004u128, 1_000_000_000_000_000_000u128)
		);
	});
}

#[test]
fn accumulate_interest_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(2, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		CDPEngineModule::accumulate_interest(1, 0);
		assert_eq!(CDPEngineModule::last_accumulation_secs(), 1);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(BTC),
			ExchangeRate::saturating_from_rational(1, 10)
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(BTC), None);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(DOT),
			ExchangeRate::saturating_from_rational(1, 10)
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 300));

		CDPEngineModule::accumulate_interest(2, 1);
		assert_eq!(CDPEngineModule::last_accumulation_secs(), 2);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(BTC),
			ExchangeRate::saturating_from_rational(101, 1000)
		);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 1000))
		);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(DOT),
			ExchangeRate::saturating_from_rational(1, 10)
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);

		mock_shutdown();
		assert!(<Runtime as Config>::EmergencyShutdown::is_shutdown());

		CDPEngineModule::accumulate_interest(3, 2);
		assert_eq!(CDPEngineModule::last_accumulation_secs(), 3);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(BTC),
			ExchangeRate::saturating_from_rational(101, 1000)
		);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 1000))
		);
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(DOT),
			ExchangeRate::saturating_from_rational(1, 10)
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
	});
}

#[test]
fn settle_cdp_has_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_noop!(
			CDPEngineModule::settle_cdp_has_debit(ALICE, BTC),
			Error::<Runtime>::NoDebitValue,
		);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 500));
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_ok!(CDPEngineModule::settle_cdp_has_debit(ALICE, BTC));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::SettleCDPInDebit {
			collateral_type: BTC,
			owner: ALICE,
		}));
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 50);

		assert_noop!(
			CDPEngineModule::settle(Origin::none(), BTC, ALICE),
			Error::<Runtime>::MustAfterShutdown
		);
	});
}

#[test]
fn close_cdp_has_debit_by_dex_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);

		assert_noop!(
			CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 100),
			Error::<Runtime>::NoDebitValue
		);

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::get_debit_pool(), 0);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_noop!(
			CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 100),
			Error::<Runtime>::MustBeSafe
		);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		// max collateral amount limit swap
		assert_noop!(
			CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 5),
			cdp_treasury::Error::<Runtime>::CannotSwap,
		);

		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 1000));
		assert_ok!(CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 6));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::CloseCDPInDebitByDEX {
			collateral_type: BTC,
			owner: ALICE,
			sold_collateral_amount: 6,
			refund_collateral_amount: 94,
			debit_value: 50,
		}));

		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (106, 950));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 994);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 50);
		assert_eq!(CDPTreasuryModule::get_debit_pool(), 50);
	});
}

#[test]
fn close_cdp_has_debit_by_swap_on_alternative_path() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			BTC,
			ACA,
			100,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			ACA,
			AUSD,
			1000,
			1000,
			0,
			false
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		assert_eq!(DEXModule::get_liquidity_pool(BTC, ACA), (100, 1000));
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (1000, 1000));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::get_debit_pool(), 0);

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_ok!(CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 100));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::CloseCDPInDebitByDEX {
			collateral_type: BTC,
			owner: ALICE,
			sold_collateral_amount: 6,
			refund_collateral_amount: 94,
			debit_value: 50,
		}));

		assert_eq!(DEXModule::get_liquidity_pool(BTC, ACA), (106, 947));
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (1053, 950));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 994);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 50);
		assert_eq!(CDPTreasuryModule::get_debit_pool(), 50);
	});
}

#[test]
fn offchain_worker_works_cdp() {
	let (offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain));

	ext.execute_with(|| {
		// number of currencies allowed as collateral (cycles through all of them)
		let collateral_currencies_num = CollateralCurrencyIds::get().len() as u64;
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		// offchain worker will not liquidate alice
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_ok!(CDPEngineModule::adjust_position(&BOB, BTC, 100, 100));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		// jump 2 blocks at a time because code rotates through the different T::CollateralCurrencyIds
		run_to_block_offchain(System::block_number() + collateral_currencies_num);

		// checks that offchain worker tx pool is empty (therefore tx to liquidate alice is not present)
		assert!(pool_state.write().transactions.pop().is_none());
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);

		// changes alice into unsafe position
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		run_to_block_offchain(System::block_number() + collateral_currencies_num);

		// offchain worker will liquidate alice
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::CDPEngineModule(crate::Call::liquidate {
			currency_id: currency_call,
			who: who_call,
		}) = tx.call
		{
			assert_ok!(CDPEngineModule::liquidate(Origin::none(), currency_call, who_call));
		}
		// empty offchain tx pool (Bob was not liquidated)
		assert!(pool_state.write().transactions.pop().is_none());
		// alice is liquidated but bob is not
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, BOB).debit, 100);
		assert_eq!(LoansModule::positions(BTC, BOB).collateral, 100);

		// emergency shutdown will settle Bobs debit position
		mock_shutdown();
		assert!(MockEmergencyShutdown::is_shutdown());
		run_to_block_offchain(System::block_number() + collateral_currencies_num);
		// offchain worker will settle bob's position
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::CDPEngineModule(crate::Call::settle {
			currency_id: currency_call,
			who: who_call,
		}) = tx.call
		{
			assert_ok!(CDPEngineModule::settle(Origin::none(), currency_call, who_call));
		}
		// emergency shutdown settles bob's debit position
		assert_eq!(LoansModule::positions(BTC, BOB).debit, 0);
		assert_eq!(LoansModule::positions(BTC, BOB).collateral, 90);
	});
}

#[test]
fn offchain_worker_iteration_limit_works() {
	let (mut offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		// sets max iterations value to 1
		offchain.local_storage_set(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS, &1u32.encode());
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_ok!(CDPEngineModule::adjust_position(&BOB, BTC, 100, 500));
		// make both positions unsafe
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		run_to_block_offchain(2);
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::CDPEngineModule(crate::Call::liquidate {
			currency_id: currency_call,
			who: who_call,
		}) = tx.call
		{
			assert_ok!(CDPEngineModule::liquidate(Origin::none(), currency_call, who_call));
		}
		// alice is liquidated but not bob, he will get liquidated next block due to iteration limit
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		// only one tx is submitted due to iteration limit
		assert!(pool_state.write().transactions.pop().is_none());

		// Iterator continues where it was from storage and now liquidates bob
		run_to_block_offchain(3);
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::CDPEngineModule(crate::Call::liquidate {
			currency_id: currency_call,
			who: who_call,
		}) = tx.call
		{
			assert_ok!(CDPEngineModule::liquidate(Origin::none(), currency_call, who_call));
		}
		assert_eq!(LoansModule::positions(BTC, BOB).debit, 0);
		assert_eq!(LoansModule::positions(BTC, BOB).collateral, 0);
		assert!(pool_state.write().transactions.pop().is_none());
	});
}

#[test]
fn offchain_default_max_iterator_works() {
	let (mut offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::lots_of_accounts().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		// checks that max iterations is stored as none
		assert!(offchain
			.local_storage_get(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS)
			.is_none());

		for i in 0..1001 {
			let acount_id: AccountId = i;
			assert_ok!(CDPEngineModule::adjust_position(&acount_id, BTC, 10, 50));
		}

		// make all positions unsafe
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		run_to_block_offchain(2);
		// should only run 1000 iterations stopping due to DEFAULT_MAX_ITERATIONS
		assert_eq!(pool_state.write().transactions.len(), 1000);
		// should only now run 1 iteration to finish off where it ended last block
		run_to_block_offchain(3);
		assert_eq!(pool_state.write().transactions.len(), 1001);
	});
}
