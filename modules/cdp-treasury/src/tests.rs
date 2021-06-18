// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

//! Unit tests for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;

#[test]
fn surplus_pool_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_ok!(Currencies::deposit(
			GetStableCurrencyId::get(),
			&CDPTreasuryModule::account_id(),
			500
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 500);
	});
}

#[test]
fn total_collaterals_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_ok!(Currencies::deposit(BTC, &CDPTreasuryModule::account_id(), 10));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
	});
}

#[test]
fn on_system_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(1000));
		assert_eq!(CDPTreasuryModule::debit_pool(), 1000);
		assert_noop!(
			CDPTreasuryModule::on_system_debit(Balance::max_value()),
			ArithmeticError::Overflow,
		);
	});
}

#[test]
fn on_system_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
	});
}

#[test]
fn offset_surplus_and_debit_on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(300));
		assert_eq!(CDPTreasuryModule::debit_pool(), 300);
		CDPTreasuryModule::on_finalize(2);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 700);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 700);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(800));
		assert_eq!(CDPTreasuryModule::debit_pool(), 800);
		CDPTreasuryModule::on_finalize(3);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 100);
	});
}

#[test]
fn issue_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);

		assert_ok!(CDPTreasuryModule::issue_debit(&ALICE, 1000, true));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 2000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);

		assert_ok!(CDPTreasuryModule::issue_debit(&ALICE, 1000, false));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 3000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 1000);
	});
}

#[test]
fn burn_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::burn_debit(&ALICE, 300));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 700);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
	});
}

#[test]
fn deposit_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_ok!(CDPTreasuryModule::deposit_surplus(&ALICE, 300));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 700);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 300);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 300);
	});
}

#[test]
fn deposit_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(CDPTreasuryModule::deposit_collateral(&ALICE, BTC, 10000).is_ok(), false);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&ALICE, BTC, 500));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 500);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 500);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 500);
	});
}

#[test]
fn withdraw_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::deposit_collateral(&ALICE, BTC, 500));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 500);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 500);
		assert_eq!(Currencies::free_balance(BTC, &BOB), 1000);
		assert_eq!(CDPTreasuryModule::withdraw_collateral(&BOB, BTC, 501).is_ok(), false);
		assert_ok!(CDPTreasuryModule::withdraw_collateral(&BOB, BTC, 400));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 100);
		assert_eq!(Currencies::free_balance(BTC, &BOB), 1400);
	});
}

#[test]
fn get_total_collaterals_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::deposit_collateral(&ALICE, BTC, 500));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(BTC), 500);
	});
}

#[test]
fn get_debit_proportion_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CDPTreasuryModule::get_debit_proportion(100),
			Ratio::saturating_from_rational(100, Currencies::total_issuance(AUSD))
		);
	});
}

#[test]
fn swap_collateral_to_exact_stable_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(ALICE),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(ALICE),
			BTC,
			DOT,
			900,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(BOB),
			DOT,
			AUSD,
			1000,
			1000,
			0,
			false
		));
		assert_ok!(CDPTreasuryModule::deposit_collateral(&BOB, BTC, 200));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals_not_in_auction(BTC), 200);

		assert_noop!(
			CDPTreasuryModule::swap_collateral_to_exact_stable(BTC, 201, 499, None, None, false),
			Error::<Runtime>::CollateralNotEnough,
		);

		assert_ok!(CDPTreasuryModule::swap_collateral_to_exact_stable(
			BTC, 100, 499, None, None, false
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 499);
		assert_eq!(CDPTreasuryModule::total_collaterals_not_in_auction(BTC), 100);

		assert_noop!(
			CDPTreasuryModule::swap_collateral_to_exact_stable(BTC, 100, 199, None, Some(&vec![BTC]), false),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			CDPTreasuryModule::swap_collateral_to_exact_stable(BTC, 100, 199, None, Some(&vec![BTC, DOT]), false),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			CDPTreasuryModule::swap_collateral_to_exact_stable(BTC, 100, 199, None, Some(&vec![DOT, AUSD]), false),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_ok!(CDPTreasuryModule::swap_collateral_to_exact_stable(
			BTC,
			100,
			10,
			None,
			Some(&vec![BTC, DOT, AUSD]),
			false
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 509);
		assert_eq!(CDPTreasuryModule::total_collaterals_not_in_auction(BTC), 89);
	});
}

