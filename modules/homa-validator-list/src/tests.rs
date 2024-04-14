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
		assert_noop!(
			HomaValidatorListModule::freeze(
				RuntimeOrigin::signed(ALICE),
				vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
			),
			BadOrigin
		);

		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen,
		);
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen,
		);
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen,
		);
		assert_ok!(HomaValidatorListModule::freeze(
			RuntimeOrigin::root(),
			vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
		));
		assert!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen
		);

		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::FreezeValidator { validator: VALIDATOR_1 },
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::FreezeValidator { validator: VALIDATOR_2 },
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::FreezeValidator { validator: VALIDATOR_3 },
		));
	});
}

#[test]
fn thaw_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaValidatorListModule::thaw(
				RuntimeOrigin::signed(ALICE),
				vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
			),
			BadOrigin
		);

		assert_ok!(HomaValidatorListModule::freeze(
			RuntimeOrigin::root(),
			vec![VALIDATOR_1, VALIDATOR_2]
		));
		assert!(
			HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen
		);
		assert_ok!(HomaValidatorListModule::thaw(
			RuntimeOrigin::root(),
			vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3]
		));
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_1)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_2)
				.unwrap_or_default()
				.is_frozen
		);
		assert!(
			!HomaValidatorListModule::validator_backings(VALIDATOR_3)
				.unwrap_or_default()
				.is_frozen
		);
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::ThawValidator { validator: VALIDATOR_1 },
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::ThawValidator { validator: VALIDATOR_2 },
		));
	});
}

#[test]
fn bond_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaValidatorListModule::bond(RuntimeOrigin::signed(ALICE), VALIDATOR_1, 99),
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

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			100
		));
		System::assert_last_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::BondGuarantee {
				who: ALICE,
				validator: VALIDATOR_1,
				bond: 100,
			},
		));
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

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_1,
			300
		));
		System::assert_last_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::BondGuarantee {
				who: BOB,
				validator: VALIDATOR_1,
				bond: 300,
			},
		));
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

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_2,
			200
		));
		System::assert_last_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::BondGuarantee {
				who: BOB,
				validator: VALIDATOR_2,
				bond: 200,
			},
		));
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
		MockBlockNumberProvider::set(1);

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			200
		));
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
			HomaValidatorListModule::unbond(RuntimeOrigin::signed(ALICE), VALIDATOR_1, 199),
			Error::<Runtime>::BelowMinBondAmount
		);

		assert_ok!(HomaValidatorListModule::unbond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			100
		));
		System::assert_last_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::UnbondGuarantee {
				who: ALICE,
				validator: VALIDATOR_1,
				bond: 100,
			},
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
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		assert_noop!(
			HomaValidatorListModule::unbond(RuntimeOrigin::signed(ALICE), VALIDATOR_1, 100),
			Error::<Runtime>::UnbondingExists
		);
	});
}

#[test]
fn rebond_work() {
	ExtBuilder::default().build().execute_with(|| {
		MockBlockNumberProvider::set(1);

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			200
		));
		assert_ok!(HomaValidatorListModule::unbond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			100
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
			200
		);
		assert_eq!(
			SHARES.with(|v| *v.borrow().get(&(ALICE, VALIDATOR_1)).unwrap_or(&0)),
			200
		);

		assert_ok!(HomaValidatorListModule::rebond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			50
		));
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
		MockBlockNumberProvider::set(1);

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			200
		));
		assert_ok!(HomaValidatorListModule::unbond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			100
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_1,
			200
		));
		assert_ok!(HomaValidatorListModule::unbond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_1,
			100
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

		MockBlockNumberProvider::set(100);
		assert_ok!(HomaValidatorListModule::withdraw_unbonded(
			RuntimeOrigin::signed(ALICE),
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
		System::reset_events();
		MockBlockNumberProvider::set(101);
		assert_ok!(HomaValidatorListModule::withdraw_unbonded(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::WithdrawnGuarantee {
				who: ALICE,
				validator: VALIDATOR_1,
				bond: 100,
			},
		));
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

		assert_ok!(HomaValidatorListModule::freeze(
			RuntimeOrigin::root(),
			vec![VALIDATOR_1]
		));
		assert_noop!(
			HomaValidatorListModule::withdraw_unbonded(RuntimeOrigin::signed(BOB), VALIDATOR_1),
			Error::<Runtime>::FrozenValidator
		);
	});
}

