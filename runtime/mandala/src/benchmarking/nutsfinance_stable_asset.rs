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

use crate::{AccountId, Runtime, StableAsset};

use super::utils::{
	create_stable_pools, dollar, register_stable_asset, set_balance, LIQUID, NATIVE, STABLECOIN, STAKING,
};
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use primitives::currency::CurrencyId;
use sp_std::prelude::*;

const SEED: u32 = 0;

fn currency_list() -> Vec<CurrencyId> {
	vec![
		NATIVE,
		STABLECOIN,
		LIQUID,
		STAKING,
		CurrencyId::join_dex_share_currency_id(LIQUID, STAKING).unwrap(),
	]
}

runtime_benchmarks! {
	{ Runtime, nutsfinance_stable_asset }

	create_pool {
		let pool_asset = CurrencyId::StableAssetPoolToken(0);
		let assets = vec![LIQUID, STAKING];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 10000u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let yield_recipient: AccountId = account("yield", 1, SEED);
		register_stable_asset()?;
	}: _(RawOrigin::Root, pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient, yield_recipient, 1_000_000_000_000u128)

	modify_a {
		let assets = vec![LIQUID, STAKING];
		let precisions = vec![1u128, 1u128];
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Root, pool_id, 1000u128, 2629112370)

	modify_fees {
		let assets = vec![LIQUID, STAKING];
		let precisions = vec![1u128, 1u128];
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Root, pool_id, Some(100u128), Some(200u128), Some(300u128))

	modify_recipients {
		let assets = vec![LIQUID, STAKING];
		let precisions = vec![1u128, 1u128];
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Root, pool_id, Some(account("fee-1", 3, SEED)), Some(account("yield-1", 4, SEED)))

	mint {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let currency_id = currency_list()[i_idx];
			assets.push(currency_id);
			precisions.push(1u128);
			mint_args.push(dollar(currency_id));
			set_balance(currency_id, &tester, 10 * dollar(currency_id));
		}
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Signed(tester), pool_id, mint_args, 0u128)

	swap {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let currency_id = currency_list()[i_idx];
			assets.push(currency_id);
			precisions.push(1u128);
			set_balance(currency_id, &tester, u128::MAX / 2);
		}
		let mint_args = match u {
			2 => vec![u128::MAX / 10, 1],
			3 => vec![u128::MAX / 10, 1, 1],
			4 => vec![u128::MAX / 100000, 10000, 10000, 10000],
			5 => vec![u128::MAX / 100000000, 100000000, 100000000, 100000000, 100000000],
			_ => vec![]
		};
		create_stable_pools(assets, precisions, 10000)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args.clone(), 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, 1, 0, 100000u128, 0u128, u)

	redeem_proportion {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		let mut redeem_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let currency_id = currency_list()[i_idx];
			let multiple: u128 = (i + 1).into();
			assets.push(currency_id);
			precisions.push(1u128);
			mint_args.push(1000 * dollar(currency_id) * multiple);
			redeem_args.push(0u128);
			set_balance(currency_id, &tester, u128::MAX / 10);
		}
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, 1_000_000_000_000u128, redeem_args)

	redeem_single {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(0);
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let currency_id = currency_list()[i_idx];
			assets.push(currency_id);
			precisions.push(1u128);
			set_balance(currency_id, &tester, u128::MAX / 2);
		}
		let mint_args = match u {
			2 => vec![u128::MAX / 10, 1],
			3 => vec![u128::MAX / 10, 1, 1],
			4 => vec![u128::MAX / 100000, 10000, 10000, 10000],
			5 => vec![u128::MAX / 100000000, 100000000, 100000000, 100000000, 100000000],
			_ => vec![]
		};
		create_stable_pools(assets, precisions, 10000)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: {
		let _ = StableAsset::redeem_single(RawOrigin::Signed(tester).into(), pool_id, 10_000u128, 0u32, 0u128, u);
	}

	redeem_multi {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		let mut redeem_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let currency_id = currency_list()[i_idx];
			assets.push(currency_id);
			precisions.push(1u128);
			mint_args.push(100 * dollar(currency_id));
			redeem_args.push(dollar(currency_id));
			set_balance(currency_id, &tester, u128::MAX / 10);
		}
		create_stable_pools(assets, precisions, 10000u128)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, redeem_args, u128::MAX / 10)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
