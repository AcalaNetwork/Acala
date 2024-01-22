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

//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_err, assert_noop, assert_ok};
use mock::{RuntimeCall as MockCall, RuntimeEvent, *};
use module_support::{DEXManager, SwapError};
use orml_traits::MultiCurrency;
use sp_core::offchain::{testing, OffchainDbExt, OffchainWorkerExt, TransactionPoolExt};
use sp_io::offchain;
use sp_runtime::{
	offchain::{DbExternalities, StorageKind},
	traits::BadOrigin,
};

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

fn setup_default_collateral(currency_id: CurrencyId) {
	assert_ok!(CDPEngineModule::set_collateral_params(
		RuntimeOrigin::signed(ALICE),
		currency_id,
		Change::NewValue(Some(Default::default())),
		Change::NoChange,
		Change::NoChange,
		Change::NoChange,
		Change::NewValue(10000),
	));
}

#[test]
fn check_cdp_status_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(CDPEngineModule::check_cdp_status(BTC, 100, 500), CDPStatus::Safe);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_eq!(CDPEngineModule::check_cdp_status(BTC, 100, 500), CDPStatus::Unsafe);

		MockPriceSource::set_price(BTC, None);
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
		assert_noop!(
			CDPEngineModule::get_liquidation_penalty(BTC),
			Error::<Runtime>::InvalidCollateralType
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::get_liquidation_penalty(BTC),
			Ok(Rate::saturating_from_rational(2, 10))
		);
	});
}

#[test]
fn get_liquidation_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CDPEngineModule::get_liquidation_ratio(BTC),
			Error::<Runtime>::InvalidCollateralType
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(5, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_eq!(
			CDPEngineModule::get_liquidation_ratio(BTC),
			Ok(Ratio::saturating_from_rational(5, 2))
		);
	});
}

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			CDPEngineModule::set_collateral_params(
				RuntimeOrigin::signed(AccountId::new([5u8; 32])),
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		System::assert_has_event(RuntimeEvent::CDPEngineModule(crate::Event::InterestRatePerSecUpdated {
			collateral_type: BTC,
			new_interest_rate_per_sec: Some(Rate::saturating_from_rational(1, 100000)),
		}));
		System::assert_has_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidationRatioUpdated {
			collateral_type: BTC,
			new_liquidation_ratio: Some(Ratio::saturating_from_rational(3, 2)),
		}));
		System::assert_has_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidationPenaltyUpdated {
			collateral_type: BTC,
			new_liquidation_penalty: Some(Rate::saturating_from_rational(2, 10)),
		}));
		System::assert_has_event(RuntimeEvent::CDPEngineModule(
			crate::Event::RequiredCollateralRatioUpdated {
				collateral_type: BTC,
				new_required_collateral_ratio: Some(Ratio::saturating_from_rational(9, 5)),
			},
		));
		System::assert_has_event(RuntimeEvent::CDPEngineModule(
			crate::Event::MaximumTotalDebitValueUpdated {
				collateral_type: BTC,
				new_total_debit_value: 10000,
			},
		));

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		let new_collateral_params = CDPEngineModule::collateral_params(BTC).unwrap();

		assert_eq!(
			new_collateral_params.interest_rate_per_sec.map(|v| v.into_inner()),
			Some(Rate::saturating_from_rational(1, 100000))
		);
		assert_eq!(
			new_collateral_params.liquidation_ratio,
			Some(Ratio::saturating_from_rational(3, 2))
		);
		assert_eq!(
			new_collateral_params.liquidation_penalty.map(|v| v.into_inner()),
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
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(1, 1))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(10000),
		));

		MockPriceSource::set_price(BTC, None);
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 100, 500, true),
			Error::<Runtime>::InvalidFeedPrice
		);

		MockPriceSource::set_price(BTC, Some(Price::one()));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 100, 500, true));
	});
}

