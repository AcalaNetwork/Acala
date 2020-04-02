//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CDPEngineModule, CDPTreasuryModule, Currencies, DefaultDebitExchangeRate, DefaultLiquidationPenalty,
	DefaultLiquidationRatio, ExtBuilder, LoansModule, Origin, Runtime, System, TestEvent, ACA, ALICE, AUSD, BTC, DOT,
};
use sp_runtime::traits::{BadOrigin, OnFinalize};

#[test]
fn is_cdp_unsafe_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CDPEngineModule::is_cdp_unsafe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(CDPEngineModule::is_cdp_unsafe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_eq!(CDPEngineModule::is_cdp_unsafe(BTC, &ALICE), true);
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
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(5, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CDPEngineModule::get_liquidation_penalty(BTC),
			Rate::from_rational(2, 10)
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
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(5, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CDPEngineModule::get_liquidation_ratio(BTC), Ratio::from_rational(5, 2));
	});
}

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CDPEngineModule::set_collateral_params(
				Origin::signed(5),
				BTC,
				Some(Some(Rate::from_rational(1, 100000))),
				Some(Some(Ratio::from_rational(3, 2))),
				Some(Some(Rate::from_rational(2, 10))),
				Some(Some(Ratio::from_rational(9, 5))),
				Some(10000),
			),
			BadOrigin
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));

		let update_stability_fee_event =
			TestEvent::cdp_engine(RawEvent::UpdateStabilityFee(BTC, Some(Rate::from_rational(1, 100000))));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_stability_fee_event));
		let update_liquidation_ratio_event =
			TestEvent::cdp_engine(RawEvent::UpdateLiquidationRatio(BTC, Some(Ratio::from_rational(3, 2))));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_ratio_event));
		let update_liquidation_penalty_event = TestEvent::cdp_engine(RawEvent::UpdateLiquidationPenalty(
			BTC,
			Some(Rate::from_rational(2, 10)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_penalty_event));
		let update_required_collateral_ratio_event = TestEvent::cdp_engine(RawEvent::UpdateRequiredCollateralRatio(
			BTC,
			Some(Ratio::from_rational(9, 5)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_required_collateral_ratio_event));
		let update_maximum_total_debit_value_event =
			TestEvent::cdp_engine(RawEvent::UpdateMaximumTotalDebitValue(BTC, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_maximum_total_debit_value_event));

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CDPEngineModule::stability_fee(BTC),
			Some(Rate::from_rational(1, 100000))
		);
		assert_eq!(
			CDPEngineModule::liquidation_ratio(BTC),
			Some(Ratio::from_rational(3, 2))
		);
		assert_eq!(
			CDPEngineModule::liquidation_penalty(BTC),
			Some(Rate::from_rational(2, 10))
		);
		assert_eq!(
			CDPEngineModule::required_collateral_ratio(BTC),
			Some(Ratio::from_rational(9, 5))
		);
		assert_eq!(CDPEngineModule::maximum_total_debit_value(BTC), 10000);
	});
}

#[test]
fn calculate_collateral_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CDPEngineModule::calculate_collateral_ratio(BTC, 100, 50, Price::from_rational(1, 1)),
			Ratio::from_rational(100, 50)
		);
	});
}

#[test]
fn check_debit_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
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
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(1, 1))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::check_position_valid(BTC, 100, 50));
	});
}

#[test]
fn check_position_valid_ratio_below_liquidate_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(10, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
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
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
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
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, ACA, 100, 50),
			Error::<Runtime>::InvalidCurrencyId,
		);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::debits(BTC, ALICE), 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 0);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE), 50);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_eq!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 20).is_ok(), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -20));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		assert_eq!(LoansModule::debits(BTC, ALICE), 30);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
	});
}

#[test]
fn remain_debit_value_too_small_check() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -49).is_ok(), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, -100, -50));
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE), 50);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::<Runtime>::MustBeUnsafe,
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));

		let liquidate_unsafe_cdp_event = TestEvent::cdp_engine(RawEvent::LiquidateUnsafeCDP(BTC, ALICE, 100, 50));
		assert!(System::events()
			.iter()
			.any(|record| record.event == liquidate_unsafe_cdp_event));

		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE), 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 0);
	});
}

#[test]
fn on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			DOT,
			Some(Some(Rate::from_rational(2, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		CDPEngineModule::on_finalize(1);
		assert_eq!(CDPEngineModule::debit_exchange_rate(BTC), None);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		CDPEngineModule::on_finalize(2);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		CDPEngineModule::on_finalize(3);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(10201, 10000))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		CDPEngineModule::on_finalize(4);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(10201, 10000))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
	});
}

#[test]
fn emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 30));
		CDPEngineModule::on_finalize(1);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
		assert_eq!(CDPEngineModule::is_shutdown(), false);
		CDPEngineModule::emergency_shutdown();
		assert_eq!(CDPEngineModule::is_shutdown(), true);
		CDPEngineModule::on_finalize(2);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
	});
}

#[test]
fn settle_cdp_has_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(LoansModule::debits(BTC, ALICE), 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CDPEngineModule::settle_cdp_has_debit(ALICE, BTC),
			Error::<Runtime>::AlreadyNoDebit,
		);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, 50));
		assert_eq!(LoansModule::debits(BTC, ALICE), 50);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_ok!(CDPEngineModule::settle_cdp_has_debit(ALICE, BTC));

		let settle_cdp_in_debit_event = TestEvent::cdp_engine(RawEvent::SettleCDPInDebit(BTC, ALICE));
		assert!(System::events()
			.iter()
			.any(|record| record.event == settle_cdp_in_debit_event));

		assert_eq!(LoansModule::debits(BTC, ALICE), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 50);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 50);
	});
}
