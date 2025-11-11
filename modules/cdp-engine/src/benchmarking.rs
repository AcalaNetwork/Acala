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
pub trait BenchmarkHelper<CurrencyId, BlockNumber, LookupSource> {
	fn setup_on_initialize(c: u32) -> Option<BlockNumber>;
	fn setup_liquidate_by_auction(b: u32) -> Option<(CurrencyId, LookupSource)>;
	fn setup_liquidate_by_dex() -> Option<(CurrencyId, LookupSource)>;
	fn setup_settle() -> Option<(CurrencyId, LookupSource)>;
}

impl<CurrencyId, BlockNumber, LookupSource> BenchmarkHelper<CurrencyId, BlockNumber, LookupSource> for () {
	fn setup_on_initialize(_c: u32) -> Option<BlockNumber> {
		None
	}
	fn setup_liquidate_by_auction(_b: u32) -> Option<(CurrencyId, LookupSource)> {
		None
	}
	fn setup_liquidate_by_dex() -> Option<(CurrencyId, LookupSource)> {
		None
	}
	fn setup_settle() -> Option<(CurrencyId, LookupSource)> {
		None
	}
}

#[benchmarks(
	where T: Config + module_cdp_treasury::Config,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize(c: Linear<0, 10>) {
		let block_number = <T as Config>::BenchmarkHelper::setup_on_initialize(c).unwrap();

		#[block]
		{
			Pallet::<T>::on_initialize(block_number);
		}
	}

	#[benchmark]
	fn set_collateral_params() {
		#[extrinsic_call]
		_(
			RawOrigin::Root,
			<T as Config>::GetStableCurrencyId::get(),
			Change::NewValue(Some(Rate::saturating_from_rational(1, 1000000))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(180, 100))),
			Change::NewValue(100_000),
		);
	}

	// `liquidate` by_auction
	#[benchmark]
	fn liquidate_by_auction(b: Linear<1, { T::MaxAuctionsCount::get() }>) {
		let (staking_currency_id, owner_lookup) =
			<T as Config>::BenchmarkHelper::setup_liquidate_by_auction(b).unwrap();

		#[extrinsic_call]
		liquidate(RawOrigin::None, staking_currency_id, owner_lookup);
	}

	// `liquidate` by dex
	#[benchmark]
	fn liquidate_by_dex() {
		let (liquid_currency_id, owner_lookup) = <T as Config>::BenchmarkHelper::setup_liquidate_by_dex().unwrap();

		#[extrinsic_call]
		liquidate(RawOrigin::None, liquid_currency_id, owner_lookup);
	}

	#[benchmark]
	fn settle() {
		let (staking_currency_id, owner_lookup) = <T as Config>::BenchmarkHelper::setup_settle().unwrap();

		#[extrinsic_call]
		_(RawOrigin::None, staking_currency_id, owner_lookup);
	}

	#[benchmark]
	fn register_liquidation_contract() {
		#[extrinsic_call]
		_(RawOrigin::Root, EvmAddress::default());
	}

	#[benchmark]
	fn deregister_liquidation_contract() {
		assert_ok!(Pallet::<T>::register_liquidation_contract(
			RawOrigin::Root.into(),
			EvmAddress::default()
		));

		#[extrinsic_call]
		_(RawOrigin::Root, EvmAddress::default());
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
