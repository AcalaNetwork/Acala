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

//! Unit tests for nominees election module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::traits::BadOrigin;

#[test]
fn bond_below_min_bond_threshold() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 4),
			Error::<Runtime>::BelowMinBondThreshold,
		);
	});
}

#[test]
fn bond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 0);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 0);
		assert_eq!(NomineesElectionModule::ledger(&ALICE), None);
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 50));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Bond {
			who: ALICE,
			amount: 50,
		}));
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 50);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 50);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 50);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 50);

		// bond amount over remain free
		assert_eq!(SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 0);
		assert_eq!(TokensModule::accounts(&BOB, LDOT).frozen, 0);
		assert_eq!(NomineesElectionModule::ledger(&BOB), None);
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(BOB), 2000));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Bond {
			who: BOB,
			amount: 1000,
		}));
		assert_eq!(TokensModule::accounts(&BOB, LDOT).frozen, 1000);
		assert_eq!(NomineesElectionModule::ledger(&BOB).unwrap().total(), 1000);
		assert_eq!(NomineesElectionModule::ledger(&BOB).unwrap().active(), 1000);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&BOB).unwrap_or(&0)), 1000);
	});
}

#[test]
fn unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 200));
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 200);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 200);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 200);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 200);

		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Unbond {
			who: ALICE,
			amount: 100,
		}));
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 100);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 200);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 100);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 200);

		MockCurrentEra::set(4);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(RuntimeOrigin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 100);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 100);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 100);

		// unbond amount over active
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 200));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Unbond {
			who: ALICE,
			amount: 100,
		}));
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 0);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 100);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 0);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 100);
	});
}

#[test]
fn unbond_exceed_max_unlock_chunk() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(1);
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(2);
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(3);
		assert_noop!(
			NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100),
			Error::<Runtime>::MaxUnlockChunksExceeded,
		);
	});
}

#[test]
fn unbond_remain_below_threshold() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 1000));
		assert_noop!(
			NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 996),
			Error::<Runtime>::BelowMinBondThreshold,
		);
	});
}

#[test]
fn rebond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NomineesElectionModule::rebond(RuntimeOrigin::signed(ALICE), 100),
			Error::<Runtime>::NotBonded,
		);
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(1);
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(2);
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		MockCurrentEra::set(3);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 700);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 700);

		assert_ok!(NomineesElectionModule::rebond(RuntimeOrigin::signed(ALICE), 150));
		System::assert_last_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Rebond {
			who: ALICE,
			amount: 150,
		}));
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 850);

		MockCurrentEra::set(4);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(RuntimeOrigin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 900);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 850);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 900);

		// rebond amount over unbonding
		assert_ok!(NomineesElectionModule::rebond(RuntimeOrigin::signed(ALICE), 200));
		System::assert_last_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Rebond {
			who: ALICE,
			amount: 50,
		}));
		assert_eq!(SHARES.with(|v| *v.borrow().get(&ALICE).unwrap_or(&0)), 900);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 900);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().active(), 900);
		assert_eq!(TokensModule::accounts(&ALICE, LDOT).frozen, 900);
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(MockCurrentEra::get(), 0);
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 1000));
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 1000);

		MockCurrentEra::set(3);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(RuntimeOrigin::signed(ALICE)));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 1000);
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().unlocking_len(), 1);
		assert_ok!(NomineesElectionModule::unbond(RuntimeOrigin::signed(ALICE), 100));

		MockCurrentEra::set(4);
		assert_ok!(NomineesElectionModule::withdraw_unbonded(RuntimeOrigin::signed(ALICE)));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(
			crate::Event::WithdrawUnbonded {
				who: ALICE,
				amount: 100,
			},
		));
		assert_eq!(NomineesElectionModule::ledger(&ALICE).unwrap().total(), 900);
	});
}

#[test]
fn nominate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NomineesElectionModule::nominate(RuntimeOrigin::signed(ALICE), vec![NOMINATEE_1]),
			Error::<Runtime>::NotBonded,
		);

		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 500));

		assert_noop!(
			NomineesElectionModule::nominate(RuntimeOrigin::signed(ALICE), vec![]),
			Error::<Runtime>::InvalidTargetsLength,
		);
		assert_noop!(
			NomineesElectionModule::nominate(
				RuntimeOrigin::signed(ALICE),
				vec![
					NOMINATEE_1,
					NOMINATEE_2,
					NOMINATEE_3,
					NOMINATEE_4,
					NOMINATEE_5,
					NOMINATEE_6
				]
			),
			Error::<Runtime>::InvalidTargetsLength,
		);

		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![]);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 0);
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Nominate {
			who: ALICE,
			targets: vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5],
		}));
		assert_eq!(
			NomineesElectionModule::nominations(&ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 500);
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5, NOMINATEE_6]
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Nominate {
			who: ALICE,
			targets: vec![NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5, NOMINATEE_6],
		}));
		assert_eq!(
			NomineesElectionModule::nominations(&ALICE),
			vec![NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5, NOMINATEE_6]
		);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 500);

		InvalidNominees::set(vec![NOMINATEE_8]);
		assert_noop!(
			NomineesElectionModule::nominate(RuntimeOrigin::signed(ALICE), vec![NOMINATEE_8]),
			Error::<Runtime>::InvalidNominee,
		);
	});
}

