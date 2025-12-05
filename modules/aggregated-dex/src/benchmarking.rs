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
use frame_system::RawOrigin;
use sp_std::vec;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<AccountId, CurrencyId, Balance> {
	fn setup_currency_lists() -> Vec<CurrencyId>;
	// return (path, supply_amount, target_amount)
	fn setup_dex(u: u32, taker: AccountId) -> Option<(Vec<CurrencyId>, Balance, Balance)>;
}

impl<AccountId, CurrencyId, Balance> BenchmarkHelper<AccountId, CurrencyId, Balance> for () {
	fn setup_currency_lists() -> Vec<CurrencyId> {
		vec![]
	}
	fn setup_dex(_u: u32, _taker: AccountId) -> Option<(Vec<CurrencyId>, Balance, Balance)> {
		None
	}
}

#[benchmarks(
	where
	T: Config + module_dex::Config
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn swap_with_exact_supply(u: Linear<2, { <T as module_dex::Config>::TradingPathLimit::get() }>) {
		let taker: T::AccountId = account("taker", 0, 0);

		let (path, supply_amount, _target_amount) =
			<T as Config>::BenchmarkHelper::setup_dex(u, taker.clone()).unwrap();

		#[extrinsic_call]
		_(
			RawOrigin::Signed(taker),
			vec![SwapPath::Dex(path.clone())],
			supply_amount,
			0,
		);
	}

	#[benchmark]
	fn swap_with_exact_target(u: Linear<2, { <T as module_dex::Config>::TradingPathLimit::get() }>) {
		let taker: T::AccountId = account("taker", 0, 0);

		let (path, supply_amount, target_amount) = <T as Config>::BenchmarkHelper::setup_dex(u, taker.clone()).unwrap();

		#[extrinsic_call]
		_(
			RawOrigin::Signed(taker),
			vec![SwapPath::Dex(path.clone())],
			target_amount,
			supply_amount,
		);
	}

	#[benchmark]
	fn update_aggregated_swap_paths(
		n: Linear<0, { <T as Config>::BenchmarkHelper::setup_currency_lists().len() as u32 }>,
	) {
		let currency_lists = <T as Config>::BenchmarkHelper::setup_currency_lists();
		let mut updates: Vec<((CurrencyId, CurrencyId), Option<Vec<SwapPath>>)> = vec![];
		for i in 1..n {
			let token_a = currency_lists[i as usize];
			updates.push((
				(token_a, currency_lists[0]),
				Some(vec![SwapPath::Dex(vec![token_a, currency_lists[0]])]),
			));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