#[test]
fn check_position_valid_failed_when_remain_debit_value_too_small() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
		setup_default_collateral(BTC);
		setup_default_collateral(AUSD);

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
fn adjust_position_by_debit_value_work() {
	ExtBuilder::default().build().execute_with(|| {
		setup_default_collateral(BTC);

		assert_noop!(
			CDPEngineModule::adjust_position_by_debit_value(&ALICE, ACA, 100, 5000),
			Error::<Runtime>::InvalidCollateralType,
		);

		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);

		assert_ok!(CDPEngineModule::adjust_position_by_debit_value(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);

		assert_ok!(CDPEngineModule::adjust_position_by_debit_value(&ALICE, BTC, 100, 100));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 800);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 200);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 1000);

		assert_ok!(CDPEngineModule::adjust_position_by_debit_value(&ALICE, BTC, 0, -30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 800);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 70);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 200);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 700);

		assert_noop!(
			CDPEngineModule::adjust_position_by_debit_value(&ALICE, BTC, 0, -69),
			Error::<Runtime>::RemainDebitValueTooSmall
		);

		// if payback value is over the actual debit, just payback the actual debit.
		assert_ok!(CDPEngineModule::adjust_position_by_debit_value(&ALICE, BTC, 0, -999999));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 800);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 200);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
	});
}

#[test]
fn expand_position_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		MockPriceSource::set_price(DOT, Some(Price::saturating_from_rational(10, 1)));
		setup_default_collateral(AUSD);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, DOT, 100, 2500));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 100,
				debit: 2500
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 250);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 100);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);

		assert_noop!(
			CDPEngineModule::expand_position_collateral(&ALICE, DOT, 0, 1),
			SwapError::CannotSwap
		);

		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			10000,
			1000,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 10000));
		assert_noop!(
			CDPEngineModule::expand_position_collateral(&ALICE, DOT, 250, 100),
			SwapError::CannotSwap
		);

		assert_ok!(CDPEngineModule::expand_position_collateral(&ALICE, DOT, 250, 20));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 124,
				debit: 5000
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 250);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 124);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (976, 10250));

		assert_ok!(CDPEngineModule::expand_position_collateral(&ALICE, DOT, 200, 18));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 142,
				debit: 7000
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 142);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (958, 10450));

		// make position below the RequireCollateralRatio
		assert_ok!(CDPEngineModule::expand_position_collateral(&ALICE, DOT, 100, 0));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 151,
				debit: 8000,
			}
		);

		assert_noop!(
			CDPEngineModule::expand_position_collateral(&ALICE, DOT, 800, 0),
			Error::<Runtime>::BelowLiquidationRatio
		);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
			Change::NewValue(900),
		));
		assert_noop!(
			CDPEngineModule::expand_position_collateral(&ALICE, DOT, 101, 0),
			Error::<Runtime>::ExceedDebitValueHardCap
		);
	});
}

#[test]
fn expand_position_collateral_for_lp_ausd_dot_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			10000,
			1000,
			0,
			false
		));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 20000);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CAROL),
			ALICE,
			LP_AUSD_DOT,
			1000
		));

		MockPriceSource::set_price(LP_AUSD_DOT, Some(Price::saturating_from_rational(1, 1)));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, LP_AUSD_DOT, 1000, 2000));
		assert_eq!(
			LoansModule::positions(LP_AUSD_DOT, ALICE),
			Position {
				collateral: 1000,
				debit: 2000
			}
		);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 200);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1000);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 10000));

		assert_noop!(
			CDPEngineModule::expand_position_collateral(&ALICE, LP_AUSD_DOT, 200, 200),
			module_dex::Error::<Runtime>::UnacceptableShareIncrement
		);

		assert_ok!(CDPEngineModule::expand_position_collateral(
			&ALICE,
			LP_AUSD_DOT,
			300,
			100
		));
		assert_eq!(
			LoansModule::positions(LP_AUSD_DOT, ALICE),
			Position {
				collateral: 1283,
				debit: 5000
			}
		);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 206);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1283);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 10294));
	});
}

