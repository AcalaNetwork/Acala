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
pub trait BenchmarkHelper {
	fn setup_parameter_store();
}

impl BenchmarkHelper for () {
	fn setup_parameter_store() {}
}

fn make_max_unbonding_chunk<T>(who: T::AccountId, amount: Balance)
where
	T: Config + frame_system::Config,
{
	frame_system::Pallet::<T>::set_block_number(0u32.into());
	let max_unlock_chunk: u32 = T::MaxUnbondingChunks::get();

	assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(who.clone()).into(), 100 * amount));

	for _ in 0..(max_unlock_chunk) {
		frame_system::Pallet::<T>::set_block_number(frame_system::Pallet::<T>::block_number() + 1u32.into());
		assert_ok!(Pallet::<T>::unbond(RawOrigin::Signed(who.clone()).into(), amount));
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn bond() {
		T::BenchmarkHelper::setup_parameter_store();

		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		let _ = T::Currency::make_free_balance_be(&caller, amount);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::Bonded {
				who: caller.clone(),
				amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn unbond() {
		T::BenchmarkHelper::setup_parameter_store();

		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		let _ = T::Currency::make_free_balance_be(&caller, amount);

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::Unbonded {
				who: caller.clone(),
				amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn unbond_instant() {
		T::BenchmarkHelper::setup_parameter_store();

		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		let _ = T::Currency::make_free_balance_be(&caller, amount);

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount);

		let fee = Pallet::<T>::get_instant_unstake_fee().unwrap().mul_ceil(amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::InstantUnbonded {
				who: caller,
				amount: amount - fee,
				fee,
			}
			.into(),
		);
	}

	#[benchmark]
	fn rebond() {
		T::BenchmarkHelper::setup_parameter_store();

		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		let _ = T::Currency::make_free_balance_be(&caller, 100 * amount);

		make_max_unbonding_chunk::<T>(caller.clone(), amount);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), 10 * amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::Rebonded {
				who: caller.clone(),
				amount: amount.saturating_mul(T::MaxUnbondingChunks::get().into()),
			}
			.into(),
		);
	}

	#[benchmark]
	fn withdraw_unbonded() {
		T::BenchmarkHelper::setup_parameter_store();

		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		let _ = T::Currency::make_free_balance_be(&caller, 100 * amount);

		make_max_unbonding_chunk::<T>(caller.clone(), amount);

		// large number to unlock all chunks
		frame_system::Pallet::<T>::set_block_number(1_000_000u32.into());

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			Event::Withdrawn {
				who: caller,
				amount: amount.saturating_mul(T::MaxUnbondingChunks::get().into()),
			}
			.into(),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
