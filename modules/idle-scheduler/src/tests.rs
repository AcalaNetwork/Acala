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

//! Unit tests for idle-scheduler module.

#![cfg(test)]

use super::*;
use crate::mock::{IdleScheduler, RuntimeEvent, *};
use frame_support::assert_ok;

// Can schedule tasks
#[test]
fn can_schedule_tasks() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Tasks::<Runtime>::get(0), None);

		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::BalancesTask(BalancesTask::OnIdle)
		));
		assert_eq!(
			Tasks::<Runtime>::get(0),
			Some(ScheduledTasks::BalancesTask(BalancesTask::OnIdle))
		);
		System::assert_has_event(RuntimeEvent::IdleScheduler(crate::Event::TaskAdded {
			task_id: 0,
			task: ScheduledTasks::BalancesTask(BalancesTask::OnIdle),
		}));

		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::HomaLiteTask(HomaLiteTask::OnIdle)
		));
		assert_eq!(
			Tasks::<Runtime>::get(1),
			Some(ScheduledTasks::HomaLiteTask(HomaLiteTask::OnIdle))
		);

		assert_eq!(Tasks::<Runtime>::get(2), None);
	});
}

// can process tasks up to weight limit
#[test]
fn can_process_tasks_up_to_weight_limit() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::BalancesTask(BalancesTask::OnIdle)
		));
		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::BalancesTask(BalancesTask::OnIdle)
		));
		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::HomaLiteTask(HomaLiteTask::OnIdle)
		));

		// Given enough weights for only 2 tasks: MinimumWeightRemainInBlock::get() + BASE_WEIGHT*2 +
		// on_idle_base()
		IdleScheduler::on_idle(
			0,
			Weight::from_parts(100_002_000_000, 0) + <()>::on_idle_base() + (<()>::clear_tasks() * 2),
		);

		// Due to hashing, excution is not guaranteed to be in order.
		assert_eq!(
			Tasks::<Runtime>::get(0),
			Some(ScheduledTasks::BalancesTask(BalancesTask::OnIdle))
		);
		assert_eq!(Tasks::<Runtime>::get(1), None);
		assert_eq!(Tasks::<Runtime>::get(2), None);

		IdleScheduler::on_idle(0, Weight::from_parts(100_000_000_000, 0) + <()>::on_idle_base());
		assert_eq!(
			Tasks::<Runtime>::get(0),
			Some(ScheduledTasks::BalancesTask(BalancesTask::OnIdle))
		);

		IdleScheduler::on_idle(0, Weight::from_parts(100_001_000_000, 0) + <()>::on_idle_base());
		assert_eq!(Tasks::<Runtime>::get(0), None);
	});
}

// can increment next task ID
#[test]
fn can_increment_next_task_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(NextTaskId::<Runtime>::get(), 0);
		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::BalancesTask(BalancesTask::OnIdle)
		));

		assert_eq!(NextTaskId::<Runtime>::get(), 1);
	});
}

#[test]
fn on_idle_works() {
	ExtBuilder::default().build().execute_with(|| {
		IdleScheduler::on_initialize(0);
		assert_ok!(IdleScheduler::schedule_task(
			RuntimeOrigin::root(),
			ScheduledTasks::BalancesTask(BalancesTask::OnIdle)
		));
		// simulate relay block number jumping 10 blocks
		sp_io::storage::set(&RELAY_BLOCK_KEY, &10_u32.encode());
		assert_eq!(IdleScheduler::on_idle(System::block_number(), Weight::MAX), Weight::MAX);

		System::set_block_number(1);
		IdleScheduler::on_initialize(1);
		// On_initialize is called it will execute, as now relay block number is the same
		assert_eq!(
			IdleScheduler::on_idle(System::block_number(), Weight::MAX),
			BASE_WEIGHT + <()>::on_idle_base() + <()>::clear_tasks()
		);
		assert!(!PreviousRelayBlockNumber::<Runtime>::exists());
	});
}
