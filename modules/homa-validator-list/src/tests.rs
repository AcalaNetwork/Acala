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

//! Unit tests for homa validator list module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::traits::BadOrigin;

#[test]
fn guarantee_work() {
	ExtBuilder::default().build().execute_with(|| {
		let guarantee = Guarantee {
			total: 1000,
			bonded: 800,
			unbonding: Some((200, 10)),
		};

		assert_eq!(guarantee.consolidate_unbonding(9).unbonding, Some((200, 10)));
		assert_eq!(guarantee.consolidate_unbonding(10).unbonding, None);

		assert_eq!(
			guarantee.rebond(50),
			Guarantee {
				total: 1000,
				bonded: 850,
				unbonding: Some((150, 10)),
			}
		);
		assert_eq!(
			guarantee.rebond(200),
			Guarantee {
				total: 1000,
				bonded: 1000,
				unbonding: None,
			}
		);

		assert_eq!(
			guarantee.slash(200),
			Guarantee {
				total: 800,
				bonded: 600,
				unbonding: Some((200, 10)),
			}
		);
		assert_eq!(
			guarantee.slash(850),
			Guarantee {
				total: 150,
				bonded: 0,
				unbonding: Some((150, 10)),
			}
		);
		assert_eq!(
			guarantee.slash(1000),
			Guarantee {
				total: 0,
				bonded: 0,
				unbonding: None,
			}
		);
	});
}

#[test]
fn freeze_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			HomaValidatorListModule::freeze(Origin::signed(ALICE), vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]),
			BadOrigin
		);

		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_ok!(HomaValidatorListModule::freeze(
			Origin::signed(10),
			vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen,
			true
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen,
			true
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen,
			true
		);
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::FreezeValidator(
			VALIDATOR_1,
		)));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::FreezeValidator(
			VALIDATOR_2,
		)));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::FreezeValidator(
			VALIDATOR_3,
		)));
	});
}

#[test]
fn thaw_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			HomaValidatorListModule::thaw(Origin::signed(ALICE), vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]),
			BadOrigin
		);

		assert_ok!(HomaValidatorListModule::freeze(
			Origin::signed(10),
			vec![VALIDATOR_1, VALIDATOR_2]
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen,
			true
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen,
			true
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_ok!(HomaValidatorListModule::thaw(
			Origin::signed(10),
			vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen,
			false
		);
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::ThawValidator(
			VALIDATOR_1,
		)));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::ThawValidator(
			VALIDATOR_2,
		)));
	});
}

#[test]
fn bond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_noop!(
			HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 99),
			Error::<Runtime>::BelowMinBondAmount
		);
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 0,
				bonded: 0,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 0);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			0
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			0
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)), 0);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 100));
		System::assert_last_event(mock::Event::HomaValidatorListModule(crate::Event::BondGuarantee(
			ALICE,
			VALIDATOR_1,
			100,
		)));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 100,
				bonded: 100,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 100);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			100
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			100
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			100
		);

		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, BOB).unwrap_or_default(),
			Guarantee {
				total: 0,
				bonded: 0,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 0);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			0
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			100
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_1)).unwrap_or(&0)), 0);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(BOB), VALIDATOR_1, 300));
		System::assert_last_event(mock::Event::HomaValidatorListModule(crate::Event::BondGuarantee(
			BOB,
			VALIDATOR_1,
			300,
		)));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, BOB).unwrap_or_default(),
			Guarantee {
				total: 300,
				bonded: 300,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 300);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			300
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			400
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_1)).unwrap_or(&0)), 300);

		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_2, BOB).unwrap_or_default(),
			Guarantee {
				total: 0,
				bonded: 0,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 300);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			300
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.total_insurance,
			0
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_2)).unwrap_or(&0)), 0);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(BOB), VALIDATOR_2, 200));
		System::assert_last_event(mock::Event::HomaValidatorListModule(crate::Event::BondGuarantee(
			BOB,
			VALIDATOR_2,
			200,
		)));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_2, BOB).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 200,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 500);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			500
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_2)).unwrap_or(&0)), 200);
	});
}

#[test]
fn unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 200));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 200,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		assert_noop!(
			HomaValidatorListModule::unbond(Origin::signed(ALICE), VALIDATOR_1, 199),
			Error::<Runtime>::BelowMinBondAmount
		);

		assert_ok!(HomaValidatorListModule::unbond(Origin::signed(ALICE), VALIDATOR_1, 100));
		System::assert_last_event(mock::Event::HomaValidatorListModule(crate::Event::UnbondGuarantee(
			ALICE,
			VALIDATOR_1,
			100,
		)));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 100,
				unbonding: Some((100, 101))
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		assert_noop!(
			HomaValidatorListModule::unbond(Origin::signed(ALICE), VALIDATOR_1, 100),
			Error::<Runtime>::UnbondingExists
		);
	});
}

