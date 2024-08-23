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

//! Unit tests for example module.

#![cfg(test)]

use super::*;
use crate::mock::*;
use frame_support::{assert_err, assert_ok};
use orml_traits::MultiCurrency;

#[test]
fn redeem_works() {
	ExtBuilder::default()
		.balances(vec![(BOB, LCDOT, 100), (LiquidCrowdloan::account_id(), DOT, 100)])
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::free_balance(LCDOT, &BOB), 100);
			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100));
			assert_eq!(Currencies::free_balance(LCDOT, &BOB), 0);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 100);
			assert_eq!(Currencies::free_balance(DOT, &LiquidCrowdloan::account_id()), 0);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed {
				currency_id: DOT,
				amount: 100,
			}));
		});
}

#[test]
fn redeem_fails_if_not_enough_liquid_crowdloan_token() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		assert_err!(
			LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), u128::MAX),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn redeem_fails_if_not_enough_relay_chain_token() {
	ExtBuilder::default()
		.balances(vec![(BOB, LCDOT, 100)])
		.build()
		.execute_with(|| {
			assert_err!(
				LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
		});
}

#[test]
fn set_redeem_currency_id() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LCDOT, 100),
			(BOB, LCDOT, 100),
			(LiquidCrowdloan::account_id(), LDOT, 2200),
		])
		.build()
		.execute_with(|| {
			assert_ok!(LiquidCrowdloan::set_redeem_currency_id(
				RuntimeOrigin::signed(ALICE),
				LDOT
			));

			assert_eq!(Currencies::free_balance(LCDOT, &ALICE), 100);
			assert_err!(
				LiquidCrowdloan::redeem(RuntimeOrigin::signed(ALICE), u128::MAX),
				sp_runtime::ArithmeticError::Overflow
			);

			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(ALICE), 10));
			assert_eq!(Currencies::free_balance(LCDOT, &ALICE), 90);
			assert_eq!(Currencies::free_balance(LDOT, &ALICE), 110);
			assert_eq!(Currencies::free_balance(LDOT, &LiquidCrowdloan::account_id()), 2090);
			assert_eq!(Currencies::total_issuance(LCDOT), 190);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed {
				currency_id: LDOT,
				amount: 110,
			}));

			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(ALICE), 10));
			assert_eq!(Currencies::free_balance(LCDOT, &ALICE), 80);
			assert_eq!(Currencies::free_balance(LDOT, &ALICE), 220);
			assert_eq!(Currencies::free_balance(LDOT, &LiquidCrowdloan::account_id()), 1980);
			assert_eq!(Currencies::total_issuance(LCDOT), 180);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed {
				currency_id: LDOT,
				amount: 110,
			}));

			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(ALICE), 80));
			assert_eq!(Currencies::free_balance(LCDOT, &ALICE), 0);
			assert_eq!(Currencies::free_balance(LDOT, &ALICE), 1100);
			assert_eq!(Currencies::free_balance(LDOT, &LiquidCrowdloan::account_id()), 1100);
			assert_eq!(Currencies::total_issuance(LCDOT), 100);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed {
				currency_id: LDOT,
				amount: 880,
			}));

			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100));
			assert_eq!(Currencies::free_balance(LCDOT, &BOB), 0);
			assert_eq!(Currencies::free_balance(LDOT, &BOB), 1100);
			assert_eq!(Currencies::free_balance(LDOT, &LiquidCrowdloan::account_id()), 0);
			assert_eq!(Currencies::total_issuance(LCDOT), 0);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed {
				currency_id: LDOT,
				amount: 1100,
			}));
		});
}
