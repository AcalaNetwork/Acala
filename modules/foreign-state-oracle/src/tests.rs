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
use mock::{Call, Event, Origin, *};
use sp_std::ops::{Add, Sub};

#[test]
fn dispatch_and_remove_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(query_example::Pallet::<Runtime>::example_query_call(ALICE));
		assert!(ActiveQuery::<Runtime>::get(0).is_some());
		assert_ok!(ForeignStateOracle::cancel_task(&ALICE, 0));
		assert!(ActiveQuery::<Runtime>::get(0).is_none());

		assert_ok!(query_example::Pallet::<Runtime>::example_query_call(ALICE));
		assert_noop!(
			ForeignStateOracle::dispatch_task(Origin::signed(1), 0, b"hello".to_vec()),
			Error::<Runtime>::NoMatchingCall
		);
		// Cannot remove active query
		assert_noop!(
			ForeignStateOracle::remove_expired_call(Origin::signed(BOB), 1),
			Error::<Runtime>::QueryNotExpired
		);

		// Call is sucessfully dispatched with bytes injected into origin
		assert_ok!(ForeignStateOracle::dispatch_task(
			Origin::signed(1),
			1,
			b"hello".to_vec()
		));
		System::assert_last_event(Event::ForeignStateOracle(crate::Event::CallDispatched {
			task_result: Ok(()),
		}));
		System::assert_has_event(Event::QueryExample(mock::query_example::Event::OriginInjected {
			origin_data: b"hello".to_vec(),
		}));

		assert_ok!(query_example::Pallet::<Runtime>::example_query_call(ALICE));
		System::set_block_number(11);
		assert_noop!(
			ForeignStateOracle::dispatch_task(Origin::signed(1), 2, b"hello".to_vec()),
			Error::<Runtime>::QueryExpired
		);

		let bob_before = Balances::free_balance(BOB);
		assert_ok!(ForeignStateOracle::remove_expired_call(Origin::signed(BOB), 2));
		assert_eq!(bob_before.add(QueryFee::get().div(2u32)), Balances::free_balance(BOB))
	});
}

#[test]
fn query_and_cancel_works() {
	ExtBuilder::default().build().execute_with(|| {
		let call = Call::QueryExample(query_example::Call::injected_call {});
		// Bound can't be smaller than encoded call length
		assert_noop!(
			ForeignStateOracle::query_task(&ALICE, 1, call.clone()),
			Error::<Runtime>::TooLargeVerifiableCall
		);
		// Need native token to query the oracle
		assert_noop!(
			ForeignStateOracle::query_task(&BOB, call.using_encoded(|x| x.len()), call.clone()),
			pallet_balances::Error::<Runtime>::InsufficientBalance
		);

		let alice_before = Balances::free_balance(ALICE);
		assert_ok!(ForeignStateOracle::query_task(
			&ALICE,
			call.using_encoded(|x| x.len()),
			call.clone()
		));
		// Takes the query fee
		assert_eq!(alice_before, Balances::free_balance(ALICE).add(QueryFee::get()));

		assert_ok!(ForeignStateOracle::cancel_task(&ALICE, 0));
		// Balance is restored other than the cancel fee
		assert_eq!(alice_before.sub(CancelFee::get()), Balances::free_balance(ALICE));
	});
}