#[test]
fn slash_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(ALICE),
			VALIDATOR_1,
			100
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_1,
			200
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(BOB),
			VALIDATOR_2,
			300
		));

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
				RuntimeOrigin::signed(ALICE),
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
			RuntimeOrigin::root(),
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
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::SlashGuarantee {
				who: ALICE,
				validator: VALIDATOR_1,
				bond: 59,
			},
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::SlashGuarantee {
				who: BOB,
				validator: VALIDATOR_1,
				bond: 119,
			},
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::SlashGuarantee {
				who: BOB,
				validator: VALIDATOR_2,
				bond: 100,
			},
		));
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
fn set_reserved_validators_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaValidatorListModule::set_reserved_validators(RuntimeOrigin::signed(ALICE), vec![]),
			BadOrigin
		);

		assert_noop!(
			HomaValidatorListModule::set_reserved_validators(
				RuntimeOrigin::root(),
				vec![(
					0,
					vec![
						VALIDATOR_1,
						VALIDATOR_2,
						VALIDATOR_3,
						VALIDATOR_4,
						VALIDATOR_5,
						VALIDATOR_6
					]
				)]
			),
			Error::<Runtime>::ExceedMaxNominations
		);

		assert_eq!(HomaValidatorListModule::reserved_validators(0), vec![]);
		assert_eq!(HomaValidatorListModule::reserved_validators(1), vec![]);
		assert_eq!(HomaValidatorListModule::reserved_validators(2), vec![]);

		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![
				(
					0,
					vec![
						VALIDATOR_1,
						VALIDATOR_2,
						VALIDATOR_3,
						VALIDATOR_4,
						VALIDATOR_5,
						VALIDATOR_5
					]
				),
				(2, vec![VALIDATOR_5, VALIDATOR_3, VALIDATOR_4, VALIDATOR_1]),
				(1, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_1, VALIDATOR_2, VALIDATOR_1]),
			]
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::ResetReservedValidators {
				sub_account_index: 0,
				reserved_validators: vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3, VALIDATOR_4, VALIDATOR_5],
			},
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::ResetReservedValidators {
				sub_account_index: 1,
				reserved_validators: vec![VALIDATOR_1, VALIDATOR_2],
			},
		));
		System::assert_has_event(mock::RuntimeEvent::HomaValidatorListModule(
			crate::Event::ResetReservedValidators {
				sub_account_index: 2,
				reserved_validators: vec![VALIDATOR_1, VALIDATOR_3, VALIDATOR_4, VALIDATOR_5],
			},
		));

		assert_eq!(
			HomaValidatorListModule::reserved_validators(0).to_vec(),
			vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_3, VALIDATOR_4, VALIDATOR_5]
		);
		assert_eq!(
			HomaValidatorListModule::reserved_validators(1).to_vec(),
			vec![VALIDATOR_1, VALIDATOR_2]
		);
		assert_eq!(
			HomaValidatorListModule::reserved_validators(2).to_vec(),
			vec![VALIDATOR_1, VALIDATOR_3, VALIDATOR_4, VALIDATOR_5]
		);

		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![
				(0, vec![VALIDATOR_3, VALIDATOR_4]),
				(2, vec![]),
				(1, vec![VALIDATOR_5, VALIDATOR_3, VALIDATOR_4, VALIDATOR_1]),
			]
		));
		assert_eq!(
			HomaValidatorListModule::reserved_validators(0).to_vec(),
			vec![VALIDATOR_3, VALIDATOR_4]
		);
		assert_eq!(
			HomaValidatorListModule::reserved_validators(1).to_vec(),
			vec![VALIDATOR_1, VALIDATOR_3, VALIDATOR_4, VALIDATOR_5]
		);
		assert_eq!(HomaValidatorListModule::reserved_validators(2).to_vec(), vec![]);
	});
}

