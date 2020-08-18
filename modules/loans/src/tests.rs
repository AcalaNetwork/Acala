//! Unit tests for the loans module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CDPTreasuryModule, Currencies, ExtBuilder, LoansModule, Runtime, System, TestEvent, ALICE, AUSD, BOB, BTC, DOT,
};

#[test]
fn debits_key() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, 100, 100));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 100);
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, -100, -100));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
	});
}

#[test]
fn check_update_loan_overflow_work() {
	ExtBuilder::default().build().execute_with(|| {
		// collateral underflow
		assert_noop!(
			LoansModule::update_loan(&ALICE, BTC, -100, 0),
			Error::<Runtime>::CollateralTooLow,
		);

		// debit underflow
		assert_noop!(
			LoansModule::update_loan(&ALICE, BTC, 0, -100),
			Error::<Runtime>::DebitTooLow,
		);
	});
}

#[test]
fn adjust_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);

		// balance too low
		assert_eq!(LoansModule::adjust_position(&ALICE, BTC, 2000, 0).is_ok(), false);

		// mock can't pass position valid check
		assert_eq!(LoansModule::adjust_position(&ALICE, DOT, 500, 0).is_ok(), false);

		// mock exceed debit value cap
		assert_eq!(LoansModule::adjust_position(&ALICE, BTC, 1000, 1000).is_ok(), false);

		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
		assert_eq!(LoansModule::total_positions(BTC).debit, 0);
		assert_eq!(LoansModule::total_positions(BTC).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 0);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);

		// success
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, 500, 300));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 500);
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 500);
		assert_eq!(LoansModule::total_positions(BTC).debit, 300);
		assert_eq!(LoansModule::total_positions(BTC).collateral, 500);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 300);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 500);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 150);

		let update_position_event = TestEvent::loans(RawEvent::PositionUpdated(ALICE, BTC, 500, 300));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_position_event));
	});
}

#[test]
fn update_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(LoansModule::total_positions(BTC).debit, 0);
		assert_eq!(LoansModule::total_positions(BTC).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 0);
		assert_eq!(<Positions<Runtime>>::contains_key(BTC, &ALICE), false);
		assert_eq!(System::refs(&ALICE), 0);

		assert_ok!(LoansModule::update_loan(&ALICE, BTC, 3000, 2000));

		// just update records
		assert_eq!(LoansModule::total_positions(BTC).debit, 2000);
		assert_eq!(LoansModule::total_positions(BTC).collateral, 3000);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 2000);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 3000);

		// increase ref count when open new position
		assert_eq!(System::refs(&ALICE), 1);

		// dot not manipulate balance
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);

		// should remove position storage if zero
		assert_eq!(<Positions<Runtime>>::contains_key(BTC, &ALICE), true);
		assert_ok!(LoansModule::update_loan(&ALICE, BTC, -3000, -2000));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 0);
		assert_eq!(<Positions<Runtime>>::contains_key(BTC, &ALICE), false);

		// decrease ref count after remove position
		assert_eq!(System::refs(&ALICE), 0);
	});
}

#[test]
fn transfer_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(LoansModule::update_loan(&ALICE, BTC, 400, 500));
		assert_ok!(LoansModule::update_loan(&BOB, BTC, 100, 600));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 500);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 400);
		assert_eq!(LoansModule::positions(BTC, &BOB).debit, 600);
		assert_eq!(LoansModule::positions(BTC, &BOB).collateral, 100);

		assert_ok!(LoansModule::transfer_loan(&ALICE, &BOB, BTC));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, &BOB).debit, 1100);
		assert_eq!(LoansModule::positions(BTC, &BOB).collateral, 500);

		let transfer_loan_event = TestEvent::loans(RawEvent::TransferLoan(ALICE, BOB, BTC));
		assert!(System::events()
			.iter()
			.any(|record| record.event == transfer_loan_event));
	});
}

#[test]
fn confiscate_collateral_and_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(LoansModule::update_loan(&BOB, BTC, 5000, 1000));
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);

		// have no sufficient balance
		assert_eq!(
			LoansModule::confiscate_collateral_and_debit(&BOB, BTC, 5000, 1000).is_ok(),
			false,
		);

		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, 500, 300));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(BTC), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 300);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 500);

		assert_ok!(LoansModule::confiscate_collateral_and_debit(&ALICE, BTC, 300, 200));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(BTC), 300);
		assert_eq!(CDPTreasuryModule::debit_pool(), 100);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 100);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 200);

		let confiscate_event = TestEvent::loans(RawEvent::ConfiscateCollateralAndDebit(ALICE, BTC, 300, 200));
		assert!(System::events().iter().any(|record| record.event == confiscate_event));
	});
}
