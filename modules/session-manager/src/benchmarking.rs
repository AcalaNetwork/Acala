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

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn schedule_session_duration() {
		let block_number: BlockNumberFor<T> = 10u32.into();
		SessionDuration::<T>::put(block_number);

		frame_system::Pallet::<T>::set_block_number(2u32.into());

		#[block]
		{
			assert_ok!(Pallet::<T>::schedule_session_duration(
				RawOrigin::Root.into(),
				1u32.into(),
				100u32.into()
			));
		}

		frame_system::Pallet::<T>::assert_last_event(
			Event::ScheduledSessionDuration {
				block_number: block_number,
				session_index: 1u32.into(),
				session_duration: 100u32.into(),
			}
			.into(),
		);
	}

	#[benchmark]
	fn on_initialize_skip() {
		let block_number: BlockNumberFor<T> = 10u32.into();
		SessionDuration::<T>::put(block_number);

		frame_system::Pallet::<T>::set_block_number(2u32.into());

		assert_ok!(Pallet::<T>::schedule_session_duration(
			RawOrigin::Root.into(),
			1u32.into(),
			100u32.into()
		));

		#[block]
		{
			Pallet::<T>::on_initialize(9u32.into());
		}
	}

	#[benchmark]
	fn on_initialize() {
		let block_number: BlockNumberFor<T> = 10u32.into();
		SessionDuration::<T>::put(block_number);

		frame_system::Pallet::<T>::set_block_number(2u32.into());

		assert_ok!(Pallet::<T>::schedule_session_duration(
			RawOrigin::Root.into(),
			1u32.into(),
			100u32.into()
		));

		#[block]
		{
			Pallet::<T>::on_initialize(10u32.into());
		}
	}

	#[benchmark]
	fn estimate_current_session_progress() {
		let block_number: BlockNumberFor<T> = 10u32.into();
		SessionDuration::<T>::put(block_number);

		#[block]
		{
			Pallet::<T>::estimate_current_session_progress(10u32.into());
		}
	}

	#[benchmark]
	fn estimate_next_session_rotation() {
		let block_number: BlockNumberFor<T> = 10u32.into();
		SessionDuration::<T>::put(block_number);

		#[block]
		{
			Pallet::<T>::estimate_next_session_rotation(10u32.into());
		}
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
