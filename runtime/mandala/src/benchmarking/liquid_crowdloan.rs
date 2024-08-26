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

use crate::{
	AccountId, GetLiquidCurrencyId, GetStakingCurrencyId, LiquidCrowdloan, LiquidCrowdloanCurrencyId, PolkadotXcm,
	Runtime, RuntimeOrigin, System,
};

use super::utils::{set_balance, STAKING};
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_liquid_crowdloan }

	redeem {
		let caller: AccountId = whitelisted_caller();
		let amount = 100_000_000_000_000;
		set_balance(LiquidCrowdloanCurrencyId::get(), &caller, amount);
		set_balance(STAKING, &LiquidCrowdloan::account_id(), amount);
	}: _(RawOrigin::Signed(caller), amount)
	verify {
		System::assert_last_event(module_liquid_crowdloan::Event::Redeemed { currency_id: GetStakingCurrencyId::get(), amount }.into());
	}

	set_redeem_currency_id {
	}: _(RawOrigin::Root, GetLiquidCurrencyId::get())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
