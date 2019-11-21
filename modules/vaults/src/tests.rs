//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use mock::{ExtBuilder, VaultsModule, ALICE, STABLE_COIN_ID, X_TOKEN_ID, Y_TOKEN_ID};
use palette_support::{assert_noop, assert_ok};

#[test]
fn update_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 100, 100));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 100);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 100);
	});
}

#[test]
fn update_position_larger_than_collater_currency_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			VaultsModule::update_position(ALICE, Y_TOKEN_ID, 100000, 100),
			Error::UpdateCollateralFailed
		);
	});
}

#[test]
fn update_position_negative_collateral_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 100, 100));
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, -10, -10));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 90);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 90);
	});
}

#[test]
fn update_position_zero_collateral_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 0, 0));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 0);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 0);
	});
}

#[test]
fn update_position_under_safe_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			VaultsModule::update_position(ALICE, X_TOKEN_ID, 1, 1),
			Error::PositionWillUnsafe
		);
	});
}

#[test]
fn update_collateral_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_collaterals_and_debits(ALICE, X_TOKEN_ID, 1, 1));
	});
}

#[test]
fn update_collateral_negative_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 100, 100));
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, -10, -10));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 90);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 90);
	});
}

#[test]
fn update_collateral_zero_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 0, 0));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 0);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 0);
	});
}

#[test]
fn get_collateral_rate_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(ALICE, Y_TOKEN_ID, 100, 100));
		assert_eq!(
			VaultsModule::collateral_ratio(ALICE, Y_TOKEN_ID),
			Some(Fixed64::from_rational(1, 0))
		);
	});
}
