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
pub trait BenchmarkHelper<AccountId, CurrencyId> {
	fn setup_dex_pools(caller: AccountId) -> Option<CurrencyId>;
}

impl<AccountId, CurrencyId> BenchmarkHelper<AccountId, CurrencyId> for () {
	fn setup_dex_pools(_caller: AccountId) -> Option<CurrencyId> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn auction_collateral(b: Liner<1, { T::MaxAuctionsCount::get() }>) {
		let amount = 1_000_000_000_000_000u128;
		let auction_size = (1_000 * amount) / b as u128;

		assert_ok!(Pallet::<T>::set_expected_collateral_auction_size(
			RawOrigin::Root.into(),
			T::GetStableCurrencyId::get(),
			auction_size
		));

		assert_ok!(T::Currency::deposit(
			T::GetStableCurrencyId::get(),
			&Pallet::<T>::account_id(),
			10_000 * amount
		));

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			T::GetStableCurrencyId::get(),
			1_000 * amount,
			1_000 * amount,
			true,
		);
	}

	#[benchmark]
	fn exchange_collateral_to_stable() {
		let amount = 1_000_000_000_000_000u128;
		let caller: T::AccountId = account("caller", 0, 0);

		let staking_currency_id = T::BenchmarkHelper::setup_dex_pools(caller.clone()).unwrap();

		assert_ok!(T::Currency::deposit(
			T::GetStableCurrencyId::get(),
			&Pallet::<T>::account_id(),
			10_000 * amount
		));
		assert_ok!(T::Currency::deposit(
			T::GetStableCurrencyId::get(),
			&caller,
			1000 * amount
		));
		assert_ok!(T::Currency::deposit(staking_currency_id, &caller, 1000 * amount));

		assert_ok!(Pallet::<T>::deposit_collateral(
			&caller,
			staking_currency_id,
			100 * amount
		));

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			staking_currency_id,
			SwapLimit::ExactSupply(100 * amount, 0),
		);
	}

	#[benchmark]
	fn set_expected_collateral_auction_size() {
		let amount = 1_000_000_000_000_000u128;
		#[extrinsic_call]
		_(RawOrigin::Root, T::GetStableCurrencyId::get(), amount);
	}

	#[benchmark]
	fn extract_surplus_to_treasury() {
		let amount = 1_000_000_000_000_000u128;

		assert_ok!(Pallet::<T>::on_system_surplus(5 * amount));

		#[extrinsic_call]
		_(RawOrigin::Root, amount);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
