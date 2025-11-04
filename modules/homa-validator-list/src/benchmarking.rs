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
pub trait BenchmarkHelper<EraIndex> {
	fn setup_homa_bump_era(era_index: EraIndex);
}

impl<EraIndex> BenchmarkHelper<EraIndex> for () {
	fn setup_homa_bump_era(_era_index: EraIndex) {}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn bond() {
		let caller: T::AccountId = account("caller", 0, 0);
		let validator: T::ValidatorId = account("validator", 0, 0);
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 2 * amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), validator.clone(), amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::BondGuarantee {
				who: caller,
				validator: validator.clone(),
				bond: amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn unbond() {
		let caller: T::AccountId = account("caller", 0, 0);
		let validator: T::ValidatorId = account("validator", 0, 0);
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 2 * amount));

		assert_ok!(Pallet::<T>::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			amount
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), validator.clone(), amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::UnbondGuarantee {
				who: caller,
				validator: validator.clone(),
				bond: amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn rebond() {
		let caller: T::AccountId = account("caller", 0, 0);
		let validator: T::ValidatorId = account("validator", 0, 0);
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 10 * amount));

		assert_ok!(Pallet::<T>::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			10 * amount
		));

		assert_ok!(Pallet::<T>::unbond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			5 * amount
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), validator.clone(), 5 * amount);

		frame_system::Pallet::<T>::assert_last_event(
			Event::RebondGuarantee {
				who: caller,
				validator: validator.clone(),
				bond: 5 * amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn withdraw_unbonded() {
		let caller: T::AccountId = account("caller", 0, 0);
		let validator: T::ValidatorId = account("validator", 0, 0);
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 10 * amount));

		assert_ok!(Pallet::<T>::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			10 * amount
		));

		assert_ok!(Pallet::<T>::unbond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			5 * amount
		));

		T::BenchmarkHelper::setup_homa_bump_era(T::CurrentEra::get().saturating_add(T::BondingDuration::get()));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), validator.clone());

		frame_system::Pallet::<T>::assert_has_event(
			Event::WithdrawnGuarantee {
				who: caller,
				validator: validator.clone(),
				bond: 5 * amount,
			}
			.into(),
		);
	}

	#[benchmark]
	fn freeze(n: Linear<1, 10>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let mut validators: Vec<T::ValidatorId> = vec![];
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 100 * amount));

		for i in 0..n {
			let validator: T::ValidatorId = account("validator", i, 0);

			assert_ok!(Pallet::<T>::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				amount
			));

			validators.push(validator);
		}

		#[extrinsic_call]
		_(RawOrigin::Root, validators.clone());

		for validator in validators {
			frame_system::Pallet::<T>::assert_has_event(
				Event::FreezeValidator {
					validator: validator.clone(),
				}
				.into(),
			);
		}
	}

	#[benchmark]
	fn thaw(n: Linear<1, 10>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let mut validators: Vec<T::ValidatorId> = vec![];
		let amount = T::MinBondAmount::get();

		assert_ok!(T::LiquidTokenCurrency::deposit(&caller, 100 * amount));

		for i in 0..n {
			let validator: T::ValidatorId = account("validator", i, 0);

			assert_ok!(Pallet::<T>::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				amount
			));

			validators.push(validator);
		}

		assert_ok!(Pallet::<T>::freeze(RawOrigin::Root.into(), validators.clone()));

		#[extrinsic_call]
		_(RawOrigin::Root, validators.clone());

		for validator in validators {
			frame_system::Pallet::<T>::assert_has_event(
				Event::ThawValidator {
					validator: validator.clone(),
				}
				.into(),
			);
		}
	}

	#[benchmark]
	fn slash(n: Linear<1, 10>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let mut slashes: Vec<SlashInfo<Balance, T::ValidatorId>> = vec![];

		assert_ok!(T::LiquidTokenCurrency::deposit(
			&caller,
			100 * T::ValidatorInsuranceThreshold::get()
		));

		for i in 0..n {
			let validator: T::ValidatorId = account("validator", i, 0);

			assert_ok!(Pallet::<T>::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				T::ValidatorInsuranceThreshold::get() * 10
			));

			slashes.push(SlashInfo {
				validator,
				token_amount: T::ValidatorInsuranceThreshold::get() * 9,
			});
		}

		#[extrinsic_call]
		_(RawOrigin::Root, slashes.clone());
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
