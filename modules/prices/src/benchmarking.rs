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
pub trait BenchmarkHelper<CurrencyId> {
	fn setup_feed_price() -> Option<CurrencyId>;
}

impl<CurrencyId> BenchmarkHelper<CurrencyId> for () {
	fn setup_feed_price() -> Option<CurrencyId> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn lock_price() {
		// feed price
		let currency_id = T::BenchmarkHelper::setup_feed_price().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, currency_id);
	}

	#[benchmark]
	fn unlock_price() {
		// feed price
		let currency_id = T::BenchmarkHelper::setup_feed_price().unwrap();

		assert_ok!(Pallet::<T>::lock_price(RawOrigin::Root.into(), currency_id));

		#[extrinsic_call]
		_(RawOrigin::Root, currency_id);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
