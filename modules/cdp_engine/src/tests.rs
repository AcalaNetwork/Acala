//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::OnFinalize};
use mock::{
	CDPEngineModule, CDPTreasuryModule, Currencies, DefaultDebitExchangeRate, DefaultLiquidationPenalty,
	DefaultLiquidationRatio, ExtBuilder, LoansModule, Origin, Runtime, System, TestEvent, ACA, ALICE, AUSD, BTC, DOT,
};
use orml_traits::MultiCurrency;
use sp_runtime::traits::BadOrigin;

#[test]
fn is_cdp_unsafe_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));
		assert_eq!(CDPEngineModule::is_cdp_unsafe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 50));
		assert_eq!(CDPEngineModule::is_cdp_unsafe(BTC, &ALICE), false);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 1))),
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(5, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(5, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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

		let update_global_stability_fee_event = TestEvent::cdp_engine(RawEvent::GlobalStabilityFeeUpdated(
			Rate::saturating_from_rational(1, 10000),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_global_stability_fee_event));

		assert_eq!(
			CDPEngineModule::global_stability_fee(),
			Rate::saturating_from_rational(1, 10000)
		);
	});
}

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			CDPEngineModule::set_collateral_params(
				Origin::signed(5),
				BTC,
				CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
				CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
				CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
				CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
				CollateralParamChange::New(10000),
			),
			BadOrigin
		);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));

		let update_stability_fee_event = TestEvent::cdp_engine(RawEvent::StabilityFeeUpdated(
			BTC,
			Some(Rate::saturating_from_rational(1, 100000)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_stability_fee_event));
		let update_liquidation_ratio_event = TestEvent::cdp_engine(RawEvent::LiquidationRatioUpdated(
			BTC,
			Some(Ratio::saturating_from_rational(3, 2)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_ratio_event));
		let update_liquidation_penalty_event = TestEvent::cdp_engine(RawEvent::LiquidationPenaltyUpdated(
			BTC,
			Some(Rate::saturating_from_rational(2, 10)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_penalty_event));
		let update_required_collateral_ratio_event = TestEvent::cdp_engine(RawEvent::RequiredCollateralRatioUpdated(
			BTC,
			Some(Ratio::saturating_from_rational(9, 5)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_required_collateral_ratio_event));
		let update_maximum_total_debit_value_event =
			TestEvent::cdp_engine(RawEvent::MaximumTotalDebitValueUpdated(BTC, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_maximum_total_debit_value_event));

		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));

		let new_collateral_params = CDPEngineModule::collateral_params(BTC);

		assert_eq!(
			new_collateral_params.stability_fee,
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
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(1, 1))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(10000),
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(10, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));
		assert_noop!(
			CDPEngineModule::adjust_position(&ALICE, ACA, 100, 50),
			Error::<Runtime>::InvalidCollateralType,
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 1))),
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
		));
		assert_ok!(CDPEngineModule::liquidate_unsafe_cdp(ALICE, BTC));

		let liquidate_unsafe_cdp_event = TestEvent::cdp_engine(RawEvent::LiquidateUnsafeCDP(
			BTC,
			ALICE,
			100,
			50,
			LiquidationStrategy::Auction,
		));
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
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			DOT,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 100))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
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
			Some(ExchangeRate::saturating_from_rational(101, 100))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		CDPEngineModule::on_finalize(3);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(10201, 10000))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 0, -30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		CDPEngineModule::on_finalize(4);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(10201, 10000))
		);
		assert_eq!(CDPEngineModule::debit_exchange_rate(DOT), None);
	});
}

#[test]
fn on_emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 30));
		CDPEngineModule::on_finalize(1);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 100))
		);
		assert_eq!(CDPEngineModule::is_shutdown(), false);
		CDPEngineModule::on_emergency_shutdown();
		assert_eq!(CDPEngineModule::is_shutdown(), true);
		CDPEngineModule::on_finalize(2);
		assert_eq!(
			CDPEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::saturating_from_rational(101, 100))
		);
	});
}

#[test]
fn settle_cdp_has_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			CollateralParamChange::New(Some(Rate::saturating_from_rational(1, 100000))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(3, 2))),
			CollateralParamChange::New(Some(Rate::saturating_from_rational(2, 10))),
			CollateralParamChange::New(Some(Ratio::saturating_from_rational(9, 5))),
			CollateralParamChange::New(10000),
		));
		assert_ok!(CDPEngineModule::adjust_position(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(LoansModule::debits(BTC, ALICE), 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CDPEngineModule::settle_cdp_has_debit(ALICE, BTC),
			Error::<Runtime>::NoDebitValue,
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
