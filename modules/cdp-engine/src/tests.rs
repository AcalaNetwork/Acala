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
use mock::{Event, *};
use orml_traits::MultiCurrency;
use sp_runtime::traits::BadOrigin;

#[test]
fn is_cdp_unsafe_work() {
	fn is_user_safe(currency_id: CurrencyId, who: &AccountId) -> bool {
		let Position { collateral, debit } = LoansModule::positions(currency_id, &who);
		CDPEngineModule::is_cdp_unsafe(currency_id, collateral, debit)
	}

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
		assert_eq!(is_user_safe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(is_user_safe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		));
		assert_eq!(is_user_safe(BTC, &ALICE), true);
	});
}

#[test]
fn get_debit_exchange_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CDPEngineModule::get_debit_exchange_rate(BTC),
			DefaultDebitExchangeRate::get()
		);
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
		System::assert_last_event(Event::CDPEngineModule(crate::Event::GlobalInterestRatePerSecUpdated(
			Rate::saturating_from_rational(1, 10000),
		)));
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
		System::assert_has_event(Event::CDPEngineModule(crate::Event::InterestRatePerSec(
			BTC,
			Some(Rate::saturating_from_rational(1, 100000)),
		)));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::LiquidationRatioUpdated(
			BTC,
			Some(Ratio::saturating_from_rational(3, 2)),
		)));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::LiquidationPenaltyUpdated(
			BTC,
			Some(Rate::saturating_from_rational(2, 10)),
		)));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::RequiredCollateralRatioUpdated(
			BTC,
			Some(Ratio::saturating_from_rational(9, 5)),
		)));
		System::assert_has_event(Event::CDPEngineModule(crate::Event::MaximumTotalDebitValueUpdated(
			BTC, 10000,
		)));

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
			CDPEngineModule::calculate_collateral_ratio(BTC, 100, 50, Price::saturating_from_rational(1, 1)),
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
		assert_ok!(CDPEngineModule::check_debit_cap(BTC, 9999));
		assert_noop!(
			CDPEngineModule::check_debit_cap(BTC, 10001),
			Error::<Runtime>::ExceedDebitValueHardCap,
		);
	});
}

#[test]
fn check_position_valid_work() {
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
			CDPEngineModule::check_position_valid(BTC, 100, 50),
			Error::<Runtime>::InvalidFeedPrice
		);
		MockPriceSource::set_relative_price(Some(Price::one()));

		assert_ok!(CDPEngineModule::check_position_valid(BTC, 100, 50));
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
			CDPEngineModule::check_position_valid(BTC, 2, 1),
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
			CDPEngineModule::check_position_valid(BTC, 91, 50),
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
		assert_noop!(
			CDPEngineModule::check_position_valid(BTC, 89, 50),
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
			CDPEngineModule::adjust_position(&ALICE, ACA, 100, 50),
			Error::<Runtime>::InvalidCollateralType,
		);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 20).is_ok(), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -20));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 30);
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
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -49).is_ok(), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, -100, -50));
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
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);
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
		System::assert_last_event(Event::CDPEngineModule(crate::Event::LiquidateUnsafeCDP(
			BTC,
			ALICE,
			100,
			50,
			LiquidationStrategy::Auction,
		)));
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
		assert_eq!(CDPEngineModule::debit_exchange_rate(BTC), None);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 30));

		CDPEngineModule::accumulate_interest(2, 1);
		assert_eq!(CDPEngineModule::last_accumulation_secs(), 2);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 100))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);

		mock_shutdown();
		assert_eq!(<Runtime as Config>::EmergencyShutdown::is_shutdown(), true);

		CDPEngineModule::accumulate_interest(3, 2);
		assert_eq!(CDPEngineModule::last_accumulation_secs(), 3);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 100))
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
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 50));
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_ok!(CDPEngineModule::settle_cdp_has_debit(ALICE, BTC));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::SettleCDPInDebit(BTC, ALICE)));
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
			CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, None),
			Error::<Runtime>::NoDebitValue
		);

		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);
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
			CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, None),
			Error::<Runtime>::IsUnsafe
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
		assert_ok!(CDPEngineModule::close_cdp_has_debit_by_dex(ALICE, BTC, None));
		System::assert_last_event(Event::CDPEngineModule(crate::Event::CloseCDPInDebitByDEX(
			BTC, ALICE, 6, 94, 50,
		)));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 994);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 50);
		assert_eq!(CDPTreasuryModule::get_debit_pool(), 50);
	});
}
