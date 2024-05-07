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

//! Unit tests for the loans module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeEvent, *};

#[test]
fn debits_key() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, 200, 200));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 200);
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 200);
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, -100, -100));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 100);
	});
}

#[test]
fn check_update_loan_underflow_work() {
	ExtBuilder::default().build().execute_with(|| {
		// collateral underflow
		assert_noop!(
			LoansModule::update_loan(&ALICE, BTC, -100, 0),
			ArithmeticError::Underflow,
		);

		// debit underflow
		assert_noop!(
			LoansModule::update_loan(&ALICE, BTC, 0, -100),
			ArithmeticError::Underflow,
		);
	});
}

#[test]
fn adjust_position_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);

		// balance too low
		assert_noop!(
			LoansModule::adjust_position(&ALICE, BTC, 2000, 0),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// mock can't pass liquidation ratio check
		assert_noop!(
			LoansModule::adjust_position(&ALICE, DOT, 500, 0),
			sp_runtime::DispatchError::Other("mock below liquidation ratio error")
		);

		// mock can't pass required ratio check
		assert_noop!(
			LoansModule::adjust_position(&ALICE, DOT, 500, 1),
			sp_runtime::DispatchError::Other("mock below required collateral ratio error")
		);

		// mock exceed debit value cap
		assert_noop!(
			LoansModule::adjust_position(&ALICE, BTC, 1000, 1000),
			sp_runtime::DispatchError::Other("mock exceed debit value cap error")
		);

		// failed because ED of collateral
		assert_noop!(
			LoansModule::adjust_position(&ALICE, BTC, 99, 0),
			orml_tokens::Error::<Runtime>::ExistentialDeposit,
		);

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
		System::assert_has_event(RuntimeEvent::LoansModule(crate::Event::PositionUpdated {
			owner: ALICE,
			collateral_type: BTC,
			collateral_adjustment: 500,
			debit_adjustment: 300,
		}));

		// collateral_adjustment is negatives
		assert_eq!(Currencies::total_balance(BTC, &LoansModule::account_id()), 500);
		assert_ok!(LoansModule::adjust_position(&ALICE, BTC, -500, 0));
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
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
		assert!(!<Positions<Runtime>>::contains_key(BTC, &ALICE));

		let alice_ref_count_0 = System::consumers(&ALICE);

		assert_ok!(LoansModule::update_loan(&ALICE, BTC, 3000, 2000));

		// just update records
		assert_eq!(LoansModule::total_positions(BTC).debit, 2000);
		assert_eq!(LoansModule::total_positions(BTC).collateral, 3000);
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 2000);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 3000);

		// increase ref count when open new position
		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 + 1);

		// dot not manipulate balance
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);

		// should remove position storage if zero
		assert!(<Positions<Runtime>>::contains_key(BTC, &ALICE));
		assert_ok!(LoansModule::update_loan(&ALICE, BTC, -3000, -2000));
		assert_eq!(LoansModule::positions(BTC, &ALICE).debit, 0);
		assert_eq!(LoansModule::positions(BTC, &ALICE).collateral, 0);
		assert!(!<Positions<Runtime>>::contains_key(BTC, &ALICE));

		// decrease ref count after remove position
		let alice_ref_count_2 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_2, alice_ref_count_1 - 1);
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
		System::assert_last_event(RuntimeEvent::LoansModule(crate::Event::TransferLoan {
			from: ALICE,
			to: BOB,
			currency_id: BTC,
		}));
	});
}

#[test]
fn confiscate_collateral_and_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(LoansModule::update_loan(&BOB, BTC, 5000, 1000));
		assert_eq!(Currencies::free_balance(BTC, &LoansModule::account_id()), 0);

		// have no sufficient balance
		assert_noop!(
			LoansModule::confiscate_collateral_and_debit(&BOB, BTC, 5000, 1000),
			orml_tokens::Error::<Runtime>::BalanceTooLow
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
		System::assert_last_event(RuntimeEvent::LoansModule(crate::Event::ConfiscateCollateralAndDebit {
			owner: ALICE,
			collateral_type: BTC,
			confiscated_collateral_amount: 300,
			deduct_debit_amount: 200,
		}));
	});
}

#[test]
fn loan_updated_updated_when_adjust_collateral() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(DOT_SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 0);

		assert_ok!(LoansModule::update_loan(&BOB, DOT, 1000, 0));
		assert_eq!(DOT_SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 1000);

		assert_ok!(LoansModule::update_loan(&BOB, DOT, 0, 200));
		assert_eq!(DOT_SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 1000);

		assert_ok!(LoansModule::update_loan(&BOB, DOT, -800, 500));
		assert_eq!(DOT_SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 200);
	});
}
