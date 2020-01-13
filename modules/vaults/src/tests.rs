//! Unit tests for the vaults module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Currencies, ExtBuilder, Runtime, System, TestEvent, VaultsModule, ALICE, AUSD, X_TOKEN_ID, Y_TOKEN_ID};

#[test]
fn update_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(&ALICE, Y_TOKEN_ID, 100, 100));

		let update_position_event = TestEvent::vaults(RawEvent::UpdatePosition(ALICE, Y_TOKEN_ID, 100, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_position_event));

		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 100);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 100);
	});
}

#[test]
fn update_position_with_larger_than_collater_currency_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			VaultsModule::update_position(&ALICE, Y_TOKEN_ID, 100000, 100),
			Error::<Runtime>::CollateralInSufficient
		);
	});
}

#[test]
fn update_position_with_negative_collateral_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(&ALICE, Y_TOKEN_ID, 100, 100));
		// ensure collateral and debit
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 100);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 100);
		// ensure tokens
		assert_eq!(Currencies::balance(Y_TOKEN_ID, &ALICE), 900);
		assert_eq!(Currencies::balance(Y_TOKEN_ID, &VaultsModule::account_id()), 100);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);

		assert_ok!(VaultsModule::update_position(&ALICE, Y_TOKEN_ID, -10, -10));
		// ensure collateral and debit
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 90);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 90);
		// ensure tokens
		assert_eq!(Currencies::balance(Y_TOKEN_ID, &ALICE), 910);
		assert_eq!(Currencies::balance(Y_TOKEN_ID, &VaultsModule::account_id()), 90);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 45);
	});
}

#[test]
fn update_position_with_zero_collateral_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_position(&ALICE, Y_TOKEN_ID, 0, 0));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 0);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 0);
	});
}

#[test]
fn update_position_with_under_safe_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			VaultsModule::update_position(&ALICE, X_TOKEN_ID, 1, 1),
			Error::<Runtime>::RiskCheckFailed
		);
	});
}

#[test]
fn update_position_with_overflow_debits_cap_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			VaultsModule::update_position(&ALICE, X_TOKEN_ID, 100, 1000),
			Error::<Runtime>::ExceedDebitValueHardCap
		);
	});
}

#[test]
fn update_collaterals_and_debits_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_collaterals_and_debits(ALICE, Y_TOKEN_ID, 100, 100));

		let update_position_event_1 =
			TestEvent::vaults(RawEvent::UpdateCollateralsAndDebits(ALICE, Y_TOKEN_ID, 100, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_position_event_1));

		assert_ok!(VaultsModule::update_collaterals_and_debits(ALICE, Y_TOKEN_ID, -10, -10));

		let update_position_event_2 =
			TestEvent::vaults(RawEvent::UpdateCollateralsAndDebits(ALICE, Y_TOKEN_ID, -10, -10));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_position_event_2));

		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 90);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 90);
		// ensure tokens don't change
		assert_eq!(Currencies::balance(Y_TOKEN_ID, &ALICE), 1000);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 0);
	});
}

#[test]
fn update_collaterals_and_debits_with_zero_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(VaultsModule::update_collaterals_and_debits(ALICE, Y_TOKEN_ID, 0, 0));
		assert_eq!(VaultsModule::collaterals(ALICE, Y_TOKEN_ID), 0);
		assert_eq!(VaultsModule::debits(ALICE, Y_TOKEN_ID), 0);
	});
}
