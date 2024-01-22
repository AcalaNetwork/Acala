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

use crate::{Runtime, RuntimeEvent, SessionManager, System};

use frame_support::{
	assert_ok,
	traits::{EstimateNextSessionRotation, OnInitialize},
};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;

fn assert_last_event(generic_event: RuntimeEvent) {
	System::assert_last_event(generic_event.into());
}

runtime_benchmarks! {
	{ Runtime, module_session_manager }

	schedule_session_duration {
		System::set_block_number(2u32.into());
		module_session_manager::SessionDuration::<Runtime>::put(10);
	}: {
		assert_ok!(
			SessionManager::schedule_session_duration(RawOrigin::Root.into(), 1, 100)
		);
	}
	verify {
		assert_last_event(module_session_manager::Event::ScheduledSessionDuration{block_number: 10, session_index: 1, session_duration: 100}.into());
	}

	on_initialize_skip {
		System::set_block_number(2u32.into());
		module_session_manager::SessionDuration::<Runtime>::put(10);
		SessionManager::schedule_session_duration(RawOrigin::Root.into(), 1, 100)?;
	}: {
		SessionManager::on_initialize(9)
	}

	on_initialize {
		System::set_block_number(2u32.into());
		module_session_manager::SessionDuration::<Runtime>::put(10);
		SessionManager::schedule_session_duration(RawOrigin::Root.into(), 1, 100)?;
	}: {
		SessionManager::on_initialize(10)
	}

	estimate_current_session_progress {
		module_session_manager::SessionDuration::<Runtime>::put(10);
	}: {
		SessionManager::estimate_current_session_progress(10)
	}

	estimate_next_session_rotation {
		module_session_manager::SessionDuration::<Runtime>::put(10);
	}: {
		SessionManager::estimate_next_session_rotation(10)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
