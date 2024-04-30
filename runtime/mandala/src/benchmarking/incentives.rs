// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use crate::{AccountId, AccumulatePeriod, Currencies, CurrencyId, Incentives, Rate, Rewards, Runtime, System};

use super::{
	get_benchmarking_collateral_currency_ids,
	utils::{dollar, set_balance, NATIVE, STABLECOIN, STAKING},
};
use frame_benchmarking::whitelisted_caller;
use frame_support::{assert_ok, traits::OnInitialize};
use frame_system::RawOrigin;
use module_support::PoolId;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_incentives }

	on_initialize {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let block_number = AccumulatePeriod::get();

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let pool_id = PoolId::Loans(currency_id);

			Incentives::update_incentive_rewards(RawOrigin::Root.into(), vec![(pool_id.clone(), vec![(NATIVE, 100 * dollar(NATIVE))])])?;
			orml_rewards::PoolInfos::<Runtime>::mutate(pool_id, |pool_info| {
				pool_info.total_shares += 100;
			});
		}

		Incentives::on_initialize(1);
		System::set_block_number(block_number);
	}: {
		Incentives::on_initialize(System::block_number());
	}

	deposit_dex_share {
		let caller: AccountId = whitelisted_caller();
		let native_stablecoin_lp = CurrencyId::join_dex_share_currency_id(NATIVE, STABLECOIN).unwrap();
		set_balance(native_stablecoin_lp, &caller, 10_000 * dollar(STABLECOIN));
	}: _(RawOrigin::Signed(caller), native_stablecoin_lp, 10_000 * dollar(STABLECOIN))

	withdraw_dex_share {
		let caller: AccountId = whitelisted_caller();
		let native_stablecoin_lp = CurrencyId::join_dex_share_currency_id(NATIVE, STABLECOIN).unwrap();
		set_balance(native_stablecoin_lp, &caller, 10_000 * dollar(STABLECOIN));
		Incentives::deposit_dex_share(
			RawOrigin::Signed(caller.clone()).into(),
			native_stablecoin_lp,
			10_000 * dollar(STABLECOIN)
		)?;
	}: _(RawOrigin::Signed(caller), native_stablecoin_lp, 8000 * dollar(STABLECOIN))

	claim_rewards {
		let caller: AccountId = whitelisted_caller();
		let pool_id = PoolId::Loans(STAKING);

		assert_ok!(Rewards::add_share(&caller, &pool_id, dollar(NATIVE)));
		Currencies::deposit(NATIVE, &Incentives::account_id(), 80 * dollar(NATIVE))?;
		Rewards::accumulate_reward(&pool_id, NATIVE, 80 * dollar(NATIVE))?;
	}: _(RawOrigin::Signed(caller), pool_id)

	update_incentive_rewards {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let mut updates = vec![];

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			updates.push((PoolId::Loans(currency_id), vec![(NATIVE, dollar(NATIVE))]));
		}
	}: _(RawOrigin::Root, updates)

	update_claim_reward_deduction_rates {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let mut updates = vec![];

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			updates.push((PoolId::Loans(currency_id), Rate::default()));
		}
	}: _(RawOrigin::Root, updates)

	update_claim_reward_deduction_currency {
	}: _(RawOrigin::Root, PoolId::Earning(NATIVE), Some(NATIVE))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
