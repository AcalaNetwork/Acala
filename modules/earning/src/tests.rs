// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! Unit tests for the prices module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::fungible::Inspect};
use mock::*;

fn assert_no_handler_events() {
	OnBonded::assert_empty();
	OnUnbonded::assert_empty();
	OnUnstakeFee::assert_empty();
}

fn clear_handler_events() {
	OnBonded::clear();
	OnUnbonded::clear();
	OnUnstakeFee::clear();
}

#[test]
fn bond_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Earning::bond(Origin::signed(ALICE), 10),
			Error::<Runtime>::BelowMinBondThreshold,
		);

		assert_ok!(Earning::bond(Origin::signed(ALICE), 100));
		System::assert_last_event(
			Event::Bonded {
				who: ALICE,
				amount: 100,
			}
			.into(),
		);
		OnBonded::assert_eq_and_clear(vec![(ALICE, 100)]);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 900);

		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));
		System::assert_last_event(
			Event::Bonded {
				who: ALICE,
				amount: 900,
			}
			.into(),
		);
		OnBonded::assert_eq_and_clear(vec![(ALICE, 900)]);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 0);

		assert_no_handler_events();
	});
}

#[test]
fn unbonding_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Earning::unbond(Origin::signed(ALICE), 1000),
			Error::<Runtime>::NotBonded
		);
		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));

		assert_noop!(
			Earning::unbond(Origin::signed(ALICE), 999),
			Error::<Runtime>::BelowMinBondThreshold
		);

		clear_handler_events();

		// Won't unbond before unbonding period passes
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 1001));
		System::assert_last_event(
			Event::Unbonded {
				who: ALICE,
				amount: 1000,
			}
			.into(),
		);
		OnUnbonded::assert_eq_and_clear(vec![(ALICE, 1000)]);
		System::reset_events();
		assert_ok!(Earning::withdraw_unbonded(Origin::signed(ALICE)));
		assert_eq!(System::events(), vec![]);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 0);

		System::set_block_number(4);

		assert_ok!(Earning::withdraw_unbonded(Origin::signed(ALICE)));
		System::assert_last_event(
			Event::Withdrawn {
				who: ALICE,
				amount: 1000,
			}
			.into(),
		);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 1000);

		assert_noop!(
			Earning::unbond_instant(Origin::signed(ALICE), 1000),
			Error::<Runtime>::NotBonded
		);

		assert_no_handler_events();

		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));
		assert_eq!(Balances::reducible_balance(&ALICE, false), 0);
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 1000));

		System::reset_events();
		clear_handler_events();

		// unbond instant will not work on pending unbond funds
		assert_ok!(Earning::unbond_instant(Origin::signed(ALICE), 1001));
		assert_eq!(System::events(), vec![]);
		clear_handler_events();

		assert_ok!(Earning::rebond(Origin::signed(ALICE), 1000));
		OnBonded::assert_eq_and_clear(vec![(ALICE, 1000)]);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 0);

		assert_noop!(
			Earning::unbond_instant(Origin::signed(ALICE), 999),
			Error::<Runtime>::BelowMinBondThreshold
		);
		assert_ok!(Earning::unbond_instant(Origin::signed(ALICE), 1001));
		System::assert_last_event(
			Event::InstantUnbonded {
				who: ALICE,
				amount: 900,
				fee: 100,
			}
			.into(),
		);
		OnUnbonded::assert_eq_and_clear(vec![(ALICE, 900)]);
		OnUnstakeFee::assert_eq_and_clear(vec![100]);
		// takes instant unbonding fee
		assert_eq!(Balances::reducible_balance(&ALICE, false), 900);

		assert_no_handler_events();
	});
}

#[test]
fn unbonding_max_unlock_chunks_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));
		System::set_block_number(1);
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 100));
		System::set_block_number(2);
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 100));
		System::set_block_number(3);
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 100));
		System::set_block_number(4);
		assert_noop!(
			Earning::unbond(Origin::signed(ALICE), 100),
			Error::<Runtime>::MaxUnlockChunksExceeded
		);
	});
}

#[test]
fn rebond_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));
		assert_ok!(Earning::unbond(Origin::signed(ALICE), 1000));

		assert_noop!(
			Earning::rebond(Origin::signed(ALICE), 1),
			Error::<Runtime>::BelowMinBondThreshold
		);

		clear_handler_events();

		assert_ok!(Earning::rebond(Origin::signed(ALICE), 100));
		System::assert_last_event(
			Event::Rebonded {
				who: ALICE,
				amount: 100,
			}
			.into(),
		);
		OnBonded::assert_eq_and_clear(vec![(ALICE, 100)]);

		System::set_block_number(4);

		assert_ok!(Earning::withdraw_unbonded(Origin::signed(ALICE)));
		assert_eq!(Balances::reducible_balance(&ALICE, false), 900);

		assert_no_handler_events();
	});
}
