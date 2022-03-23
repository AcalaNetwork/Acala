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

//! Unit tests for the foreign state oracle module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, Origin, *};
use sp_runtime::traits::Scale;
use sp_std::ops::{Add, Sub};

const CALL_WEIGHT: Weight = u64::MAX;

#[test]
fn dispatch_and_remove_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(QueryExample::mock_create_query(Origin::signed(ALICE), vec![], None));
		assert!(QueryRequests::<Runtime>::get(0).is_some());
		assert_ok!(QueryExample::mock_cancel_query(Origin::none(), ALICE, 0));
		assert!(QueryRequests::<Runtime>::get(0).is_none());

		assert_ok!(QueryExample::mock_create_query(
			Origin::signed(ALICE),
			b"world".to_vec(),
			None
		));
		System::assert_last_event(Event::ForeignStateOracle(crate::Event::QueryRequestCreated {
			expiry: None,
			query_id: 1,
		}));

		assert_noop!(
			ForeignStateOracle::respond_query_request(Origin::signed(1), 0, b"hello".to_vec(), CALL_WEIGHT),
			Error::<Runtime>::NoMatchingCall
		);
		// Cannot remove active query that isn't expired
		assert_noop!(
			ForeignStateOracle::purge_expired_query(Origin::signed(BOB), 1),
			Error::<Runtime>::QueryNotExpired
		);
		// Fails when weight bound too low
		assert_noop!(
			ForeignStateOracle::respond_query_request(Origin::signed(1), 1, b"hello".to_vec(), 0),
			Error::<Runtime>::WrongRequestWeightBound
		);

		// Call is successfully dispatched with bytes injected into origin
		assert_ok!(ForeignStateOracle::respond_query_request(
			Origin::signed(1),
			1,
			b"hello".to_vec(),
			CALL_WEIGHT
		));
		System::assert_last_event(Event::ForeignStateOracle(crate::Event::CallDispatched {
			query_id: 1,
			task_result: Ok(()),
		}));
		System::assert_has_event(Event::QueryExample(mock::query_example::Event::OriginInjected {
			origin_data: b"hello".to_vec(),
			call_data: b"world".to_vec(),
		}));

		assert_ok!(QueryExample::mock_create_query(Origin::signed(ALICE), vec![], Some(10)));
		System::set_block_number(100);
		assert_noop!(
			ForeignStateOracle::respond_query_request(Origin::signed(ALICE), 2, b"hello".to_vec(), CALL_WEIGHT),
			Error::<Runtime>::QueryExpired
		);

		let bob_before = Balances::free_balance(BOB);
		assert_ok!(ForeignStateOracle::purge_expired_query(Origin::signed(BOB), 2));
		assert_eq!(bob_before.add(QueryFee::get().div(2u32)), Balances::free_balance(BOB))
	});
}

#[test]
fn create_query_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Correct event emitted when given expiry of None
		assert_ok!(QueryExample::mock_create_query(
			Origin::signed(ALICE),
			b"hi".to_vec(),
			None
		));
		System::assert_last_event(Event::ForeignStateOracle(crate::Event::QueryRequestCreated {
			expiry: None,
			query_id: 0,
		}));
		// Correct event emited when given expiry of Some()
		assert_ok!(QueryExample::mock_create_query(
			Origin::signed(ALICE),
			b"hi".to_vec(),
			Some(10)
		));
		System::assert_last_event(Event::ForeignStateOracle(crate::Event::QueryRequestCreated {
			expiry: Some(11),
			query_id: 1,
		}));
	});
}

#[test]
fn query_and_cancel_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Encoded call must be smaller than max size allowed.
		assert_noop!(
			QueryExample::mock_create_query(
				Origin::signed(ALICE),
				[0u8; MaxQueryCallSize::get() as usize].to_vec(),
				None
			),
			Error::<Runtime>::TooLargeForeignQueryRequest
		);
		// Need native token to query the oracle
		assert_noop!(
			QueryExample::mock_create_query(Origin::signed(BOB), vec![], None),
			pallet_balances::Error::<Runtime>::InsufficientBalance
		);

		let alice_before = Balances::free_balance(ALICE);
		assert_ok!(QueryExample::mock_create_query(Origin::signed(ALICE), vec![], None,));
		// Takes the query fee
		assert_eq!(alice_before, Balances::free_balance(ALICE).add(QueryFee::get()));

		assert_ok!(QueryExample::mock_cancel_query(Origin::none(), ALICE, 0));
		// Balance is restored other than the cancel fee
		assert_eq!(alice_before.sub(CancelFee::get()), Balances::free_balance(ALICE));
	});
}
