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
use sp_runtime::SaturatedConversion;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<EraIndex, AccountId, NomineeId> {
	fn setup_homa_bump_era(era_index: EraIndex);
	fn setup_homa_validators(caller: AccountId, targets: Vec<NomineeId>);
}

impl<EraIndex, AccountId, NomineeId> BenchmarkHelper<EraIndex, AccountId, NomineeId> for () {
	fn setup_homa_bump_era(_era_index: EraIndex) {}
	fn setup_homa_validators(_caller: AccountId, _targets: Vec<NomineeId>) {}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn bond() {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		assert_ok!(T::Currency::deposit(&caller, 2 * amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), amount);
	}

	#[benchmark]
	fn unbond() {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		assert_ok!(T::Currency::deposit(&caller, 2 * amount));

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount);
	}

	#[benchmark]
	fn rebond(c: Linear<1, { T::MaxUnbondingChunks::get() }>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		assert_ok!(T::Currency::deposit(&caller, 2 * amount));

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), 2 * amount));

		for _ in 0..c {
			assert_ok!(Pallet::<T>::unbond(
				RawOrigin::Signed(caller.clone()).into(),
				amount.saturating_div(c.into())
			));
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), amount);
	}

	#[benchmark]
	fn withdraw_unbonded(c: Linear<1, { T::MaxUnbondingChunks::get() }>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();

		assert_ok!(T::Currency::deposit(&caller, 2 * amount));

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), 2 * amount));

		for _ in 0..c {
			assert_ok!(Pallet::<T>::unbond(
				RawOrigin::Signed(caller.clone()).into(),
				amount.saturating_div(c.into())
			));
		}

		T::BenchmarkHelper::setup_homa_bump_era(T::CurrentEra::get().saturating_add(T::BondingDuration::get()));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			Event::WithdrawUnbonded {
				who: caller,
				amount: amount.saturating_div(c.into()).saturating_mul(c.into()),
			}
			.into(),
		);
	}

	#[benchmark]
	fn nominate(c: Linear<1, { T::MaxNominateesCount::get() }>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();
		let targets: Vec<T::NomineeId> = (0..c).map(|c| account("nominatees", c, 0)).collect();

		assert_ok!(T::Currency::deposit(&caller, 100_000_000 * amount));

		T::BenchmarkHelper::setup_homa_validators(caller.clone(), targets.clone());

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), targets.clone());

		let mut sorted_targets = targets.clone();
		sorted_targets.sort();

		frame_system::Pallet::<T>::assert_last_event(
			Event::Nominate {
				who: caller,
				targets: sorted_targets,
			}
			.into(),
		);
	}

	#[benchmark]
	fn chill(c: Linear<1, { T::MaxNominateesCount::get() }>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MinBond::get();
		let targets: Vec<T::NomineeId> = (0..c).map(|c| account("nominatees", c, 0)).collect();

		assert_ok!(T::Currency::deposit(&caller, 100_000_000 * amount));

		T::BenchmarkHelper::setup_homa_validators(caller.clone(), targets.clone());

		assert_ok!(Pallet::<T>::bond(RawOrigin::Signed(caller.clone()).into(), amount));

		assert_ok!(Pallet::<T>::nominate(RawOrigin::Signed(caller.clone()).into(), targets));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()));

		frame_system::Pallet::<T>::assert_last_event(
			Event::Nominate {
				who: caller,
				targets: vec![],
			}
			.into(),
		);
	}

	#[benchmark]
	fn reset_reserved_nominees(c: Linear<1, 4>) {
		let updates: Vec<(u16, BoundedVec<T::NomineeId, T::MaxNominateesCount>)> = (0..c)
			.map(|c| {
				let reserved: BoundedVec<T::NomineeId, T::MaxNominateesCount> = (0..T::MaxNominateesCount::get())
					.map(|c| account("nominatees", c, 0))
					.collect::<Vec<T::NomineeId>>()
					.try_into()
					.unwrap();

				(c.saturated_into(), reserved)
			})
			.collect();

		#[extrinsic_call]
		_(RawOrigin::Root, updates);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