#[test]
fn rebond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 200));
		assert_ok!(HomaValidatorListModule::unbond(Origin::signed(ALICE), VALIDATOR_1, 100));

		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 100,
				unbonding: Some((100, 101))
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		assert_ok!(HomaValidatorListModule::rebond(Origin::signed(ALICE), VALIDATOR_1, 50));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 150,
				unbonding: Some((50, 101))
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 200));
		assert_ok!(HomaValidatorListModule::unbond(Origin::signed(ALICE), VALIDATOR_1, 100));
		assert_ok!(HomaValidatorListModule::bond(Origin::signed(BOB), VALIDATOR_1, 200));
		assert_ok!(HomaValidatorListModule::unbond(Origin::signed(BOB), VALIDATOR_1, 100));

		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 100,
				unbonding: Some((100, 101))
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			400
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		System::set_block_number(100);
		assert_ok!(HomaValidatorListModule::withdraw_unbonded(
			Origin::signed(ALICE),
			VALIDATOR_1
		));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 100,
				unbonding: Some((100, 101))
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 200);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			200
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			400
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		System::set_block_number(101);
		assert_ok!(HomaValidatorListModule::withdraw_unbonded(
			Origin::signed(ALICE),
			VALIDATOR_1
		));
		System::assert_last_event(mock::Event::HomaValidatorListModule(crate::Event::WithdrawnGuarantee(
			ALICE,
			VALIDATOR_1,
			100,
		)));
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 100,
				bonded: 100,
				unbonding: None
			}
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 100);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			100
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			300
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			100
		);

		assert_ok!(HomaValidatorListModule::freeze(Origin::signed(10), vec![VALIDATOR_1]));
		assert_noop!(
			HomaValidatorListModule::withdraw_unbonded(Origin::signed(BOB), VALIDATOR_1),
			Error::<Runtime>::FrozenValidator
		);
	});
}

#[test]
fn slash_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 100));
		assert_ok!(HomaValidatorListModule::bond(Origin::signed(BOB), VALIDATOR_1, 200));
		assert_ok!(HomaValidatorListModule::bond(Origin::signed(BOB), VALIDATOR_2, 300));

		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			300
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.total_insurance,
			300
		);

		// ALICE
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 100,
				bonded: 100,
				unbonding: None
			}
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			100
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 100);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			100
		);

		// BOB
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, BOB).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 200,
				unbonding: None
			}
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_1)).unwrap_or(&0)), 200);
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_2, BOB).unwrap_or_default(),
			Guarantee {
				total: 300,
				bonded: 300,
				unbonding: None
			}
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_2)).unwrap_or(&0)), 300);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 500);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			500
		);

		assert_noop!(
			HomaValidatorListModule::slash(
				Origin::signed(ALICE),
				vec![
					SlashInfo {
						validator: VALIDATOR_1,
						relaychain_token_amount: 90
					},
					SlashInfo {
						validator: VALIDATOR_2,
						relaychain_token_amount: 50
					},
				]
			),
			BadOrigin
		);

		assert_ok!(HomaValidatorListModule::slash(
			Origin::signed(10),
			vec![
				SlashInfo {
					validator: VALIDATOR_1,
					relaychain_token_amount: 90
				},
				SlashInfo {
					validator: VALIDATOR_2,
					relaychain_token_amount: 50
				},
			]
		));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::SlashGuarantee(
			ALICE,
			VALIDATOR_1,
			59,
		)));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::SlashGuarantee(
			BOB,
			VALIDATOR_1,
			119,
		)));
		System::assert_has_event(mock::Event::HomaValidatorListModule(crate::Event::SlashGuarantee(
			BOB,
			VALIDATOR_2,
			100,
		)));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			122
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.total_insurance,
			200
		);

		// ALICE
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, ALICE).unwrap_or_default(),
			Guarantee {
				total: 41,
				bonded: 41,
				unbonding: None
			}
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			41
		);
		assert_eq!(OrmlTokens::accounts(ALICE, LDOT).frozen, 41);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(ALICE).unwrap_or_default(),
			41
		);

		// BOB
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_1, BOB).unwrap_or_default(),
			Guarantee {
				total: 81,
				bonded: 81,
				unbonding: None
			}
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_1)).unwrap_or(&0)), 81);
		assert_eq!(
			HomaValidatorListModule::guarantees(VALIDATOR_2, BOB).unwrap_or_default(),
			Guarantee {
				total: 200,
				bonded: 200,
				unbonding: None
			}
		);
		assert_eq!(SHARES.with(|v| *v.borrow().get(&(BOB, VALIDATOR_2)).unwrap_or(&0)), 200);
		assert_eq!(OrmlTokens::accounts(BOB, LDOT).frozen, 281);
		assert_eq!(
			HomaValidatorListModule::total_locked_by_guarantor(BOB).unwrap_or_default(),
			281
		);
	});
}

#[test]
fn contains_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 100));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			100
		);
		assert_eq!(HomaValidatorListModule::contains(&VALIDATOR_1), false);

		assert_ok!(HomaValidatorListModule::bond(Origin::signed(ALICE), VALIDATOR_1, 100));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.total_insurance,
			200
		);
		assert_eq!(HomaValidatorListModule::contains(&VALIDATOR_1), true);
	});
}
