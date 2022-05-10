// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

use crate::{AccountId, AssetRegistry, DispatchResult, Runtime, StableAsset};

use super::utils::set_balance_fungibles;
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use primitives::{
	currency::{AssetMetadata, CurrencyId, AUSD, BNC, LDOT, VSKSM},
	DexShare, TokenSymbol,
};
use sp_std::prelude::*;

const SEED: u32 = 0;
const CURRENCY_LIST: [CurrencyId; 5] = [
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::BNC), DexShare::Token(TokenSymbol::VSKSM)),
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::VSKSM), DexShare::Token(TokenSymbol::LDOT)),
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::VSKSM), DexShare::Token(TokenSymbol::AUSD)),
	BNC,
	VSKSM,
];

fn register_stable_asset() -> DispatchResult {
	let asset_metadata = AssetMetadata {
		name: b"Token Name".to_vec(),
		symbol: b"TN".to_vec(),
		decimals: 12,
		minimal_balance: 1,
	};
	AssetRegistry::register_stable_asset(RawOrigin::Root.into(), Box::new(asset_metadata.clone()))
}

fn create_pools(assets: Vec<CurrencyId>, precisions: Vec<u128>) -> DispatchResult {
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	let mint_fee = 10000000u128;
	let swap_fee = 20000000u128;
	let redeem_fee = 50000000u128;
	let intial_a = 10000u128;
	let fee_recipient: AccountId = account("fee", 0, SEED);
	let yield_recipient: AccountId = account("yield", 1, SEED);

	register_stable_asset()?;
	StableAsset::create_pool(
		RawOrigin::Root.into(),
		pool_asset,
		assets,
		precisions,
		mint_fee,
		swap_fee,
		redeem_fee,
		intial_a,
		fee_recipient,
		yield_recipient,
		1000000000000000000u128,
	)
}

runtime_benchmarks! {
	{ Runtime, nutsfinance_stable_asset }

	create_pool {
		let pool_asset = CurrencyId::StableAssetPoolToken(0);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 10000u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let yield_recipient: AccountId = account("yield", 1, SEED);
		register_stable_asset()?;
	}: _(RawOrigin::Root, pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient, yield_recipient, 1000000000000000000u128)

	modify_a {
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Root, pool_id, 1000u128, 2629112370)

	mint {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let multiple: u128 = (i + 1).into();
			assets.push(CURRENCY_LIST[i_idx]);
			precisions.push(1u128);
			mint_args.push(10000000000u128 * multiple);
		}
		for asset in &CURRENCY_LIST {
			set_balance_fungibles(*asset, &tester, 200000000000u128);
		}
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Signed(tester), pool_id, mint_args, 0u128)

	swap {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let multiple: u128 = (i + 1).into();
			assets.push(CURRENCY_LIST[i_idx]);
			precisions.push(1u128);
			mint_args.push(10000000000u128 * multiple);
		}
		for asset in &CURRENCY_LIST {
			set_balance_fungibles(*asset, &tester, 200000000000u128);
		}
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, 0, 1, 5000000u128, 0u128, u)

	redeem_proportion {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		let mut redeem_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let multiple: u128 = (i + 1).into();
			assets.push(CURRENCY_LIST[i_idx]);
			precisions.push(1u128);
			mint_args.push(10000000000u128 * multiple);
			redeem_args.push(0u128);
		}
		for asset in &CURRENCY_LIST {
			set_balance_fungibles(*asset, &tester, 200000000000u128);
		}
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, 100000000u128, redeem_args)

	redeem_single {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(0);
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let multiple: u128 = (i + 1).into();
			assets.push(CURRENCY_LIST[i_idx]);
			precisions.push(1u128);
			mint_args.push(10000000000u128 * multiple);
		}
		for asset in &CURRENCY_LIST {
			set_balance_fungibles(*asset, &tester, 200000000000u128);
		}
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, 100000000u128, 0u32, 0u128, u)

	redeem_multi {
		let tester: AccountId = whitelisted_caller();
		let u in 2u32 .. <Runtime as nutsfinance_stable_asset::Config>::PoolAssetLimit::get();
		let mut assets = vec![];
		let mut precisions = vec![];
		let mut mint_args = vec![];
		let mut redeem_args = vec![];
		for i in 0 .. u {
			let i_idx: usize = usize::try_from(i).unwrap();
			let multiple: u128 = (i + 1).into();
			assets.push(CURRENCY_LIST[i_idx]);
			precisions.push(1u128);
			mint_args.push(10000000000u128 * multiple);
			redeem_args.push(500000u128);
		}
		for asset in &CURRENCY_LIST {
			set_balance_fungibles(*asset, &tester, 200000000000u128);
		}
		create_pools(assets, precisions)?;
		let pool_id = StableAsset::pool_count() - 1;
		StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, mint_args, 0u128)?;
	}: _(RawOrigin::Signed(tester), pool_id, redeem_args, 1100000000000000000u128)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
