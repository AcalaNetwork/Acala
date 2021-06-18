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

//! Unit tests for nominees election module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;

#[test]
fn bond_below_min_bond_threshold() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 0);
		assert_noop!(
			NomineesElectionModule::bond(Origin::signed(ALICE), 4),
			Error::<Runtime>::BelowMinBondThreshold,
		);
	});
}

#[test]
fn bond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 0);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 0);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 50));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 50);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 50);
	});
}

#[test]
fn bond_amount_over_remain_free() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 0);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 0);
		assert_eq!(LDOTCurrency::free_balance(&ALICE), 1000);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 2000));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 1000);
	});
}

#[test]
fn unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 200));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 200);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 200);
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 200);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 100);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking[0].value, 100);
	});
}

#[test]
fn unbond_exceed_max_unlock_chunk() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 3);
		assert_noop!(
			NomineesElectionModule::unbond(Origin::signed(ALICE), 100),
			Error::<Runtime>::MaxUnlockChunksExceeded,
		);
	});
}

#[test]
fn unbond_amount_over_active() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 1000));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 1000);
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 1500));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 0);
	});
}

#[test]
fn unbond_remain_below_threshold() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 1000));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 1000);
		assert_noop!(
			NomineesElectionModule::unbond(Origin::signed(ALICE), 996),
			Error::<Runtime>::BelowMinBondThreshold,
		);
	});
}

#[test]
fn rebond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_noop!(
			NomineesElectionModule::rebond(Origin::signed(ALICE), 100),
			Error::<Runtime>::NoUnlockChunk,
		);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 700);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 3);
		assert_ok!(NomineesElectionModule::rebond(Origin::signed(ALICE), 150));
		System::assert_last_event(mock::Event::NomineesElectionModule(crate::Event::Rebond(ALICE, 150)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 850);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 2);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking[1].value, 50);
		assert_ok!(NomineesElectionModule::rebond(Origin::signed(ALICE), 200));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).active, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 0);
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NomineesElectionModule::current_era(), 0);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		NomineesElectionModule::on_new_era(3);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(Origin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 1);
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::unbond(Origin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 3);
		NomineesElectionModule::on_new_era(4);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(Origin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).total, 900);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unlocking.len(), 2);
	});
}

#[test]
fn nominate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NomineesElectionModule::nominate(Origin::signed(ALICE), vec![]),
			Error::<Runtime>::InvalidTargetsLength,
		);
		assert_noop!(
			NomineesElectionModule::nominate(Origin::signed(ALICE), vec![1, 2, 3, 4, 5, 6]),
			Error::<Runtime>::InvalidTargetsLength,
		);
		assert_noop!(
			NomineesElectionModule::nominate(Origin::signed(ALICE), vec![1, 2, 3, 4, 5]),
			Error::<Runtime>::NoBonded,
		);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 500));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![]);
		assert_eq!(NomineesElectionModule::votes(1), 0);
		assert_ok!(NomineesElectionModule::nominate(
			Origin::signed(ALICE),
			vec![1, 2, 3, 4, 5]
		));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![1, 2, 3, 4, 5]);
		assert_eq!(NomineesElectionModule::votes(1), 500);
		assert_eq!(NomineesElectionModule::votes(2), 500);
		assert_ok!(NomineesElectionModule::nominate(
			Origin::signed(ALICE),
			vec![2, 3, 4, 5, 6]
		));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![2, 3, 4, 5, 6]);
		assert_eq!(NomineesElectionModule::votes(1), 0);
		assert_eq!(NomineesElectionModule::votes(2), 500);
	});
}

#[test]
fn chill_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 500));
		assert_ok!(NomineesElectionModule::nominate(
			Origin::signed(ALICE),
			vec![1, 2, 3, 4, 5]
		));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![1, 2, 3, 4, 5]);
		assert_eq!(NomineesElectionModule::votes(1), 500);
		assert_eq!(NomineesElectionModule::votes(2), 500);
		assert_ok!(NomineesElectionModule::chill(Origin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![]);
		assert_eq!(NomineesElectionModule::votes(1), 0);
		assert_eq!(NomineesElectionModule::votes(2), 0);
	});
}

#[test]
fn rebalance_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(Origin::signed(ALICE), 500));
		assert_ok!(NomineesElectionModule::nominate(
			Origin::signed(ALICE),
			vec![1, 2, 3, 4, 5]
		));
		assert_eq!(NomineesElectionModule::nominees(), vec![]);
		assert_eq!(NomineesElectionModule::nominees().len(), 0);
		NomineesElectionModule::rebalance();
		assert_eq!(NomineesElectionModule::nominees().len(), 5);
		assert_eq!(NomineesElectionModule::nominees().contains(&1), true);
		assert_ok!(NomineesElectionModule::bond(Origin::signed(BOB), 600));
		assert_ok!(NomineesElectionModule::nominate(
			Origin::signed(ALICE),
			vec![2, 3, 4, 5, 6]
		));
		NomineesElectionModule::rebalance();
		assert_eq!(NomineesElectionModule::nominees().len(), 5);
		assert_eq!(NomineesElectionModule::nominees().contains(&1), false);
	});
}

#[test]
fn update_votes_work() {
	ExtBuilder::default().build().execute_with(|| {
		<Votes<Runtime>>::insert(1, 50);
		<Votes<Runtime>>::insert(2, 100);
		NomineesElectionModule::update_votes(30, &vec![1, 2], 50, &vec![1, 2]);
		assert_eq!(NomineesElectionModule::votes(1), 70);
		assert_eq!(NomineesElectionModule::votes(2), 120);
		NomineesElectionModule::update_votes(0, &vec![1, 2], 50, &vec![3, 4]);
		assert_eq!(NomineesElectionModule::votes(1), 70);
		assert_eq!(NomineesElectionModule::votes(2), 120);
		assert_eq!(NomineesElectionModule::votes(3), 50);
		assert_eq!(NomineesElectionModule::votes(4), 50);
		NomineesElectionModule::update_votes(200, &vec![1, 2, 3, 4], 10, &vec![3, 4]);
		assert_eq!(NomineesElectionModule::votes(1), 0);
		assert_eq!(NomineesElectionModule::votes(2), 0);
		assert_eq!(NomineesElectionModule::votes(3), 10);
		assert_eq!(NomineesElectionModule::votes(4), 10);
	});
}
