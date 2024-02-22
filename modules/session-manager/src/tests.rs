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

//! Unit tests for session-manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{new_test_ext, Runtime, RuntimeEvent, RuntimeOrigin, Session, SessionManager, System};

#[test]
fn schedule_session_duration_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(System::block_number(), 1);
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);

		assert_noop!(
			SessionManager::schedule_session_duration(RuntimeOrigin::root(), 0, 0),
			Error::<Runtime>::InvalidSession
		);
		assert_noop!(
			SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 0),
			Error::<Runtime>::InvalidDuration
		);

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 10));
		System::assert_last_event(RuntimeEvent::SessionManager(crate::Event::ScheduledSessionDuration {
			block_number: 1,
			session_index: 1,
			session_duration: 10,
		}));
		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		System::assert_last_event(RuntimeEvent::SessionManager(crate::Event::ScheduledSessionDuration {
			block_number: 10,
			session_index: 1,
			session_duration: 11,
		}));

		SessionDuration::<Runtime>::put(0);
		assert_noop!(
			SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 12),
			Error::<Runtime>::EstimateNextSessionFailed
		);
	});
}

#[test]
fn on_initialize_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		System::assert_last_event(RuntimeEvent::SessionManager(crate::Event::ScheduledSessionDuration {
			block_number: 10,
			session_index: 1,
			session_duration: 11,
		}));
		assert_eq!(SessionDurationChanges::<Runtime>::iter().count(), 1);

		SessionManager::on_initialize(9);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		SessionManager::on_initialize(10);
		assert_eq!(SessionDurationChanges::<Runtime>::iter().count(), 0);
		assert_eq!(SessionManager::session_duration(), 11);
		assert_eq!(SessionManager::duration_offset(), 10);
	});
}

#[test]
fn should_end_session_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		assert!(!SessionManager::should_end_session(9));
		assert!(SessionManager::should_end_session(10));

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		SessionManager::on_initialize(10);
		assert_eq!(SessionManager::session_duration(), 11);
		assert_eq!(SessionManager::duration_offset(), 10);

		assert!(!SessionManager::should_end_session(9));
		assert!(SessionManager::should_end_session(10));
		assert!(!SessionManager::should_end_session(11));
		assert!(!SessionManager::should_end_session(20));
		assert!(SessionManager::should_end_session(21));
	});
}

#[test]
fn average_session_length_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		assert_eq!(SessionManager::average_session_length(), 10);

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		SessionManager::on_initialize(10);
		assert_eq!(SessionManager::average_session_length(), 11);
	});
}

#[test]
fn estimate_current_session_progress_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		assert_eq!(
			SessionManager::estimate_current_session_progress(0).0,
			Some(Permill::from_rational(1u32, 10u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(8).0,
			Some(Permill::from_rational(9u32, 10u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(9).0,
			Some(Permill::from_rational(10u32, 10u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(10).0,
			Some(Permill::from_rational(1u32, 10u32))
		);

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		SessionManager::on_initialize(10);
		assert_eq!(SessionManager::session_duration(), 11);
		assert_eq!(SessionManager::duration_offset(), 10);

		assert_eq!(SessionManager::estimate_current_session_progress(8).0, None);
		assert_eq!(SessionManager::estimate_current_session_progress(9).0, None);
		assert_eq!(
			SessionManager::estimate_current_session_progress(10).0,
			Some(Permill::from_rational(1u32, 11u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(11).0,
			Some(Permill::from_rational(2u32, 11u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(12).0,
			Some(Permill::from_rational(3u32, 11u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(30).0,
			Some(Permill::from_rational(10u32, 11u32))
		);
		assert_eq!(
			SessionManager::estimate_current_session_progress(31).0,
			Some(Permill::from_rational(11u32, 11u32))
		);
	});
}

#[test]
fn estimate_next_session_rotation_work() {
	new_test_ext().execute_with(|| {
		assert_eq!(Session::session_index(), 0);
		assert_eq!(SessionManager::session_duration(), 10);
		assert_eq!(SessionManager::duration_offset(), 0);

		assert_eq!(SessionManager::estimate_next_session_rotation(0).0, Some(0));
		assert_eq!(SessionManager::estimate_next_session_rotation(8).0, Some(10));
		assert_eq!(SessionManager::estimate_next_session_rotation(9).0, Some(10));
		assert_eq!(SessionManager::estimate_next_session_rotation(10).0, Some(20));

		assert_ok!(SessionManager::schedule_session_duration(RuntimeOrigin::root(), 1, 11));
		SessionManager::on_initialize(10);
		assert_eq!(SessionManager::session_duration(), 11);
		assert_eq!(SessionManager::duration_offset(), 10);

		assert_eq!(SessionManager::estimate_next_session_rotation(8).0, Some(10));
		assert_eq!(SessionManager::estimate_next_session_rotation(9).0, Some(10));
		assert_eq!(SessionManager::estimate_next_session_rotation(10).0, Some(10));
		assert_eq!(SessionManager::estimate_next_session_rotation(11).0, Some(21));
		assert_eq!(SessionManager::estimate_next_session_rotation(12).0, Some(21));
		assert_eq!(SessionManager::estimate_next_session_rotation(21).0, Some(32));
	});
}