#[test]
fn swap_exact_collateral_to_stable_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(ALICE),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(ALICE),
			BTC,
			DOT,
			900,
			1000,
			0,
			false
		));
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(BOB),
			DOT,
			AUSD,
			1000,
			1000,
			0,
			false
		));
		assert_ok!(CDPTreasuryModule::deposit_collateral(&BOB, BTC, 200));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 200);

		assert_noop!(
			CDPTreasuryModule::swap_exact_collateral_to_stable(BTC, 200, 100, None, None, true),
			Error::<Runtime>::CollateralNotEnough,
		);

		assert_ok!(CDPTreasuryModule::create_collateral_auctions(
			BTC, 200, 1000, ALICE, true
		));
		assert_eq!(TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut()), 200);

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 200);
		assert_eq!(MockAuctionManager::get_total_collateral_in_auction(BTC), 200);

		assert_ok!(CDPTreasuryModule::swap_exact_collateral_to_stable(
			BTC, 100, 400, None, None, true
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 500);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);

		assert_noop!(
			CDPTreasuryModule::swap_exact_collateral_to_stable(BTC, 100, 199, None, Some(&vec![BTC]), true),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			CDPTreasuryModule::swap_exact_collateral_to_stable(BTC, 100, 199, None, Some(&vec![BTC, DOT]), true),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			CDPTreasuryModule::swap_exact_collateral_to_stable(BTC, 100, 199, None, Some(&vec![DOT, AUSD]), true),
			Error::<Runtime>::InvalidSwapPath
		);

		assert_ok!(CDPTreasuryModule::swap_exact_collateral_to_stable(
			BTC,
			100,
			10,
			None,
			Some(&vec![BTC, DOT, AUSD]),
			true
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 590);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
	});
}

#[test]
fn create_collateral_auctions_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Currencies::deposit(BTC, &CDPTreasuryModule::account_id(), 10000));
		assert_eq!(CDPTreasuryModule::expected_collateral_auction_size(BTC), 0);
		assert_noop!(
			CDPTreasuryModule::create_collateral_auctions(BTC, 10001, 1000, ALICE, true),
			Error::<Runtime>::CollateralNotEnough,
		);

		// without collateral auction maximum size
		assert_ok!(CDPTreasuryModule::create_collateral_auctions(
			BTC, 1000, 1000, ALICE, true
		));
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 1);
		assert_eq!(TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut()), 1000);

		// set collateral auction maximum size
		assert_ok!(CDPTreasuryModule::set_expected_collateral_auction_size(
			Origin::signed(1),
			BTC,
			300
		));

		// amount < collateral auction maximum size
		// auction + 1
		assert_ok!(CDPTreasuryModule::create_collateral_auctions(
			BTC, 200, 1000, ALICE, true
		));
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 2);
		assert_eq!(TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut()), 1200);

		// not exceed lots count cap
		// auction + 4
		assert_ok!(CDPTreasuryModule::create_collateral_auctions(
			BTC, 1000, 1000, ALICE, true
		));
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 6);
		assert_eq!(TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut()), 2200);

		// exceed lots count cap
		// auction + 5
		assert_ok!(CDPTreasuryModule::create_collateral_auctions(
			BTC, 2000, 1000, ALICE, true
		));
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 11);
		assert_eq!(TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut()), 4200);
	});
}

#[test]
fn set_expected_collateral_auction_size_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CDPTreasuryModule::expected_collateral_auction_size(BTC), 0);
		assert_noop!(
			CDPTreasuryModule::set_expected_collateral_auction_size(Origin::signed(5), BTC, 200),
			BadOrigin
		);
		assert_ok!(CDPTreasuryModule::set_expected_collateral_auction_size(
			Origin::signed(1),
			BTC,
			200
		));
		System::assert_last_event(Event::CDPTreasuryModule(
			crate::Event::ExpectedCollateralAuctionSizeUpdated(BTC, 200),
		));
	});
}

#[test]
fn extract_surplus_to_treasury_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &TreasuryAccount::get()), 0);

		assert_noop!(
			CDPTreasuryModule::extract_surplus_to_treasury(Origin::signed(5), 200),
			BadOrigin
		);
		assert_ok!(CDPTreasuryModule::extract_surplus_to_treasury(Origin::signed(1), 200));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 800);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 800);
		assert_eq!(Currencies::free_balance(AUSD, &TreasuryAccount::get()), 200);
	});
}
