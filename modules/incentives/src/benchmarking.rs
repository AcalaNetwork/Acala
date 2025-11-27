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
pub trait BenchmarkHelper<CurrencyId, Balance> {
	fn setup_stable_currency_id_and_amount() -> Option<(CurrencyId, Balance)>;
	fn setup_collateral_currency_ids() -> Vec<(CurrencyId, Balance)>;
}

impl<CurrencyId, Balance> BenchmarkHelper<CurrencyId, Balance> for () {
	fn setup_stable_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		None
	}
	fn setup_collateral_currency_ids() -> Vec<(CurrencyId, Balance)> {
		vec![]
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize(c: Linear<0, { T::BenchmarkHelper::setup_collateral_currency_ids().len() as u32 }>) {
		let currency_ids = T::BenchmarkHelper::setup_collateral_currency_ids();
		let block_number = T::AccumulatePeriod::get();

		for i in 0..c {
			let (currency_id, amount) = currency_ids[i as usize];
			let pool_id = PoolId::Loans(currency_id);

			assert_ok!(Pallet::<T>::update_incentive_rewards(
				RawOrigin::Root.into(),
				vec![(pool_id.clone(), vec![(T::NativeCurrencyId::get(), 100 * amount)])]
			));
			orml_rewards::PoolInfos::<T>::mutate(pool_id, |pool_info| {
				pool_info.total_shares += 100;
			});
		}

		Pallet::<T>::on_initialize(1u32.into());
		frame_system::Pallet::<T>::set_block_number(block_number);

		#[block]
		{
			Pallet::<T>::on_initialize(frame_system::Pallet::<T>::block_number());
		}
	}

	#[benchmark]
	fn deposit_dex_share() {
		let caller: T::AccountId = account("caller", 0, 0);
		let (stable_currency_id, amount) = T::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let native_stablecoin_lp =
			CurrencyId::join_dex_share_currency_id(T::NativeCurrencyId::get(), stable_currency_id).unwrap();

		let _ = T::Currency::deposit(native_stablecoin_lp, &caller, amount);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), native_stablecoin_lp, amount);
	}

	#[benchmark]
	fn withdraw_dex_share() {
		let caller: T::AccountId = account("caller", 0, 0);
		let (stable_currency_id, amount) = T::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let native_stablecoin_lp =
			CurrencyId::join_dex_share_currency_id(T::NativeCurrencyId::get(), stable_currency_id).unwrap();

		let _ = T::Currency::deposit(native_stablecoin_lp, &caller, amount);

		assert_ok!(Pallet::<T>::deposit_dex_share(
			RawOrigin::Signed(caller.clone()).into(),
			native_stablecoin_lp,
			amount
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), native_stablecoin_lp, amount);
	}

	#[benchmark]
	fn claim_rewards() {
		let caller: T::AccountId = account("caller", 0, 0);
		let (stable_currency_id, amount) = T::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let pool_id = PoolId::Loans(stable_currency_id);

		assert_ok!(orml_rewards::Pallet::<T>::add_share(&caller, &pool_id, amount));

		let _ = T::Currency::deposit(T::NativeCurrencyId::get(), &&Pallet::<T>::account_id(), 80 * amount);

		assert_ok!(orml_rewards::Pallet::<T>::accumulate_reward(
			&pool_id,
			T::NativeCurrencyId::get(),
			80 * amount
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), pool_id);
	}

	#[benchmark]
	fn update_incentive_rewards(c: Liner<0, { T::BenchmarkHelper::setup_collateral_currency_ids().len() as u32 }>) {
		let currency_ids = T::BenchmarkHelper::setup_collateral_currency_ids();
		let mut updates = vec![];

		for i in 0..c {
			let (currency_id, amount) = currency_ids[i as usize];
			updates.push((PoolId::Loans(currency_id), vec![(T::NativeCurrencyId::get(), amount)]));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates);
	}

	#[benchmark]
	fn update_claim_reward_deduction_rates(
		c: Linear<0, { T::BenchmarkHelper::setup_collateral_currency_ids().len() as u32 }>,
	) {
		let currency_ids = T::BenchmarkHelper::setup_collateral_currency_ids();
		let mut updates = vec![];

		for i in 0..c {
			let (currency_id, _amount) = currency_ids[i as usize];
			updates.push((PoolId::Loans(currency_id), Rate::default()));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates);
	}

	#[benchmark]
	fn update_claim_reward_deduction_currency() {
		#[extrinsic_call]
		_(
			RawOrigin::Root,
			PoolId::Earning(T::NativeCurrencyId::get()),
			Some(T::NativeCurrencyId::get()),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
