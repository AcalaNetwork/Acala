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

use crate::{EvmTask, IdleScheduler, Runtime, RuntimeOrigin, ScheduledTasks, Weight, H160};
use frame_support::traits::{OnIdle, OnInitialize};
use orml_benchmarking::runtime_benchmarks;
use primitives::task::TaskResult;

runtime_benchmarks! {
	{ Runtime, module_idle_scheduler}

	on_initialize {
	}: {
		IdleScheduler::on_initialize(1);
	}

	on_idle_base {
	}: {
		IdleScheduler::on_idle(0, Weight::from_parts(1_000_000_000, 0));
	}

	clear_tasks {
		let dummy_hash = [0; 20];
		let call = ScheduledTasks::EvmTask(EvmTask::Remove{caller: H160::from(&dummy_hash), contract: H160::from(&dummy_hash), maintainer: H160::from(&dummy_hash)});
		IdleScheduler::schedule_task(RuntimeOrigin::root(), call)?;
		let completed_tasks = vec![(0, TaskResult{ result: Ok(()), used_weight: Weight::zero(), finished: true })];
	}: {
		IdleScheduler::remove_completed_tasks(completed_tasks);
	}

	schedule_task {
		let dummy_hash = [0; 20];
		let call = ScheduledTasks::EvmTask(EvmTask::Remove{caller: H160::from(&dummy_hash), contract: H160::from(&dummy_hash), maintainer: H160::from(&dummy_hash)});
	}: _(RuntimeOrigin::root(), call)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