#[test]
fn sort_voted_nominations_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_6,
			200
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_7,
			300
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_8,
			500
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_9,
			400
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_10,
			100
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_6),
			Some(ValidatorBacking {
				total_insurance: 200,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_7),
			Some(ValidatorBacking {
				total_insurance: 300,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_8),
			Some(ValidatorBacking {
				total_insurance: 500,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_9),
			Some(ValidatorBacking {
				total_insurance: 400,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_10),
			Some(ValidatorBacking {
				total_insurance: 100,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_8, VALIDATOR_9, VALIDATOR_7, VALIDATOR_6]
		);

		// freeze VALIDATOR_7 & VALIDATOR_8
		assert_ok!(HomaValidatorListModule::freeze(
			RuntimeOrigin::root(),
			vec![VALIDATOR_8, VALIDATOR_7]
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_7),
			Some(ValidatorBacking {
				total_insurance: 300,
				is_frozen: true
			})
		);
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_8),
			Some(ValidatorBacking {
				total_insurance: 500,
				is_frozen: true
			})
		);
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_9, VALIDATOR_6]
		);

		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_10,
			400
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_10),
			Some(ValidatorBacking {
				total_insurance: 500,
				is_frozen: false
			})
		);
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_10, VALIDATOR_9, VALIDATOR_6]
		);

		// set VALIDATOR_8 & VALIDATOR_10 as reserved
		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![
				(0, vec![VALIDATOR_8, VALIDATOR_10]),
				(
					3,
					vec![VALIDATOR_6, VALIDATOR_7, VALIDATOR_8, VALIDATOR_9, VALIDATOR_10]
				),
			]
		));
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_9, VALIDATOR_6]
		);
	});
}

#[test]
fn get_nominations_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_1,
			100
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_2,
			150
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_3,
			250
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_4,
			350
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_5,
			450
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_6,
			240
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_7,
			300
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_8,
			500
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_9,
			400
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_10,
			100
		));

		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![
				VALIDATOR_8,
				VALIDATOR_5,
				VALIDATOR_9,
				VALIDATOR_4,
				VALIDATOR_7,
				VALIDATOR_3,
				VALIDATOR_6
			]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_8, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
				(1, vec![VALIDATOR_8, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
				(2, vec![VALIDATOR_8, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
			]
		);

		// freeze VALIDATOR_8
		assert_ok!(HomaValidatorListModule::freeze(
			RuntimeOrigin::root(),
			vec![VALIDATOR_8]
		));
		assert_eq!(
			HomaValidatorListModule::validator_backings(VALIDATOR_8),
			Some(ValidatorBacking {
				total_insurance: 500,
				is_frozen: true
			})
		);
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![
				VALIDATOR_5,
				VALIDATOR_9,
				VALIDATOR_4,
				VALIDATOR_7,
				VALIDATOR_3,
				VALIDATOR_6
			]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
				(1, vec![VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
				(2, vec![VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
			]
		);

		// set VALIDATOR_1 && VALIDATOR_2 as reserved validators
		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![(0, vec![VALIDATOR_1]), (1, vec![VALIDATOR_2]),]
		));
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![
				VALIDATOR_5,
				VALIDATOR_9,
				VALIDATOR_4,
				VALIDATOR_7,
				VALIDATOR_3,
				VALIDATOR_6
			]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_1, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
				(1, vec![VALIDATOR_2, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
				(2, vec![VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
			]
		);

		// set VALIDATOR_5 as reserved validator
		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![(2, vec![VALIDATOR_5]),]
		));
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3, VALIDATOR_6]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_1, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
				(1, vec![VALIDATOR_2, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
				(2, vec![VALIDATOR_5, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3]),
			]
		);

		// update reserved validators
		assert_ok!(HomaValidatorListModule::set_reserved_validators(
			RuntimeOrigin::root(),
			vec![
				(0, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5]),
				(1, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5]),
				(2, vec![VALIDATOR_1, VALIDATOR_2]),
			]
		));
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![VALIDATOR_9, VALIDATOR_4, VALIDATOR_7, VALIDATOR_3, VALIDATOR_6]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4]),
				(1, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5, VALIDATOR_7, VALIDATOR_3]),
				(2, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_9, VALIDATOR_4, VALIDATOR_7]),
			]
		);

		// VALIDATOR_11 & VALIDATOR_12 become sorted voted nominations
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_11,
			230
		));
		assert_ok!(HomaValidatorListModule::bond(
			RuntimeOrigin::signed(CHARLIE),
			VALIDATOR_12,
			220
		));
		assert_eq!(
			HomaValidatorListModule::sort_voted_nominations(),
			vec![
				VALIDATOR_9,
				VALIDATOR_4,
				VALIDATOR_7,
				VALIDATOR_3,
				VALIDATOR_6,
				VALIDATOR_11,
				VALIDATOR_12
			]
		);
		assert_eq!(
			HomaValidatorListModule::get(),
			vec![
				(0, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5, VALIDATOR_9, VALIDATOR_4]),
				(1, vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_5, VALIDATOR_7, VALIDATOR_3]),
				(
					2,
					vec![VALIDATOR_1, VALIDATOR_2, VALIDATOR_6, VALIDATOR_11, VALIDATOR_12]
				),
			]
		);
	});
}
