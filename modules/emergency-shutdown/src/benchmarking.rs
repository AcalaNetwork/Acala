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
	fn setup_feed_price(c: u32);
}

impl BenchmarkHelper for () {
	fn setup_feed_price(_c: u32) {}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn emergency_shutdown(c: Linear<0, 10>) {
		T::BenchmarkHelper::setup_feed_price(c);

		#[extrinsic_call]
		_(RawOrigin::Root);
	}

	#[benchmark]
	fn open_collateral_refund() {
		assert_ok!(Pallet::<T>::emergency_shutdown(RawOrigin::Root.into()));

		#[extrinsic_call]
		_(RawOrigin::Root);

		frame_system::Pallet::<T>::assert_last_event(
			Event::OpenRefund {
				block_number: <frame_system::Pallet<T>>::block_number(),
			}
			.into(),
		);
	}

	#[benchmark]
	fn refund_collaterals(c: Linear<0, 10>) {
		T::BenchmarkHelper::setup_feed_price(c);

		let caller: T::AccountId = account("caller", 0, 0);
		let funder: T::AccountId = account("funder", 0, 0);
		let amount: Balance = 100_000_000_000_000_000_000u128.into();

		assert_ok!(<T as Config>::CDPTreasury::issue_debit(&caller, amount, true));
		assert_ok!(<T as Config>::CDPTreasury::issue_debit(&funder, amount, true));

		assert_ok!(Pallet::<T>::emergency_shutdown(RawOrigin::Root.into()));
		assert_ok!(Pallet::<T>::open_collateral_refund(RawOrigin::Root.into()));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), amount);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