#[test]
fn shrink_position_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		MockPriceSource::set_price(DOT, Some(Price::saturating_from_rational(10, 1)));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(AUSD);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, DOT, 100, 5000));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 100,
				debit: 5000
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 100);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);

		MockPriceSource::set_price(DOT, Some(Price::saturating_from_rational(8, 1)));
		assert_noop!(
			CDPEngineModule::shrink_position_debit(&ALICE, DOT, 10, 0),
			SwapError::CannotSwap
		);

		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			8000,
			1000,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 8000));
		assert_noop!(
			CDPEngineModule::shrink_position_debit(&ALICE, DOT, 10, 80),
			SwapError::CannotSwap
		);

		assert_ok!(CDPEngineModule::shrink_position_debit(&ALICE, DOT, 10, 70));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 90,
				debit: 4210
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 90);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1010, 7921));

		assert_ok!(CDPEngineModule::shrink_position_debit(&ALICE, DOT, 70, 0));
		assert_eq!(
			LoansModule::positions(DOT, ALICE),
			Position {
				collateral: 20,
				debit: 0
			}
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 592);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 20);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1080, 7408));
	});
}

#[test]
fn shrink_position_debit_for_lp_ausd_dot_work() {
	ExtBuilder::default().build().execute_with(|| {
		MockPriceSource::set_price(LP_AUSD_DOT, Some(Price::saturating_from_rational(1, 1)));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			10000,
			1000,
			0,
			false
		));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 20000);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CAROL),
			ALICE,
			LP_AUSD_DOT,
			1000
		));

		MockPriceSource::set_price(LP_AUSD_DOT, Some(Price::saturating_from_rational(1, 1)));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, LP_AUSD_DOT, 1000, 5000));
		assert_eq!(
			LoansModule::positions(LP_AUSD_DOT, ALICE),
			Position {
				collateral: 1000,
				debit: 5000
			}
		);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1000);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 10000));

		assert_noop!(
			CDPEngineModule::shrink_position_debit(&ALICE, LP_AUSD_DOT, 200, 200),
			Error::<Runtime>::NotEnoughDebitDecrement
		);

		assert_ok!(CDPEngineModule::shrink_position_debit(&ALICE, LP_AUSD_DOT, 100, 80));
		assert_eq!(
			LoansModule::positions(LP_AUSD_DOT, ALICE),
			Position {
				collateral: 900,
				debit: 4010
			}
		);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 900);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 9901));

		assert_ok!(CDPEngineModule::shrink_position_debit(&ALICE, LP_AUSD_DOT, 600, 500));
		assert_eq!(
			LoansModule::positions(LP_AUSD_DOT, ALICE),
			Position {
				collateral: 300,
				debit: 0
			}
		);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 685);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 300);
		assert_eq!(Currencies::free_balance(DOT, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(AUSD, &LoansModule::account_id()), 0);
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (1000, 9315));
	});
}

