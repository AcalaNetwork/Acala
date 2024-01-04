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

use crate::{AccountId, CdpTreasury, CurrencyId, EmergencyShutdown, Price, Runtime};

use super::{
	get_benchmarking_collateral_currency_ids,
	utils::{dollar, feed_price, set_balance, STABLECOIN},
};
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::traits::One;
use sp_std::vec;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_emergency_shutdown }

	emergency_shutdown {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let mut values = vec![];

		for i in 0 .. c {
			values.push((currency_ids[i as usize], Price::one()));
		}
		feed_price(values.try_into().unwrap())?;
	}: _(RawOrigin::Root)

	open_collateral_refund {
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::Root)

	refund_collaterals {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let funder: AccountId = account("funder", 0, SEED);
		let caller: AccountId = whitelisted_caller();
		let mut values = vec![];

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			if matches!(currency_id, CurrencyId::StableAssetPoolToken(_)) {
				continue;
			}
			values.push((currency_id, Price::one()));
			set_balance(currency_id, &funder, 100 * dollar(currency_id));
			CdpTreasury::deposit_collateral(&funder, currency_id, 100 * dollar(currency_id))?;
		}
		feed_price(values)?;

		CdpTreasury::issue_debit(&caller, 1_000 * dollar(STABLECOIN), true)?;
		CdpTreasury::issue_debit(&funder, 1_000 * dollar(STABLECOIN), true)?;

		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
		EmergencyShutdown::open_collateral_refund(RawOrigin::Root.into())?;
	}: _(RawOrigin::Signed(caller),  1_000 * dollar(STABLECOIN))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
