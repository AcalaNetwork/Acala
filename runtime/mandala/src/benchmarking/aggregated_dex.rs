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

use super::utils::{dollar, set_balance};
use crate::{
	AccountId, Balance, CurrencyId, Dex, GetLiquidCurrencyId, GetNativeCurrencyId, GetStableCurrencyId,
	GetStakingCurrencyId, Runtime,
};
use frame_benchmarking::account;
use frame_system::RawOrigin;
use primitives::TokenSymbol;
use sp_runtime::traits::UniqueSaturatedInto;

const SEED: u32 = 0;

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const STAKINGCOIN: CurrencyId = GetStakingCurrencyId::get();
const NATIVECOIN: CurrencyId = GetNativeCurrencyId::get();
const LIQUIDCOIN: CurrencyId = GetLiquidCurrencyId::get();

fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	// set balance
	set_balance(currency_id_a, &maker, max_amount_a.unique_saturated_into());
	set_balance(currency_id_b, &maker, max_amount_b.unique_saturated_into());

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_aggregated_dex }

	set_rebalance_swap_info {
		let supply: Balance = 100_000_000_000_000;
		let threshold: Balance = 110_000_000_000_000;

	}: _(RawOrigin::Root, STABLECOIN, supply, threshold)

	rebalance_swap {
		let funder: AccountId = account("funder", 0, SEED);

		let _ = inject_liquidity(funder.clone(), STABLECOIN, STAKINGCOIN, 100 * dollar(STABLECOIN), 200 * dollar(STAKINGCOIN));
		let _ = inject_liquidity(funder.clone(), STAKINGCOIN, NATIVECOIN, 100 * dollar(STAKINGCOIN), 200 * dollar(NATIVECOIN));
	}: _(RawOrigin::None, STABLECOIN, STAKINGCOIN, STAKINGCOIN)

	set_trading_pair_nodes {
		let funder: AccountId = account("funder", 0, SEED);

		let _ = inject_liquidity(funder.clone(), STABLECOIN, STAKINGCOIN, 100 * dollar(STABLECOIN), 200 * dollar(STAKINGCOIN));
		let _ = inject_liquidity(funder.clone(), STAKINGCOIN, NATIVECOIN, 100 * dollar(STAKINGCOIN), 200 * dollar(NATIVECOIN));
		let _ = inject_liquidity(funder.clone(), STABLECOIN, NATIVECOIN, 100 * dollar(STABLECOIN), 200 * dollar(NATIVECOIN));
		let _ = inject_liquidity(funder.clone(), STABLECOIN, LIQUIDCOIN, 100 * dollar(STABLECOIN), 200 * dollar(LIQUIDCOIN));
	}: _(RawOrigin::Root)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