#[test]
fn remain_debit_value_too_small_check() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, BTC, 0, -490),
			crate::Error::<Runtime>::RemainDebitValueTooSmall
		);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, -90, -500));
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		setup_default_collateral(AUSD);
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));

		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			target_amount: 60,
		}));
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);

		mock_shutdown();
		assert_noop!(
			CDPEngineModule::liquidate(RuntimeOrigin::none(), BTC, ALICE),
			Error::<Runtime>::AlreadyShutdown
		);
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction_when_limited_by_slippage() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		setup_default_collateral(AUSD);
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		// pool is enough, but slippage limit the swap
		MockPriceSource::set_price(BTC, Some(Price::saturating_from_rational(2, 1)));
		assert_eq!(
			DEXModule::get_swap_amount(&[BTC, AUSD], SwapLimit::ExactTarget(Balance::MAX, 60)),
			Some((99, 60))
		);
		assert_eq!(
			DEXModule::get_swap_amount(&[BTC, AUSD], SwapLimit::ExactSupply(100, 0)),
			Some((100, 60))
		);
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			target_amount: 60,
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: BTC,
			owner: ALICE,
			collateral_amount: 100,
			bad_debt_value: 50,
			target_amount: 60,
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
fn liquidate_unsafe_cdp_of_lp_ausd_dot_and_swap_dot() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);

		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			10000,
			500,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (10000, 500));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 20000);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CAROL),
			ALICE,
			LP_AUSD_DOT,
			1000
		));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);

		MockPriceSource::set_price(DOT, Price::checked_from_rational(20, 1));
		MockPriceSource::set_price(LP_AUSD_DOT, Price::checked_from_rational(1, 1));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, LP_AUSD_DOT, 1000, 5000));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 5000);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 1000);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), None);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, LP_AUSD_DOT));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: LP_AUSD_DOT,
			owner: ALICE,
			collateral_amount: 1000,
			bad_debt_value: 500,
			target_amount: 600,
		}));

		assert_eq!(
			MockPriceSource::get_relative_price(AUSD, DOT),
			Price::checked_from_rational(1, 20)
		);
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (9400, 481));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 19000);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1019);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 500);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 600);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), None);
	});
}

#[test]
fn liquidate_unsafe_cdp_of_lp_ausd_dot_and_ausd_take_whole_target() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);

		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			10000,
			500,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (10000, 500));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 20000);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CAROL),
			ALICE,
			LP_AUSD_DOT,
			1000
		));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);

		MockPriceSource::set_price(DOT, Price::checked_from_rational(20, 1));
		MockPriceSource::set_price(LP_AUSD_DOT, Price::checked_from_rational(1, 1));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, LP_AUSD_DOT, 1000, 2000));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 200);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 2000);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 1000);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), None);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, LP_AUSD_DOT));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: LP_AUSD_DOT,
			owner: ALICE,
			collateral_amount: 1000,
			bad_debt_value: 200,
			target_amount: 240,
		}));

		assert_eq!(
			MockPriceSource::get_relative_price(AUSD, DOT),
			Price::checked_from_rational(1, 20)
		);
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (9500, 475));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 19000);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1025);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 460);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 200);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 240);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), None);
	});
}

#[test]
fn liquidate_unsafe_cdp_of_lp_ausd_dot_and_create_dot_auction() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(2, 1))),
			Change::NewValue(10000),
		));
		setup_default_collateral(DOT);
		setup_default_collateral(AUSD);

		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			AUSD,
			DOT,
			500,
			25,
			0,
			false
		));
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (500, 25));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 1000);
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(CAROL),
			ALICE,
			LP_AUSD_DOT,
			1000
		));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);

		MockPriceSource::set_price(DOT, Price::checked_from_rational(20, 1));
		MockPriceSource::set_price(LP_AUSD_DOT, Price::checked_from_rational(1, 1));

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, LP_AUSD_DOT, 1000, 5000));
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 5000);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 1000);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), None);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			LP_AUSD_DOT,
			Change::NoChange,
			Change::NewValue(Some(Ratio::max_value())),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));

		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, LP_AUSD_DOT));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::LiquidateUnsafeCDP {
			collateral_type: LP_AUSD_DOT,
			owner: ALICE,
			collateral_amount: 1000,
			bad_debt_value: 500,
			target_amount: 600,
		}));

		assert_eq!(
			MockPriceSource::get_relative_price(AUSD, DOT),
			Price::checked_from_rational(1, 20)
		);
		assert_eq!(DEXModule::get_liquidity_pool(AUSD, DOT), (0, 0));
		assert_eq!(Currencies::total_issuance(LP_AUSD_DOT), 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &ALICE), 0);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 500);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(LP_AUSD_DOT, ALICE).collateral, 0);
		assert_eq!(Currencies::free_balance(LP_AUSD_DOT, &LoansModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 500);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 500);
		assert_eq!(Currencies::free_balance(DOT, &CDPTreasuryModule::account_id()), 25);
		assert_eq!(
			Currencies::free_balance(LP_AUSD_DOT, &CDPTreasuryModule::account_id()),
			0
		);
		assert_eq!(MockAuctionManager::auction(), Some((ALICE, DOT, 25, 100)));
	});
}

