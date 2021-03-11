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

use crate::{
	dollar, AcalaOracle, AccountId, CdpTreasury, CollateralCurrencyIds, EmergencyShutdown, Price, Runtime, AUSD,
};

use super::utils::set_balance;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_emergency_shutdown }

	_ {}

	emergency_shutdown {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for i in 0 .. c {
			values.push((currency_ids[i as usize], Price::one()));
		}
		AcalaOracle::feed_values(RawOrigin::Root.into(), values)?;
	}: _(RawOrigin::Root)

	open_collateral_refund {
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::Root)

	refund_collaterals {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let funder: AccountId = account("funder", 0, SEED);
		let caller: AccountId = account("caller", 0, SEED);
		let mut values = vec![];

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			values.push((currency_id, Price::one()));
			set_balance(currency_id, &funder, 100 * dollar(currency_id));
			CdpTreasury::deposit_collateral(&funder, currency_id, 100 * dollar(currency_id))?;
		}
		AcalaOracle::feed_values(RawOrigin::Root.into(), values)?;

		CdpTreasury::issue_debit(&caller, 1_000 * dollar(AUSD), true)?;
		CdpTreasury::issue_debit(&funder, 1_000 * dollar(AUSD), true)?;

		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
		EmergencyShutdown::open_collateral_refund(RawOrigin::Root.into())?;
	}: _(RawOrigin::Signed(caller),  1_000 * dollar(AUSD))
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_emergency_shutdown() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_emergency_shutdown());
		});
	}

	#[test]
	fn test_open_collateral_refund() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_open_collateral_refund());
		});
	}

	#[test]
	fn test_refund_collaterals() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_refund_collaterals());
		});
	}
}
