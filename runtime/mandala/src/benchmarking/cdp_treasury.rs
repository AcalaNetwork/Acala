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

use crate::{dollar, CdpTreasury, Currencies, CurrencyId, GetStableCurrencyId, GetStakingCurrencyId, Runtime};

use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const STAKING: CurrencyId = GetStakingCurrencyId::get();

runtime_benchmarks! {
	{ Runtime, module_cdp_treasury }

	auction_collateral {
		Currencies::deposit(STAKING, &CdpTreasury::account_id(), 10_000 * dollar(STAKING))?;
	}: _(RawOrigin::Root, STAKING, 1_000 * dollar(STAKING), 1_000 * dollar(STABLECOIN), true)

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
