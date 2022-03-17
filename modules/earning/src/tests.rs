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
		assert_eq!(Balances::reducible_balance(&ALICE, false), 900);

		assert_ok!(Earning::bond(Origin::signed(ALICE), 1000));
		System::assert_last_event(
			Event::Bonded {
				who: ALICE,
				amount: 900,
			}
			.into(),
		);
		assert_eq!(Balances::reducible_balance(&ALICE, false), 0);
	});
}
