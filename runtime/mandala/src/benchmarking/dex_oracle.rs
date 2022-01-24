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

use crate::{
	dollar, AccountId, Balance, Currencies, CurrencyId, Dex, DexOracle, GetNativeCurrencyId, GetStableCurrencyId,
	GetStakingCurrencyId, Runtime, Timestamp,
};

use frame_benchmarking::whitelisted_caller;
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrencyExtended;
use primitives::TradingPair;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;

const NATIVE: CurrencyId = GetNativeCurrencyId::get();
const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const STAKING: CurrencyId = GetStakingCurrencyId::get();

fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
	deposit: bool,
) -> Result<(), &'static str> {
	// set balance
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_a,
		&maker,
		max_amount_a.unique_saturated_into(),
	)?;
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_b,
		&maker,
		max_amount_b.unique_saturated_into(),
	)?;

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		deposit,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_dex_oracle }

	on_initialize_with_update_average_prices {
		let n in 0 .. 3;
		let u in 0 .. 3;
		let caller: AccountId = whitelisted_caller();
		let trading_pair_list = vec![
			TradingPair::from_currency_ids(NATIVE, STABLECOIN).unwrap(),
			TradingPair::from_currency_ids(NATIVE, STAKING).unwrap(),
			TradingPair::from_currency_ids(STAKING, STABLECOIN).unwrap(),
		];

		for i in 0 .. n {
			let trading_pair = trading_pair_list[i as usize];
			inject_liquidity(caller.clone(), trading_pair.first(), trading_pair.second(), dollar(trading_pair.first()) * 100, dollar(trading_pair.second()) * 1000, false)?;
			DexOracle::enable_average_price(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second(), 240000)?;
		}
		for j in 0 .. u.min(n) {
			let update_pair = trading_pair_list[j as usize];
			DexOracle::update_average_price_interval(RawOrigin::Root.into(), update_pair.first(), update_pair.second(), 24000)?;
		}
	}: {
		Timestamp::set_timestamp(24000);
		let _ = DexOracle::on_initialize(1);
	}

	enable_average_price {
		let caller: AccountId = whitelisted_caller();
		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE), dollar(STABLECOIN), false)?;
	}: _(RawOrigin::Root, NATIVE, STABLECOIN, 24000)


	disable_average_price {
		let caller: AccountId = whitelisted_caller();
		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE) * 100, dollar(STABLECOIN) * 1000, false)?;
		DexOracle::enable_average_price(RawOrigin::Root.into(), NATIVE, STABLECOIN, 24000)?;
	}: _(RawOrigin::Root, NATIVE, STABLECOIN)

	update_average_price_interval {
		let caller: AccountId = whitelisted_caller();
		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE) * 100, dollar(STABLECOIN) * 1000, false)?;
		DexOracle::enable_average_price(RawOrigin::Root.into(), NATIVE, STABLECOIN, 24000)?;
	}: _(RawOrigin::Root, NATIVE, STABLECOIN, 240000)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
