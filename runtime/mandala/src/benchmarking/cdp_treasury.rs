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
	AccountId, CdpTreasury, Currencies, CurrencyId, Dex, GetStableCurrencyId, GetStakingCurrencyId, MaxAuctionsCount,
	Runtime,
};

use super::utils::{dollar, set_balance};
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use module_support::{CDPTreasury, SwapLimit};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const STAKING: CurrencyId = GetStakingCurrencyId::get();

runtime_benchmarks! {
	{ Runtime, module_cdp_treasury }

	auction_collateral {
		let b in 1 .. MaxAuctionsCount::get();

		let auction_size = (1_000 * dollar(STAKING)) / b as u128;
		CdpTreasury::set_expected_collateral_auction_size(RawOrigin::Root.into(), STAKING, auction_size)?;

		Currencies::deposit(STAKING, &CdpTreasury::account_id(), 10_000 * dollar(STAKING))?;
	}: _(RawOrigin::Root, STAKING, 1_000 * dollar(STAKING), 1_000 * dollar(STABLECOIN), true)

	exchange_collateral_to_stable {
		let caller: AccountId = whitelisted_caller();
		set_balance(STABLECOIN, &caller, 1000 * dollar(STABLECOIN));
		set_balance(STAKING, &caller, 1000 * dollar(STAKING));
		let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), STABLECOIN, STAKING);
		Dex::add_liquidity(
			RawOrigin::Signed(caller.clone()).into(),
			STABLECOIN,
			STAKING,
			1000 * dollar(STABLECOIN),
			100 * dollar(STAKING),
			0,
			false,
		)?;
		CdpTreasury::deposit_collateral(&caller, STAKING, 100 * dollar(STAKING))?;
	}: _(RawOrigin::Root, STAKING, SwapLimit::ExactSupply(100 * dollar(STAKING), 0))

	set_expected_collateral_auction_size {
	}: _(RawOrigin::Root, STAKING, 200 * dollar(STAKING))

	extract_surplus_to_treasury {
		CdpTreasury::on_system_surplus(1_000 * dollar(STABLECOIN))?;
	}: _(RawOrigin::Root, 200 * dollar(STABLECOIN))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
