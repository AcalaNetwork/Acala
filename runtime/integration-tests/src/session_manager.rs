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

use crate::setup::*;
use frame_support::traits::ValidatorSet;

#[test]
fn test_session_manager_module() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		run_to_block(10);
		assert_eq!(Session::session_index(), 1);
		assert_eq!(SessionManager::session_duration(), 10);

		assert_ok!(SessionManager::schedule_session_duration(RawOrigin::Root.into(), 2, 11));

		run_to_block(19);
		assert_eq!(Session::session_index(), 1);
		assert_eq!(SessionManager::session_duration(), 10);

		run_to_block(20);
		assert_eq!(Session::session_index(), 2);
		assert_eq!(SessionManager::session_duration(), 11);

		run_to_block(31);
		assert_eq!(Session::session_index(), 3);
		assert_eq!(SessionManager::session_duration(), 11);

		assert_ok!(SessionManager::schedule_session_duration(RawOrigin::Root.into(), 4, 9));

		run_to_block(42);
		assert_eq!(Session::session_index(), 4);
		assert_eq!(SessionManager::session_duration(), 9);

		run_to_block(50);
		assert_eq!(Session::session_index(), 4);
		assert_eq!(SessionManager::session_duration(), 9);

		run_to_block(51);
		assert_eq!(Session::session_index(), 5);
		assert_eq!(SessionManager::session_duration(), 9);
	});
}
