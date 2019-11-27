//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{CdpEngineModule, Currencies, ExtBuilder, Origin, VaultsModule, ACA, ALICE, AUSD, BTC, DOT};
use sr_primitives::traits::OnFinalize;

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CdpEngineModule::stability_fee(BTC),
			Some(Rate::from_rational(1, 100000))
		);
		assert_eq!(
			CdpEngineModule::liquidation_ratio(BTC),
			Some(Ratio::from_rational(3, 2))
		);
		assert_eq!(
			CdpEngineModule::liquidation_penalty(BTC),
			Some(Rate::from_rational(2, 10))
		);
		assert_eq!(
			CdpEngineModule::required_collateral_ratio(BTC),
			Some(Ratio::from_rational(9, 5))
		);
		assert_eq!(CdpEngineModule::maximum_total_debit_value(BTC), 10000);
	});
}

#[test]
fn calculate_collateral_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CdpEngineModule::calculate_collateral_ratio(BTC, 100, 50),
			Ratio::from_rational(100, 50)
		);
	});
}

#[test]
fn exceed_debit_value_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 9999), false);
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 10001), true);
	});
}

#[test]
fn check_position_adjustment_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::check_position_adjustment(&ALICE, BTC, 100, 50));
	});
}

#[test]
fn check_position_adjustment_ratio_below_required_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::check_position_adjustment(&ALICE, BTC, 89, 50),
			Error::BelowRequiredCollateralRatio
		);
	});
}

#[test]
fn check_debit_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::check_debit_cap(BTC, 9999));
	});
}

#[test]
fn check_debit_cap_exceed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::check_debit_cap(BTC, 10001),
			Error::ExceedDebitValueHardCap,
		);
	});
}

#[test]
fn update_position_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::update_position(ALICE, ACA, 100, 50),
			Error::NotValidCurrencyId,
		);
		assert_eq!(Currencies::balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 0);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 0);
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 0);
		assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 100, 50));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 50);
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CdpEngineModule::update_position(ALICE, BTC, 0, 20),
			Error::UpdatePositionFailed,
		);
		assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 0, -20));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 30);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 30);
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 100);
	});
}

#[test]
fn remain_debit_value_too_small_check() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 100, 50));
		assert_noop!(
			CdpEngineModule::update_position(ALICE, BTC, 0, -49),
			Error::UpdatePositionFailed,
		);
		assert_ok!(CdpEngineModule::update_position(ALICE, BTC, -100, -50));
	});
}

#[test]
fn liquidate_unsafe_cdp_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 100, 50));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 50);
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::CollateralRatioStillSafe,
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 0);
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 0);
	});
}

#[test]
fn on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			DOT,
			Some(Some(Rate::from_rational(2, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		CdpEngineModule::on_finalize(1);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(DOT),
			Some(ExchangeRate::from_rational(102, 100))
		);
		CdpEngineModule::on_finalize(2);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(10201, 10000))
		);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(DOT),
			Some(ExchangeRate::from_rational(10404, 10000))
		);
	});
}
