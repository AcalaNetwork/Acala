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

use super::utils::{create_stable_pools, dollar, inject_liquidity, LIQUID, NATIVE, SEED, STABLECOIN, STAKING};
use crate::{AccountId, AggregatedDex, Balance, Currencies, Dex, Runtime, System, TreasuryPalletId};
use frame_benchmarking::account;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use module_aggregated_dex::SwapPath;
use module_support::DEXManager;
use orml_traits::MultiCurrency;
use sp_runtime::traits::AccountIdConversion;

runtime_benchmarks! {
	{ Runtime, module_aggregated_dex }

	update_aggregated_swap_paths {
		let funder: AccountId = account("funder", 0, SEED);

		create_stable_pools(vec![STAKING, LIQUID], vec![1, 1])?;
		inject_liquidity(funder.clone(), LIQUID, NATIVE, 1000 * dollar(LIQUID), 1000 * dollar(NATIVE), false)?;

		let swap_path = vec![
			SwapPath::Taiga(0, 0, 1),
			SwapPath::Dex(vec![LIQUID, NATIVE])
		];

		let updates = vec![((STAKING, NATIVE), Some(swap_path.clone()))];
	}: _(RawOrigin::Root, updates)
	verify {
		assert_eq!(module_aggregated_dex::AggregatedSwapPaths::<Runtime>::get((STAKING, NATIVE)).unwrap().into_inner(), swap_path);
	}

	update_rebalance_swap_paths {
		let funder: AccountId = account("funder", 0, SEED);

		create_stable_pools(vec![STAKING, LIQUID], vec![1, 1])?;
		inject_liquidity(funder.clone(), LIQUID, STAKING, 1000 * dollar(LIQUID), 1000 * dollar(STAKING), false)?;

		let swap_path = vec![
			SwapPath::Taiga(0, 0, 1),
			SwapPath::Dex(vec![LIQUID, STAKING])
		];

		let updates = vec![(STAKING, Some(swap_path.clone()))];
	}: _(RawOrigin::Root, updates)
	verify {
		assert_eq!(module_aggregated_dex::RebalanceSwapPaths::<Runtime>::get(STAKING).unwrap().into_inner(), swap_path);
	}

	set_rebalance_swap_info {
		let supply_amount: Balance = 100_000_000_000_000;
		let threshold: Balance = 110_000_000_000_000;
	}: _(RawOrigin::Root, STABLECOIN, supply_amount, threshold)
	verify {
		System::assert_has_event(module_aggregated_dex::Event::SetupRebalanceSwapInfo {
			currency_id: STABLECOIN,
			supply_amount,
			threshold,
		}.into());
	}

	force_rebalance_swap {
		let funder: AccountId = account("funder", 0, SEED);
		let supply: Balance = 10 * dollar(STABLECOIN);
		let threshold: Balance = 11 * dollar(STABLECOIN);

		inject_liquidity(funder.clone(), STABLECOIN, STAKING, 1000 * dollar(STABLECOIN), 1200 * dollar(STAKING), false)?;
		inject_liquidity(funder.clone(), STAKING, NATIVE, 1000 * dollar(STAKING), 1000 * dollar(NATIVE), false)?;
		inject_liquidity(funder.clone(), NATIVE, STABLECOIN, 1000 * dollar(NATIVE), 1000 * dollar(STABLECOIN), false)?;

		assert_ok!(AggregatedDex::set_rebalance_swap_info(RawOrigin::Root.into(), STABLECOIN, supply, threshold));

		let treasury_account: AccountId = TreasuryPalletId::get().into_account_truncating();
		Currencies::deposit(
			STABLECOIN,
			&treasury_account,
			100 * dollar(STABLECOIN),
		)?;

		let swap_path = vec![
			SwapPath::Dex(vec![STABLECOIN, STAKING]),
			SwapPath::Dex(vec![STAKING, NATIVE]),
			SwapPath::Dex(vec![NATIVE, STABLECOIN])
		];
	}: _(RawOrigin::None, STABLECOIN, swap_path)
	verify {
		assert_eq!(Dex::get_liquidity_pool(STABLECOIN, STAKING).0, 1000 * dollar(STABLECOIN) + supply);
		#[cfg(any(feature = "with-karura-runtime"))]
		assert_eq!(
			1200 * dollar(STAKING) - Dex::get_liquidity_pool(STABLECOIN, STAKING).1,
			Dex::get_liquidity_pool(STAKING, NATIVE).0 - 1000 * dollar(STAKING)
		);
		#[cfg(any(feature = "with-karura-runtime"))]
		assert_eq!(
			1000 * dollar(STAKING) - Dex::get_liquidity_pool(STAKING, NATIVE).1,
			Dex::get_liquidity_pool(NATIVE, STABLECOIN).0 - 1000 * dollar(NATIVE)
		);
		#[cfg(any(feature = "with-karura-runtime"))]
		assert_eq!(
			1000 * dollar(STAKING) - Dex::get_liquidity_pool(NATIVE, STABLECOIN).1,
			Currencies::free_balance(STABLECOIN, &treasury_account) - 90 * dollar(STABLECOIN)
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
