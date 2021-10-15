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

//! Unit tests for example module.

#![cfg(test)]

use crate::mock::*;
use frame_support::assert_ok;

#[test]
fn set_dummy_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Example::dummy(), None);
		assert_ok!(Example::set_dummy(Origin::root(), 20));
		assert_eq!(Example::dummy(), Some(20));
		System::assert_last_event(Event::Example(crate::Event::Dummy(20)));
	});
}

#[test]
fn do_set_bar_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Example::bar(2), 200);
		Example::do_set_bar(&2, 10);
		assert_eq!(Example::bar(2), 10);
	});
}
