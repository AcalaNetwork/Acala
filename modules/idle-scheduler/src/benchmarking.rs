// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<ScheduledTasks> {
	fn setup_schedule_task() -> Option<ScheduledTasks>;
}

impl<ScheduledTasks> BenchmarkHelper<ScheduledTasks> for () {
	fn setup_schedule_task() -> Option<ScheduledTasks> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize() {
		#[block]
		{
			Pallet::<T>::on_initialize(1u32.into());
		}
	}

	#[benchmark]
	fn on_idle_base() {
		let call = T::BenchmarkHelper::setup_schedule_task().unwrap();

		assert_ok!(Pallet::<T>::schedule_task(RawOrigin::Root.into(), call));

		#[block]
		{
			Pallet::<T>::on_idle(0u32.into(), Weight::from_parts(1_000_000_000, 0));
		}
	}

	#[benchmark]
	fn clear_tasks() {
		let call = T::BenchmarkHelper::setup_schedule_task().unwrap();

		let task_id = Pallet::<T>::get_next_task_id().unwrap();
		assert_ok!(Pallet::<T>::schedule_task(RawOrigin::Root.into(), call));

		let completed_tasks = vec![(
			task_id,
			TaskResult {
				result: Ok(()),
				used_weight: Weight::zero(),
				finished: true,
			},
		)];

		#[block]
		{
			Pallet::<T>::remove_completed_tasks(completed_tasks);
		}
	}

	#[benchmark]
	fn schedule_task() {
		let call = T::BenchmarkHelper::setup_schedule_task().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, call);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