#[test]
fn chill_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 500));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		));
		assert_eq!(
			NomineesElectionModule::nominations(&ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 500);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 500);

		assert_ok!(NomineesElectionModule::chill(RuntimeOrigin::signed(ALICE)));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(crate::Event::Nominate {
			who: ALICE,
			targets: vec![],
		}));
		assert_eq!(NomineesElectionModule::nominations(&ALICE), vec![]);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 0);
	});
}

#[test]
fn update_votes_work() {
	ExtBuilder::default().build().execute_with(|| {
		<Votes<Runtime>>::insert(NOMINATEE_1, 50);
		<Votes<Runtime>>::insert(NOMINATEE_2, 100);
		NomineesElectionModule::update_votes(30, &[NOMINATEE_1, NOMINATEE_2], 50, &[NOMINATEE_1, NOMINATEE_2]);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 70);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 120);
		NomineesElectionModule::update_votes(0, &[NOMINATEE_1, NOMINATEE_2], 50, &[NOMINATEE_3, NOMINATEE_4]);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 70);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 120);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 50);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 50);
		NomineesElectionModule::update_votes(
			200,
			&[NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4],
			10,
			&[NOMINATEE_3, NOMINATEE_4],
		);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 0);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 10);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 10);
	});
}

#[test]
fn reset_reserved_nominees_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NomineesElectionModule::reset_reserved_nominees(RuntimeOrigin::signed(ALICE), vec![]),
			BadOrigin
		);

		assert_eq!(NomineesElectionModule::reserved_nominees(0), vec![]);
		assert_eq!(NomineesElectionModule::reserved_nominees(1), vec![]);
		assert_eq!(NomineesElectionModule::reserved_nominees(2), vec![]);

		assert_ok!(NomineesElectionModule::reset_reserved_nominees(
			RuntimeOrigin::root(),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
						.try_into()
						.unwrap()
				),
				(
					2,
					vec![NOMINATEE_5, NOMINATEE_3, NOMINATEE_4, NOMINATEE_1]
						.try_into()
						.unwrap()
				),
				(
					1,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_1, NOMINATEE_2, NOMINATEE_1]
						.try_into()
						.unwrap()
				),
			]
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(
			crate::Event::ResetReservedNominees {
				group_index: 0,
				reserved_nominees: vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5],
			},
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(
			crate::Event::ResetReservedNominees {
				group_index: 2,
				reserved_nominees: vec![NOMINATEE_1, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5],
			},
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(
			crate::Event::ResetReservedNominees {
				group_index: 1,
				reserved_nominees: vec![NOMINATEE_1, NOMINATEE_2],
			},
		));
		assert_eq!(
			NomineesElectionModule::reserved_nominees(0),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		);
		assert_eq!(
			NomineesElectionModule::reserved_nominees(1),
			vec![NOMINATEE_1, NOMINATEE_2]
		);
		assert_eq!(
			NomineesElectionModule::reserved_nominees(2),
			vec![NOMINATEE_1, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		);

		assert_ok!(NomineesElectionModule::reset_reserved_nominees(
			RuntimeOrigin::root(),
			vec![(2, Default::default())]
		));
		System::assert_has_event(mock::RuntimeEvent::NomineesElectionModule(
			crate::Event::ResetReservedNominees {
				group_index: 2,
				reserved_nominees: vec![],
			},
		));
		assert_eq!(NomineesElectionModule::reserved_nominees(2), vec![]);
	});
}

