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

use crate as collator_selection;
use crate::{mock::*, Error, RESERVE_ID};
use frame_support::{
	assert_noop, assert_ok,
	storage::bounded_btree_set::BoundedBTreeSet,
	traits::{Currency, GenesisBuild, NamedReservableCurrency, OnInitialize},
};
use pallet_balances::Error as BalancesError;
use sp_runtime::{testing::UintAuthorityId, traits::BadOrigin};

#[test]
fn basic_setup_works() {
	new_test_ext().execute_with(|| {
		assert_eq!(CollatorSelection::desired_candidates(), 2);
		assert_eq!(CollatorSelection::candidacy_bond(), 10);

		assert!(CollatorSelection::candidates().is_empty());
		assert_eq!(CollatorSelection::invulnerables(), vec![1, 2]);
	});
}

#[test]
fn it_should_set_invulnerables() {
	new_test_ext().execute_with(|| {
		let new_set = vec![1, 2, 3, 4];
		assert_ok!(CollatorSelection::set_invulnerables(
			Origin::signed(RootAccount::get()),
			new_set.clone()
		));
		assert_eq!(CollatorSelection::invulnerables(), new_set);

		// cannot set with non-root.
		assert_noop!(
			CollatorSelection::set_invulnerables(Origin::signed(1), new_set.clone()),
			BadOrigin
		);
	});
}

#[test]
fn set_desired_candidates_works() {
	new_test_ext().execute_with(|| {
		// given
		assert_eq!(CollatorSelection::desired_candidates(), 2);

		// can set
		assert_ok!(CollatorSelection::set_desired_candidates(
			Origin::signed(RootAccount::get()),
			4
		));
		assert_eq!(CollatorSelection::desired_candidates(), 4);

		assert_noop!(
			CollatorSelection::set_desired_candidates(Origin::signed(RootAccount::get()), 5),
			Error::<Test>::MaxCandidatesExceeded
		);

		// rejects bad origin
		assert_noop!(
			CollatorSelection::set_desired_candidates(Origin::signed(1), 8),
			BadOrigin
		);
	});
}

#[test]
fn set_candidacy_bond() {
	new_test_ext().execute_with(|| {
		// given
		assert_eq!(CollatorSelection::candidacy_bond(), 10);

		// can set
		assert_ok!(CollatorSelection::set_candidacy_bond(
			Origin::signed(RootAccount::get()),
			7
		));
		assert_eq!(CollatorSelection::candidacy_bond(), 7);

		// rejects bad origin.
		assert_noop!(CollatorSelection::set_candidacy_bond(Origin::signed(1), 8), BadOrigin);
	});
}

#[test]
fn cannot_register_candidate_if_too_many() {
	new_test_ext().execute_with(|| {
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(Session::set_keys(
			Origin::signed(4),
			MockSessionKeys {
				aura: UintAuthorityId(4)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(4)));
		assert_noop!(
			CollatorSelection::register_as_candidate(Origin::signed(5)),
			Error::<Test>::MaxCandidatesExceeded
		);
	})
}

#[test]
fn cannot_register_as_candidate_if_invulnerable() {
	new_test_ext().execute_with(|| {
		assert_eq!(CollatorSelection::invulnerables(), vec![1, 2]);

		// can't 1 because it is invulnerable.
		assert_noop!(
			CollatorSelection::register_as_candidate(Origin::signed(1)),
			Error::<Test>::AlreadyInvulnerable,
		);
	})
}

#[test]
fn cannot_register_dupe_candidate() {
	new_test_ext().execute_with(|| {
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		// can add 3 as candidate
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));
		let mut collators = BoundedBTreeSet::new();
		assert_ok!(collators.try_insert(3));
		assert_eq!(CollatorSelection::candidates(), collators);
		assert_eq!(Balances::free_balance(3), 90);
		assert_eq!(Balances::reserved_balance_named(&RESERVE_ID, &3), 10);

		// but no more
		assert_noop!(
			CollatorSelection::register_as_candidate(Origin::signed(3)),
			Error::<Test>::AlreadyCandidate,
		);
	})
}

