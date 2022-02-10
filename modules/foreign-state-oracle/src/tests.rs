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
use mock::{Call as CallOf, Origin as OriginOf, *};
use sp_runtime::traits::{BlakeTwo256, Hash};

#[test]
fn dispatch_call_test() {
	ExtBuilder::default().build().execute_with(|| {
		let call = CallOf::QueryExample(query_example::Call::injected_call {});

		assert_ok!(query_example::Pallet::<Runtime>::example_query_call(&ALICE));

		assert_ok!(RelaychainOracle::dispatch_task(
			OriginOf::signed(ALICE),
			0,
			b"hello".to_vec()
		));
	});
}
