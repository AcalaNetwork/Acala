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

//! Unit tests for fee distribution module.

#![cfg(test)]

use super::*;
use crate::mock::*;
use frame_support::assert_ok;
use mock::{Event, ExtBuilder, Origin, Runtime, System};

#[test]
fn set_income_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, vec![]));

		System::assert_last_event(Event::Fees(crate::Event::IncomeFeeSet {
			income: IncomeSource::TxFee,
			pools: vec![],
		}));
	});
}
