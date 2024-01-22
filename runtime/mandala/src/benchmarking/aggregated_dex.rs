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

use super::utils::{dollar, inject_liquidity, set_balance, LIQUID, NATIVE, STABLECOIN, STAKING};
use crate::{AccountId, CurrencyId, Runtime};
use module_aggregated_dex::SwapPath;
use runtime_common::{BNC, VSKSM};

use sp_std::prelude::*;

use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;

const CURRENCY_LIST: [CurrencyId; 6] = [NATIVE, STABLECOIN, LIQUID, STAKING, BNC, VSKSM];

runtime_benchmarks! {
	{ Runtime, module_aggregated_dex }

	swap_with_exact_supply {
		let u in 2 .. <Runtime as module_dex::Config>::TradingPathLimit::get();

		let maker: AccountId = account("maker", 0, 0);
		let taker: AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity(maker.clone(), cur0, cur1, 10_000 * dollar(cur0), 10_000 * dollar(cur1), false)?;
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity(maker.clone(), CURRENCY_LIST[i as usize - 1], CURRENCY_LIST[i as usize], 10_000 * dollar(CURRENCY_LIST[i as usize - 1]), 10_000 * dollar(CURRENCY_LIST[i as usize]), false)?;
			}
		}

		set_balance(path[0], &taker, 10_000 * dollar(path[0]));
	}: swap_with_exact_supply(RawOrigin::Signed(taker), vec![SwapPath::Dex(path.clone())], 100 * dollar(path[0]), 0)

	swap_with_exact_target {
		let u in 2 .. <Runtime as module_dex::Config>::TradingPathLimit::get();

		let maker: AccountId = account("maker", 0, 0);
		let taker: AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity(maker.clone(), cur0, cur1, 10_000 * dollar(cur0), 10_000 * dollar(cur1), false)?;
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity(maker.clone(), CURRENCY_LIST[i as usize - 1], CURRENCY_LIST[i as usize], 10_000 * dollar(CURRENCY_LIST[i as usize - 1]), 10_000 * dollar(CURRENCY_LIST[i as usize]), false)?;
			}
		}

		set_balance(path[0], &taker, 10_000 * dollar(path[0]));
	}: swap_with_exact_target(RawOrigin::Signed(taker), vec![SwapPath::Dex(path.clone())], 10 * dollar(path[path.len() - 1]), 1_000 * dollar(path[0]))

	update_aggregated_swap_paths {
		let n in 0 .. CURRENCY_LIST.len() as u32;
		let mut updates: Vec<((CurrencyId, CurrencyId), Option<Vec<SwapPath>>)> = vec![];
		for i in 1..n {
			let token_a = CURRENCY_LIST[i as usize];
			updates.push(
				((token_a, CURRENCY_LIST[0]), Some(vec![SwapPath::Dex(vec![token_a, CURRENCY_LIST[0]])]))
			);
		}
	}: _(RawOrigin::Root, updates)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