#[test]
fn get_interest_rate_per_sec_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CDPEngineModule::get_interest_rate_per_sec(BTC),
			crate::Error::<Runtime>::InvalidCollateralType
		);
		assert_noop!(
			CDPEngineModule::get_interest_rate_per_sec(DOT),
			crate::Error::<Runtime>::InvalidCollateralType
		);

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(2, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_eq!(
			CDPEngineModule::get_interest_rate_per_sec(BTC),
			Ok(Rate::saturating_from_rational(2, 100000))
		);
		assert_noop!(
			CDPEngineModule::get_interest_rate_per_sec(DOT),
			crate::Error::<Runtime>::InvalidCollateralType
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::SettleCDPInDebit {
			collateral_type: BTC,
			owner: ALICE,
		}));
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 50);

		assert_noop!(
			CDPEngineModule::settle(RuntimeOrigin::none(), BTC, ALICE),
			Error::<Runtime>::MustAfterShutdown
		);
	});
}

#[test]
fn close_cdp_has_debit_by_dex_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
			SwapError::CannotSwap
		);

		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 1000));
		assert_ok!(CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 6));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::CloseCDPInDebitByDEX {
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
			RuntimeOrigin::signed(CAROL),
			BTC,
			ACA,
			100,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			ACA,
			AUSD,
			1000,
			1000,
			0,
			false
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_ok!(CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, 100));
		System::assert_last_event(RuntimeEvent::CDPEngineModule(crate::Event::CloseCDPInDebitByDEX {
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
		setup_default_collateral(BTC);
		setup_default_collateral(LP_AUSD_DOT);
		setup_default_collateral(DOT);

		let collateral_currencies_num = CollateralCurrencyIds::<Runtime>::get().len() as u64;

		System::set_block_number(1);

		// offchain worker will not liquidate alice
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 500));
		assert_ok!(CDPEngineModule::adjust_position(&BOB, BTC, 100, 100));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		// jump 2 blocks at a time because code rotates through the different supported collateral
		// currencies
		run_to_block_offchain(System::block_number() + collateral_currencies_num);

		// checks that offchain worker tx pool is empty (therefore tx to liquidate alice is not present)
		assert!(pool_state.write().transactions.pop().is_none());
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);

		// changes alice into unsafe position
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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
			assert_ok!(CDPEngineModule::liquidate(
				RuntimeOrigin::none(),
				currency_call,
				who_call
			));
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
			assert_ok!(CDPEngineModule::settle(RuntimeOrigin::none(), currency_call, who_call));
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
			RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::signed(ALICE),
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
			assert_ok!(CDPEngineModule::liquidate(
				RuntimeOrigin::none(),
				currency_call,
				who_call
			));
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
			assert_ok!(CDPEngineModule::liquidate(
				RuntimeOrigin::none(),
				currency_call,
				who_call
			));
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
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		System::set_block_number(1);

		// checks that max iterations is stored as none
		assert!(offchain
			.local_storage_get(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS)
			.is_none());

		for i in 0..1001u32 {
			let acount_id: AccountId = account_id_from_u32(i);
			assert_ok!(CDPEngineModule::adjust_position(&acount_id, BTC, 10, 50));
		}

		// make all positions unsafe
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
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