#[test]
fn sort_voted_nominees_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(BOB), 101));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(CHARLIE), 102));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(DAVE), 103));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(EVE), 104));

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 100);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![NOMINATEE_5, NOMINATEE_4, NOMINATEE_2, NOMINATEE_1, NOMINATEE_3]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(BOB),
			vec![NOMINATEE_2, NOMINATEE_6, NOMINATEE_10]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 201);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_10), 101);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_2,
				NOMINATEE_6,
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_4,
				NOMINATEE_1,
				NOMINATEE_3
			]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(CHARLIE),
			vec![NOMINATEE_3, NOMINATEE_7, NOMINATEE_11]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 201);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 202);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_7), 102);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_10), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_11), 102);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_3,
				NOMINATEE_2,
				NOMINATEE_11,
				NOMINATEE_7,
				NOMINATEE_6,
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_4,
				NOMINATEE_1
			]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(DAVE),
			vec![NOMINATEE_4, NOMINATEE_8, NOMINATEE_12]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 201);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 202);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 203);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_7), 102);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_8), 103);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_10), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_11), 102);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_12), 103);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_4,
				NOMINATEE_3,
				NOMINATEE_2,
				NOMINATEE_8,
				NOMINATEE_12,
				NOMINATEE_11,
				NOMINATEE_7,
				NOMINATEE_6,
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_1
			]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(EVE),
			vec![NOMINATEE_5, NOMINATEE_9, NOMINATEE_10, NOMINATEE_11, NOMINATEE_12]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 201);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 202);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 203);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 204);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_7), 102);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_8), 103);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_9), 104);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_10), 205);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_11), 206);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_12), 207);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_12,
				NOMINATEE_11,
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_4,
				NOMINATEE_3,
				NOMINATEE_2,
				NOMINATEE_9,
				NOMINATEE_8,
				NOMINATEE_7,
				NOMINATEE_6,
				NOMINATEE_1
			]
		);

		InvalidNominees::set(vec![NOMINATEE_12, NOMINATEE_11, NOMINATEE_2, NOMINATEE_9]);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_4,
				NOMINATEE_3,
				NOMINATEE_8,
				NOMINATEE_7,
				NOMINATEE_6,
				NOMINATEE_1
			]
		);
	});
}

#[test]
fn nominees_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NomineesElectionModule::nominees(), vec![]);
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(BOB), 101));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(CHARLIE), 102));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(BOB),
			vec![NOMINATEE_2, NOMINATEE_6, NOMINATEE_10]
		));
		assert_eq!(
			NomineesElectionModule::nominees(),
			vec![NOMINATEE_2, NOMINATEE_6, NOMINATEE_10]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		));
		assert_eq!(
			NomineesElectionModule::nominees(),
			vec![NOMINATEE_2, NOMINATEE_6, NOMINATEE_10, NOMINATEE_5, NOMINATEE_4]
		);

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(CHARLIE),
			vec![NOMINATEE_3, NOMINATEE_7, NOMINATEE_11]
		));
		assert_eq!(
			NomineesElectionModule::nominees(),
			vec![NOMINATEE_3, NOMINATEE_2, NOMINATEE_11, NOMINATEE_7, NOMINATEE_6]
		);
	});
}

