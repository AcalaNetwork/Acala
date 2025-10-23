// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

//! Unit tests for the honzon module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeEvent, *};
use module_support::{Rate, Ratio};
use orml_traits::{Change, MultiCurrency};
use sp_runtime::FixedPointNumber;

#[test]
fn authorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(PalletBalances::reserved_balance(ALICE), 0);
		assert_ok!(HonzonModule::authorize(RuntimeOrigin::signed(ALICE), BTC, BOB));
		assert_eq!(
			PalletBalances::reserved_balance(ALICE),
			<<Runtime as Config>::DepositPerAuthorization as sp_runtime::traits::Get<u128>>::get()
		);
		System::assert_last_event(RuntimeEvent::HonzonModule(crate::Event::Authorization {
			authorizer: ALICE,
			authorizee: BOB,
			collateral_type: BTC,
		}));
		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));
		assert_noop!(
			HonzonModule::authorize(RuntimeOrigin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::AlreadyAuthorized
		);
	});
}

#[test]
fn unauthorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(HonzonModule::authorize(RuntimeOrigin::signed(ALICE), BTC, BOB));
		assert_eq!(
			PalletBalances::reserved_balance(ALICE),
			<<Runtime as Config>::DepositPerAuthorization as sp_runtime::traits::Get<u128>>::get()
		);
		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));

		assert_ok!(HonzonModule::unauthorize(RuntimeOrigin::signed(ALICE), BTC, BOB));
		assert_eq!(PalletBalances::reserved_balance(ALICE), 0);
		System::assert_last_event(RuntimeEvent::HonzonModule(crate::Event::UnAuthorization {
			authorizer: ALICE,
			authorizee: BOB,
			collateral_type: BTC,
		}));
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::<Runtime>::NoPermission
		);
		assert_noop!(
			HonzonModule::unauthorize(RuntimeOrigin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::AuthorizationNotExists
		);
	});
}

#[test]
fn unauthorize_all_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(HonzonModule::authorize(RuntimeOrigin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::authorize(RuntimeOrigin::signed(ALICE), DOT, CAROL));
		assert_eq!(PalletBalances::reserved_balance(ALICE), 200);
		assert_ok!(HonzonModule::unauthorize_all(RuntimeOrigin::signed(ALICE)));
		assert_eq!(PalletBalances::reserved_balance(ALICE), 0);
		System::assert_last_event(RuntimeEvent::HonzonModule(crate::Event::UnAuthorizationAll {
			authorizer: ALICE,
		}));

		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::<Runtime>::NoPermission
		);
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, DOT),
			Error::<Runtime>::NoPermission
		);
	});
}

#[test]
fn transfer_loan_from_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 50));
		assert_ok!(HonzonModule::authorize(RuntimeOrigin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::transfer_loan_from(RuntimeOrigin::signed(BOB), BTC, ALICE));
		assert_eq!(LoansModule::positions(BTC, BOB).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, BOB).debit, 50);
	});
}

#[test]
fn transfer_unauthorization_loans_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HonzonModule::transfer_loan_from(RuntimeOrigin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::NoPermission,
		);
	});
}

#[test]
fn adjust_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 50));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);
	});
}

#[test]
fn adjust_loan_by_debit_value_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		assert_ok!(HonzonModule::adjust_loan_by_debit_value(
			RuntimeOrigin::signed(ALICE),
			BTC,
			100,
			50
		));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);

		assert_ok!(HonzonModule::adjust_loan_by_debit_value(
			RuntimeOrigin::signed(ALICE),
			BTC,
			-10,
			-5
		));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 90);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 450);
	});
}

#[test]
fn on_emergency_shutdown_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		mock_shutdown();
		assert_noop!(
			HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 50),
			Error::<Runtime>::AlreadyShutdown,
		);
		assert_noop!(
			HonzonModule::transfer_loan_from(RuntimeOrigin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::AlreadyShutdown,
		);
		assert_noop!(
			HonzonModule::close_loan_has_debit_by_dex(RuntimeOrigin::signed(ALICE), BTC, 100),
			Error::<Runtime>::AlreadyShutdown,
		);
	});
}

#[test]
fn close_loan_has_debit_by_dex_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 50));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 50);

		assert_ok!(HonzonModule::close_loan_has_debit_by_dex(
			RuntimeOrigin::signed(ALICE),
			BTC,
			100,
		));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 0);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 0);
	});
}

#[test]
fn transfer_debit_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		// set up two loans
		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 500));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);

		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), DOT, 100, 500));
		assert_eq!(LoansModule::positions(DOT, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(DOT, ALICE).debit, 500);

		// Will not work for account with no open CDP
		assert_noop!(
			HonzonModule::transfer_debit(RuntimeOrigin::signed(BOB), BTC, DOT, 1000),
			ArithmeticError::Underflow
		);
		// Won't work when transferring more debit than is present
		assert_noop!(
			HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, DOT, 10_000),
			ArithmeticError::Underflow
		);
		// Below minimum collateral threshold for the BTC CDP
		assert_noop!(
			HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, DOT, 500),
			module_cdp_engine::Error::<Runtime>::BelowRequiredCollateralRatio
		);
		// Too large of a transfer
		assert_noop!(
			HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, DOT, u128::MAX),
			ArithmeticError::Overflow
		);
		// Won't work for currency that is not collateral
		assert_noop!(
			HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, ACA, 50),
			module_cdp_engine::Error::<Runtime>::InvalidCollateralType
		);

		assert_ok!(HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, DOT, 50));
		System::assert_last_event(RuntimeEvent::HonzonModule(crate::Event::<Runtime>::TransferDebit {
			from_currency: BTC,
			to_currency: DOT,
			amount: 50,
		}));

		assert_eq!(LoansModule::positions(DOT, ALICE).debit, 550);
		assert_eq!(LoansModule::positions(DOT, ALICE).collateral, 100);

		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 450);
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
	});
}

#[test]
fn transfer_debit_no_ausd() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			BTC,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));
		assert_ok!(CDPEngineModule::set_collateral_params(
			RuntimeOrigin::signed(ALICE),
			DOT,
			Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
			Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
			Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
			Change::NewValue(10000),
		));

		// set up two loans
		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), BTC, 100, 500));
		assert_eq!(LoansModule::positions(BTC, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(BTC, ALICE).debit, 500);

		assert_ok!(HonzonModule::adjust_loan(RuntimeOrigin::signed(ALICE), DOT, 100, 500));
		assert_eq!(LoansModule::positions(DOT, ALICE).collateral, 100);
		assert_eq!(LoansModule::positions(DOT, ALICE).debit, 500);

		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 100);
		assert_ok!(Currencies::transfer(RuntimeOrigin::signed(ALICE), BOB, AUSD, 100));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_ok!(HonzonModule::transfer_debit(RuntimeOrigin::signed(ALICE), BTC, DOT, 5));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
	});
}