#[test]
fn cannot_register_as_candidate_if_poor() {
	new_test_ext().execute_with(|| {
		assert_eq!(Balances::free_balance(&3), 100);
		assert_eq!(Balances::free_balance(&33), 5);

		// works
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));

		// poor
		assert_noop!(
			CollatorSelection::register_as_candidate(Origin::signed(33)),
			Error::<Test>::RequireSessionKey,
		);

		assert_ok!(Session::set_keys(
			Origin::signed(33),
			MockSessionKeys {
				aura: UintAuthorityId(33)
			},
			vec![]
		));
		assert_noop!(
			CollatorSelection::register_as_candidate(Origin::signed(33)),
			BalancesError::<Test>::InsufficientBalance,
		);
	});
}

#[test]
fn register_as_candidate_works() {
	new_test_ext().execute_with(|| {
		// given
		assert_eq!(CollatorSelection::desired_candidates(), 2);
		assert_eq!(CollatorSelection::candidacy_bond(), 10);
		assert_eq!(CollatorSelection::candidates(), BoundedBTreeSet::new());
		assert_eq!(CollatorSelection::invulnerables(), vec![1, 2]);

		// take two endowed, non-invulnerables accounts.
		assert_eq!(Balances::free_balance(&3), 100);
		assert_eq!(Balances::free_balance(&4), 100);

		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(Session::set_keys(
			Origin::signed(4),
			MockSessionKeys {
				aura: UintAuthorityId(4)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(4)));

		assert_eq!(Balances::free_balance(&3), 90);
		assert_eq!(Balances::free_balance(&4), 90);

		assert_eq!(CollatorSelection::candidates().len(), 2);
	});
}

#[test]
fn register_candidate_works() {
	new_test_ext().execute_with(|| {
		// given
		assert_eq!(CollatorSelection::desired_candidates(), 2);
		assert_eq!(CollatorSelection::candidacy_bond(), 10);
		assert_eq!(CollatorSelection::candidates(), BoundedBTreeSet::new());
		assert_eq!(CollatorSelection::invulnerables(), vec![1, 2]);

		// take two endowed, non-invulnerables accounts.
		assert_eq!(Balances::free_balance(&33), 5);

		assert_ok!(Session::set_keys(
			Origin::signed(33),
			MockSessionKeys {
				aura: UintAuthorityId(33)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_candidate(
			Origin::signed(RootAccount::get()),
			33
		));
		assert_eq!(Balances::free_balance(&33), 5);
		assert_eq!(Balances::reserved_balance_named(&RESERVE_ID, &33), 0);
	});
}

#[test]
fn leave_intent() {
	new_test_ext().execute_with(|| {
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(Session::set_keys(
			Origin::signed(4),
			MockSessionKeys {
				aura: UintAuthorityId(4)
			},
			vec![]
		));
		// register a candidate.
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(4)));
		assert_eq!(Balances::free_balance(3), 90);
		assert_eq!(Balances::free_balance(4), 90);

		// cannot leave if not candidate.
		assert_noop!(
			CollatorSelection::leave_intent(Origin::signed(5)),
			Error::<Test>::NotCandidate
		);

		// bond is returned
		assert_ok!(CollatorSelection::leave_intent(Origin::signed(3)));
		assert_eq!(Balances::free_balance(3), 100);

		assert_noop!(
			CollatorSelection::leave_intent(Origin::signed(4)),
			Error::<Test>::BelowCandidatesMin
		);
	});
}

#[test]
fn fees_edgecases() {
	new_test_ext().execute_with(|| {
		// Nothing panics, no reward when no ED in balance
		Authorship::on_initialize(1);
		// put some money into the pot at ED
		Balances::make_free_balance_be(&CollatorSelection::account_id(), 5);
		// 4 is the default author.
		assert_eq!(Balances::free_balance(4), 100);
		assert_ok!(Session::set_keys(
			Origin::signed(4),
			MockSessionKeys {
				aura: UintAuthorityId(4)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(4)));
		// triggers `note_author`
		Authorship::on_initialize(1);

		let mut collators = BoundedBTreeSet::new();
		assert_ok!(collators.try_insert(4));
		assert_eq!(CollatorSelection::candidates(), collators);
		assert_eq!(Balances::reserved_balance_named(&RESERVE_ID, &4), 10);

		// Nothing received
		assert_eq!(Balances::free_balance(4), 90);
		// all fee stays
		assert_eq!(Balances::free_balance(CollatorSelection::account_id()), 5);
		// assert_eq!(Balances::reserved_balance(CollatorSelection::account_id()), <Balances as
		// Currency<_>>::minimum_balance());
	});
}

#[test]
fn session_management_works() {
	new_test_ext().execute_with(|| {
		initialize_to_block(1);

		assert_eq!(SessionChangeBlock::get(), 0);
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

		initialize_to_block(4);

		assert_eq!(SessionChangeBlock::get(), 0);
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

		// add a new collator
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));

		// session won't see this.
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);
		// but we have a new candidate.
		assert_eq!(CollatorSelection::candidates().len(), 1);

		initialize_to_block(10);
		assert_eq!(SessionChangeBlock::get(), 10);
		// pallet-session has 1 session delay; current validators are the same.
		assert_eq!(Session::validators(), vec![1, 2]);
		// queued ones are changed, and now we have 3.
		assert_eq!(Session::queued_keys().len(), 3);
		// session handlers (aura, et. al.) cannot see this yet.
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2]);

		initialize_to_block(20);
		assert_eq!(SessionChangeBlock::get(), 20);
		// changed are now reflected to session handlers.
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3]);
	});
}