#[test]
fn minimal_collateral_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		// Check position fails if collateral is too small
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 9, 0, true),
			Error::<Runtime>::CollateralAmountBelowMinimum,
		);
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 9, 20, true));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 10, 0, true));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 0, 0, true));

		// Adjust position fails if collateral is too small
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, BTC, 9, 0),
			Error::<Runtime>::CollateralAmountBelowMinimum,
		);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 10, 0));

		// Cannot reduce collateral amount below the minimum.
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, BTC, -1, 0),
			Error::<Runtime>::CollateralAmountBelowMinimum,
		);

		// Allow the user to withdraw all assets
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 0));
	});
}

#[test]
fn register_liquidation_contract_works() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(CDPEngineModule::register_liquidation_contract(
			RuntimeOrigin::signed(ALICE),
			address,
		));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);
		System::assert_has_event(RuntimeEvent::CDPEngineModule(
			crate::Event::LiquidationContractRegistered { address },
		));
	});
}

#[test]
fn register_liquidation_contract_fails_if_not_update_origin() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CDPEngineModule::register_liquidation_contract(RuntimeOrigin::signed(BOB), address,),
			BadOrigin
		);
	});
}

#[test]
fn deregister_liquidation_contract_works() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(LiquidationContracts::<Runtime>::try_append(address));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);

		assert_ok!(CDPEngineModule::deregister_liquidation_contract(
			RuntimeOrigin::signed(ALICE),
			address,
		));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![],);
		System::assert_has_event(RuntimeEvent::CDPEngineModule(
			crate::Event::LiquidationContractDeregistered { address },
		));
	});
}

#[test]
fn deregister_liquidation_contract_fails_if_not_update_origin() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(LiquidationContracts::<Runtime>::try_append(address));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);

		assert_noop!(
			CDPEngineModule::deregister_liquidation_contract(RuntimeOrigin::signed(BOB), address,),
			BadOrigin
		);
	});
}

#[test]
fn liquidation_via_contracts_works() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Currencies::deposit(DOT, &CDPTreasuryModule::account_id(), 1000));
		assert_ok!(LiquidationContracts::<Runtime>::try_append(address));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);
		MockLiquidationEvmBridge::set_liquidation_result(Ok(()));

		assert_ok!(LiquidateViaContracts::<Runtime>::liquidate(&ALICE, DOT, 100, 1_000));
		let contract_account_id =
			<module_evm_accounts::EvmAddressMapping<Runtime> as AddressMapping<AccountId>>::get_account_id(&address);
		assert_eq!(Currencies::free_balance(DOT, &contract_account_id), 100);
	});
}

#[test]
fn liquidation_fails_if_no_liquidation_contracts() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Currencies::deposit(DOT, &CDPTreasuryModule::account_id(), 1000));
		MockLiquidationEvmBridge::set_liquidation_result(Ok(()));

		assert_noop!(
			LiquidateViaContracts::<Runtime>::liquidate(&ALICE, DOT, 100, 1_000),
			Error::<Runtime>::LiquidationFailed
		);
	});
}

#[test]
fn liquidation_fails_if_no_liquidation_contracts_can_liquidate() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Currencies::deposit(DOT, &CDPTreasuryModule::account_id(), 1000));
		assert_ok!(LiquidationContracts::<Runtime>::try_append(address));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);

		assert_err!(
			LiquidateViaContracts::<Runtime>::liquidate(&ALICE, DOT, 100, 1_000),
			Error::<Runtime>::LiquidationFailed
		);
	});
}

#[test]
fn liquidation_fails_if_insufficient_repayment() {
	let address = liquidation_contract_addr();
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Currencies::deposit(DOT, &CDPTreasuryModule::account_id(), 1000));
		assert_ok!(LiquidationContracts::<Runtime>::try_append(address));
		assert_eq!(CDPEngineModule::liquidation_contracts(), vec![address],);
		MockLiquidationEvmBridge::set_liquidation_result(Ok(()));
		MockLiquidationEvmBridge::set_repayment(1);

		assert_err!(
			LiquidateViaContracts::<Runtime>::liquidate(&ALICE, DOT, 100, 1_000),
			Error::<Runtime>::LiquidationFailed
		);
	});
}