#[test]
fn nominees_in_groups_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(ALICE), 100));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(BOB), 101));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(CHARLIE), 102));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(DAVE), 103));
		assert_ok!(NomineesElectionModule::bond(RuntimeOrigin::signed(EVE), 104));

		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(ALICE),
			vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_3, NOMINATEE_4, NOMINATEE_5]
		));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(BOB),
			vec![NOMINATEE_2, NOMINATEE_6, NOMINATEE_10]
		));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(CHARLIE),
			vec![NOMINATEE_3, NOMINATEE_7, NOMINATEE_11]
		));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(DAVE),
			vec![NOMINATEE_4, NOMINATEE_8, NOMINATEE_12]
		));
		assert_ok!(NomineesElectionModule::nominate(
			RuntimeOrigin::signed(EVE),
			vec![NOMINATEE_5, NOMINATEE_9, NOMINATEE_10, NOMINATEE_11, NOMINATEE_12]
		));
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_1), 100);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_2), 201);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_3), 202);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_4), 203);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_5), 204);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_6), 101);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_7), 102);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_8), 103);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_9), 104);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_10), 205);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_11), 206);
		assert_eq!(NomineesElectionModule::votes(NOMINATEE_12), 207);
		assert_eq!(
			NomineesElectionModule::sort_voted_nominees(),
			vec![
				NOMINATEE_12,
				NOMINATEE_11,
				NOMINATEE_10,
				NOMINATEE_5,
				NOMINATEE_4,
				NOMINATEE_3,
				NOMINATEE_2,
				NOMINATEE_9,
				NOMINATEE_8,
				NOMINATEE_7,
				NOMINATEE_6,
				NOMINATEE_1
			]
		);

		assert_eq!(
			NomineesElectionModule::nominees(),
			vec![NOMINATEE_12, NOMINATEE_11, NOMINATEE_10, NOMINATEE_5, NOMINATEE_4]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0]),
			vec![(
				0,
				vec![NOMINATEE_12, NOMINATEE_11, NOMINATEE_10, NOMINATEE_5, NOMINATEE_4]
			)]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1]),
			vec![
				(
					0,
					vec![NOMINATEE_12, NOMINATEE_10, NOMINATEE_4, NOMINATEE_2, NOMINATEE_8]
				),
				(
					1,
					vec![NOMINATEE_11, NOMINATEE_5, NOMINATEE_3, NOMINATEE_9, NOMINATEE_7]
				)
			]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1, 2]),
			vec![
				(0, vec![NOMINATEE_12, NOMINATEE_5, NOMINATEE_2, NOMINATEE_7]),
				(1, vec![NOMINATEE_11, NOMINATEE_4, NOMINATEE_9, NOMINATEE_6]),
				(2, vec![NOMINATEE_10, NOMINATEE_3, NOMINATEE_8, NOMINATEE_1])
			]
		);

		assert_ok!(NomineesElectionModule::reset_reserved_nominees(
			RuntimeOrigin::root(),
			vec![(0, vec![NOMINATEE_10, NOMINATEE_2, NOMINATEE_1].try_into().unwrap())]
		));
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0]),
			vec![(
				0,
				vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_12, NOMINATEE_11]
			)]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1]),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_5, NOMINATEE_3]
				),
				(
					1,
					vec![NOMINATEE_12, NOMINATEE_11, NOMINATEE_10, NOMINATEE_4, NOMINATEE_2]
				)
			]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1, 2]),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_9, NOMINATEE_7]
				),
				(
					1,
					vec![NOMINATEE_12, NOMINATEE_10, NOMINATEE_4, NOMINATEE_2, NOMINATEE_6]
				),
				(
					2,
					vec![NOMINATEE_11, NOMINATEE_5, NOMINATEE_3, NOMINATEE_8, NOMINATEE_1]
				)
			]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1, 2, 3]),
			vec![
				(0, vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_7]),
				(1, vec![NOMINATEE_12, NOMINATEE_5, NOMINATEE_2, NOMINATEE_6]),
				(2, vec![NOMINATEE_11, NOMINATEE_4, NOMINATEE_9, NOMINATEE_1]),
				(3, vec![NOMINATEE_10, NOMINATEE_3, NOMINATEE_8])
			]
		);

		assert_ok!(NomineesElectionModule::reset_reserved_nominees(
			RuntimeOrigin::root(),
			vec![
				(0, vec![NOMINATEE_10, NOMINATEE_2, NOMINATEE_1].try_into().unwrap()),
				(1, vec![NOMINATEE_11, NOMINATEE_4, NOMINATEE_3].try_into().unwrap()),
				(2, vec![NOMINATEE_12, NOMINATEE_6, NOMINATEE_5].try_into().unwrap())
			]
		));
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0]),
			vec![(
				0,
				vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_12, NOMINATEE_11]
			)]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1]),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_12, NOMINATEE_11]
				),
				(
					1,
					vec![NOMINATEE_3, NOMINATEE_4, NOMINATEE_11, NOMINATEE_10, NOMINATEE_5]
				)
			]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1, 2]),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_12, NOMINATEE_5]
				),
				(
					1,
					vec![NOMINATEE_3, NOMINATEE_4, NOMINATEE_11, NOMINATEE_10, NOMINATEE_2]
				),
				(
					2,
					vec![NOMINATEE_5, NOMINATEE_6, NOMINATEE_12, NOMINATEE_11, NOMINATEE_4]
				)
			]
		);
		assert_eq!(
			NomineesElectionModule::nominees_in_groups(vec![0, 1, 2, 3]),
			vec![
				(
					0,
					vec![NOMINATEE_1, NOMINATEE_2, NOMINATEE_10, NOMINATEE_5, NOMINATEE_9]
				),
				(
					1,
					vec![NOMINATEE_3, NOMINATEE_4, NOMINATEE_11, NOMINATEE_2, NOMINATEE_8]
				),
				(
					2,
					vec![NOMINATEE_5, NOMINATEE_6, NOMINATEE_12, NOMINATEE_4, NOMINATEE_7]
				),
				(
					3,
					vec![NOMINATEE_12, NOMINATEE_11, NOMINATEE_10, NOMINATEE_3, NOMINATEE_6]
				),
			]
		);
	});
}