#[test]
fn kick_mechanism() {
	new_test_ext().execute_with(|| {
		// add a new collator
		assert_ok!(Session::set_keys(
			Origin::signed(3),
			MockSessionKeys {
				aura: UintAuthorityId(3)
			},
			vec![]
		));
		assert_ok!(Session::set_keys(
			Origin::signed(4),
			MockSessionKeys {
				aura: UintAuthorityId(4)
			},
			vec![]
		));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(3)));
		assert_ok!(CollatorSelection::register_as_candidate(Origin::signed(4)));
		assert_eq!(CollatorSelection::candidates().len(), 2);
		initialize_to_block(21);
		assert_eq!(SessionChangeBlock::get(), 20);
		assert_eq!(CollatorSelection::candidates().len(), 2);
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3, 4]);
		let mut collators = BoundedBTreeSet::new();
		assert_ok!(collators.try_insert(3));
		assert_ok!(collators.try_insert(4));
		assert_eq!(CollatorSelection::candidates(), collators);
		assert_eq!(Balances::reserved_balance_named(&RESERVE_ID, &3), 10);
		assert_eq!(Balances::reserved_balance_named(&RESERVE_ID, &4), 10);

		initialize_to_block(31);
		// 4 authored this block, gets to stay 3 was kicked
		assert_eq!(SessionChangeBlock::get(), 30);
		assert_eq!(CollatorSelection::candidates().len(), 1);
		assert_eq!(SessionHandlerCollators::get(), vec![1, 2, 3, 4]);
		let mut collators = BoundedBTreeSet::new();
		assert_ok!(collators.try_insert(4));
		assert_eq!(CollatorSelection::candidates(), collators);
		// kicked collator gets funds back
		assert_eq!(Balances::free_balance(3), 100);
	});
}

#[test]
fn exceeding_max_invulnerables_should_fail() {
	new_test_ext().execute_with(|| {
		assert_ok!(CollatorSelection::set_invulnerables(
			Origin::signed(RootAccount::get()),
			vec![1, 2, 3, 4]
		));

		assert_noop!(
			CollatorSelection::set_invulnerables(Origin::signed(RootAccount::get()), vec![1, 2, 3, 4, 5]),
			Error::<Test>::MaxInvulnerablesExceeded
		);
	});
}

#[test]
#[should_panic = "duplicate invulnerables in genesis."]
fn cannot_set_genesis_value_twice() {
	sp_tracing::try_init_simple();
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
	let invulnerables = vec![1, 1];

	let collator_selection = collator_selection::GenesisConfig::<Test> {
		desired_candidates: 2,
		candidacy_bond: 10,
		invulnerables,
	};
	// collator selection must be initialized before session.
	collator_selection.assimilate_storage(&mut t).unwrap();
}
