//! Unit tests for the loans module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CDPTreasuryModule, Currencies, ExtBuilder, LoansModule, Runtime, System, TestEvent, ALICE, AUSD, BOB, X_TOKEN_ID,
	Y_TOKEN_ID,
};

#[test]
fn debits_key() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, ALICE), (0, None));
		assert_ok!(LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, 100, 100));
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, ALICE), (100, Some((Y_TOKEN_ID, ALICE))));
		assert_ok!(LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, -100, -100));
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, ALICE), (0, None));
	});
}

#[test]
fn check_update_loan_overflow_work() {
	ExtBuilder::default().build().execute_with(|| {
		// collateral underflow
		assert_noop!(
			LoansModule::check_update_loan_overflow(&ALICE, Y_TOKEN_ID, -100, 0),
			Error::<Runtime>::CollateralTooLow,
		);

		// debit underflow
		assert_noop!(
			LoansModule::check_update_loan_overflow(&ALICE, Y_TOKEN_ID, 0, -100),
			Error::<Runtime>::DebitTooLow,
		);
	});
}

#[test]
fn adjust_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &ALICE), 1000);

		// balance too low
		assert_eq!(LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, 2000, 0).is_ok(), false);

		// mock can't pass position valid check
		assert_eq!(LoansModule::adjust_position(&ALICE, X_TOKEN_ID, 500, 0).is_ok(), false);

		// mock exceed debit value cap
		assert_eq!(
			LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, 1000, 1000).is_ok(),
			false
		);

		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &LoansModule::account_id()), 0);
		assert_eq!(LoansModule::total_debits(Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::total_collaterals(Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 0);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);

		// success
		assert_ok!(LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, 500, 300));
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &ALICE), 500);
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &LoansModule::account_id()), 500);
		assert_eq!(LoansModule::total_debits(Y_TOKEN_ID), 300);
		assert_eq!(LoansModule::total_collaterals(Y_TOKEN_ID), 500);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 300);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 500);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 150);

		let update_position_event = TestEvent::loans(RawEvent::UpdatePosition(ALICE, Y_TOKEN_ID, 500, 300));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_position_event));
	});
}

#[test]
fn update_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &ALICE), 1000);
		assert_eq!(LoansModule::total_debits(Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::total_collaterals(Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 0);

		assert_ok!(LoansModule::update_loan(&ALICE, Y_TOKEN_ID, 3000, 2000));

		// just update records
		assert_eq!(LoansModule::total_debits(Y_TOKEN_ID), 2000);
		assert_eq!(LoansModule::total_collaterals(Y_TOKEN_ID), 3000);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 2000);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 3000);

		// dot not manipulate balance
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &ALICE), 1000);
	});
}

#[test]
fn transfer_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(LoansModule::update_loan(&ALICE, Y_TOKEN_ID, 400, 500));
		assert_ok!(LoansModule::update_loan(&BOB, Y_TOKEN_ID, 100, 600));
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 500);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 400);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &BOB).0, 600);
		assert_eq!(LoansModule::collaterals(&BOB, Y_TOKEN_ID), 100);

		assert_ok!(LoansModule::transfer_loan(&ALICE, &BOB, Y_TOKEN_ID));
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &BOB).0, 1100);
		assert_eq!(LoansModule::collaterals(&BOB, Y_TOKEN_ID), 500);

		let transfer_loan_event = TestEvent::loans(RawEvent::TransferLoan(ALICE, BOB, Y_TOKEN_ID));
		assert!(System::events()
			.iter()
			.any(|record| record.event == transfer_loan_event));
	});
}

#[test]
fn confiscate_collateral_and_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(LoansModule::update_loan(&BOB, Y_TOKEN_ID, 5000, 1000));
		assert_eq!(Currencies::free_balance(Y_TOKEN_ID, &LoansModule::account_id()), 0);

		// have no sufficient balance
		assert_eq!(
			LoansModule::confiscate_collateral_and_debit(&BOB, Y_TOKEN_ID, 5000, 1000).is_ok(),
			false,
		);

		assert_ok!(LoansModule::adjust_position(&ALICE, Y_TOKEN_ID, 500, 300));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(Y_TOKEN_ID), 0);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 300);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 500);

		assert_ok!(LoansModule::confiscate_collateral_and_debit(
			&ALICE, Y_TOKEN_ID, 300, 200
		));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(Y_TOKEN_ID), 300);
		assert_eq!(LoansModule::debits(Y_TOKEN_ID, &ALICE).0, 100);
		assert_eq!(LoansModule::collaterals(&ALICE, Y_TOKEN_ID), 200);

		let confiscate_event = TestEvent::loans(RawEvent::ConfiscateCollateralAndDebit(ALICE, Y_TOKEN_ID, 300, 200));
		assert!(System::events().iter().any(|record| record.event == confiscate_event));
	});
}
