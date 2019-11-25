//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use mock::{CdpEngineModule, DebitCurrency, ExtBuilder, Tokens, VaultsModule, ACA, ALICE, AUSD, BOB, BTC, DOT};
use palette_support::{assert_noop, assert_ok};

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_eq!(CdpEngineModule::stability_fee(BTC), Some(Permill::from_parts(1)));
		assert_eq!(
			CdpEngineModule::liquidation_ratio(BTC),
			Some(Ratio::from_rational(3, 2))
		);
		assert_eq!(
			CdpEngineModule::liquidation_penalty(BTC),
			Some(Permill::from_percent(20))
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
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_eq!(
			CdpEngineModule::calculate_collateral_ratio(BTC, 100, 50),
			Ratio::from_rational(100, 50)
		);
	});
}

#[test]
fn exceed_debit_value_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 9999), false);
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 10001), true);
	});
}

#[test]
fn check_position_adjustment_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_ok!(CdpEngineModule::check_position_adjustment(&ALICE, BTC, 100, 50));
	});
}

#[test]
fn check_position_adjustment_ratio_below_required_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_noop!(
			CdpEngineModule::check_position_adjustment(&ALICE, BTC, 89, 50),
			Error::BelowRequiredCollateralRatio
		);
	});
}

#[test]
fn check_debit_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_ok!(CdpEngineModule::check_debit_cap(BTC, 9999));
	});
}

#[test]
fn check_debit_cap_exceed() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_noop!(
			CdpEngineModule::check_debit_cap(BTC, 10001),
			Error::ExceedDebitValueHardCap,
		);
	});
}

#[test]
fn update_position_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_noop!(
			CdpEngineModule::update_position(ALICE, ACA, 100, 50),
			Error::NotValidCurrencyId,
		);
		assert_ok!(DebitCurrency::update_balance(BTC, &ALICE, -10));

		//assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 100, 50));
		// assert_eq!(Tokens::balance(BTC, &ALICE), 1000);
		// assert_eq!(VaultsModule::debits(ALICE, BTC), 0);
		// assert_eq!(VaultsModule::collaterals(ALICE, BTC), 0);
		// assert_ok!(CdpEngineModule::update_position(ALICE, BTC, 100, 50));
		// assert_eq!(VaultsModule::debits(ALICE, BTC), 50);
		// assert_eq!(VaultsModule::collaterals(ALICE, BTC), 100);
	});
}
