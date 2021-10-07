// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

use super::utils::set_balance_fungibles;
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use primitives::currency::{CurrencyId, AUSD, LDOT};
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, nutsfinance_stable_asset }

	create_pool {
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
	}: _(RawOrigin::Root, pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient)

	modify_a {
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
	}: _(RawOrigin::Root, pool_id, 1000u128, 2629112370)

	mint {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
	}: _(RawOrigin::Signed(tester), pool_id, vec![10000000000u128, 20000000000u128], 0u128)

	swap {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
		let _ = StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, vec![10000000000u128, 20000000000u128], 0u128);
	}: _(RawOrigin::Signed(tester), pool_id, 0, 1, 5000000u128, 0u128)

	collect_fee {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
		let _ = StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, vec![10000000000u128, 20000000000u128], 0u128);
		let _ = StableAsset::swap(RawOrigin::Signed(tester.clone()).into(), pool_id, 0, 1, 5000000u128, 0u128);
	}: _(RawOrigin::Root, pool_id)

	redeem_proportion {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
		let _ = StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, vec![10000000000u128, 20000000000u128], 0u128);
	}: _(RawOrigin::Signed(tester), pool_id, 100000000u128, vec![0u128, 0u128])

	redeem_single {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
		let _ = StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, vec![10000000000u128, 20000000000u128], 0u128);
	}: _(RawOrigin::Signed(tester), pool_id, 100000000u128, 0u32, 0u128)

	redeem_multi {
		let tester: AccountId = whitelisted_caller();
		let pool_asset = CurrencyId::StableAssetPoolToken(1);
		let assets = vec![LDOT, AUSD];
		let precisions = vec![1u128, 1u128];
		let mint_fee = 10000000u128;
		let swap_fee = 20000000u128;
		let redeem_fee = 50000000u128;
		let intial_a = 100u128;
		let fee_recipient: AccountId = account("fee", 0, SEED);
		let _ = StableAsset::create_pool(RawOrigin::Root.into(), pool_asset, assets, precisions, mint_fee, swap_fee, redeem_fee, intial_a, fee_recipient.clone());
		let pool_id = StableAsset::pool_count() - 1;
		set_balance_fungibles(LDOT, &tester, 200000000000u128);
		set_balance_fungibles(AUSD, &tester, 200000000000u128);
		let _ = StableAsset::mint(RawOrigin::Signed(tester.clone()).into(), pool_id, vec![10000000000u128, 20000000000u128], 0u128);
	}: _(RawOrigin::Signed(tester), pool_id, vec![500000u128, 500000u128], 1100000000000000000u128)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
