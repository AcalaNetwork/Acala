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
pub trait BenchmarkHelper<CurrencyId, Moment> {
	fn setup_on_initialize(n: u32, u: u32);
	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)>;
}

impl<CurrencyId, Moment> BenchmarkHelper<CurrencyId, Moment> for () {
	fn setup_on_initialize(_n: u32, _u: u32) {}
	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize_with_update_average_prices(n: Linear<0, 3>, u: Linear<0, 3>) {
		T::BenchmarkHelper::setup_on_initialize(n, u);

		#[block]
		{
			Pallet::<T>::on_initialize(1u32.into());
		}
	}

	#[benchmark]
	fn enable_average_price() {
		let (currency_id_a, currency_id_b, interval) = T::BenchmarkHelper::setup_inject_liquidity().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, currency_id_a, currency_id_b, interval);
	}

	#[benchmark]
	fn disable_average_price() {
		let (currency_id_a, currency_id_b, interval) = T::BenchmarkHelper::setup_inject_liquidity().unwrap();

		assert_ok!(Pallet::<T>::enable_average_price(
			RawOrigin::Root.into(),
			currency_id_a,
			currency_id_b,
			interval
		));

		#[extrinsic_call]
		_(RawOrigin::Root, currency_id_a, currency_id_b);
	}

	#[benchmark]
	fn update_average_price_interval() {
		let (currency_id_a, currency_id_b, interval) = T::BenchmarkHelper::setup_inject_liquidity().unwrap();

		assert_ok!(Pallet::<T>::enable_average_price(
			RawOrigin::Root.into(),
			currency_id_a,
			currency_id_b,
			interval
		));

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			currency_id_a,
			currency_id_b,
			interval.saturating_mul(10u32.into()),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
