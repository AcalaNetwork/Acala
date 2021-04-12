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

use crate::{dollar, CdpTreasury, Currencies, CurrencyId, Runtime, KAR, KSM, KUSD};

use frame_system::RawOrigin;
use module_support::CDPTreasury;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_cdp_treasury }

	_ {}

	auction_surplus {
		CdpTreasury::on_system_surplus(100 * dollar(KUSD))?;
	}: _(RawOrigin::Root, 100 * dollar(KUSD))

	auction_debit {
		CdpTreasury::on_system_debit(100 * dollar(KUSD))?;
	}: _(RawOrigin::Root, 100 * dollar(KUSD), 200 * dollar(KAR))

	auction_collateral {
		let currency_id: CurrencyId = KSM;
		Currencies::deposit(currency_id, &CdpTreasury::account_id(), 10_000 * dollar(currency_id))?;
	}: _(RawOrigin::Root, currency_id, 1_000 * dollar(currency_id), 1_000 * dollar(KUSD), true)

	set_collateral_auction_maximum_size {
		let currency_id: CurrencyId = KSM;
	}: _(RawOrigin::Root, currency_id, 200 * dollar(currency_id))
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
	fn test_auction_surplus() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_surplus());
		});
	}

	#[test]
	fn test_auction_debit() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_debit());
		});
	}

	#[test]
	fn test_auction_collateral() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_auction_collateral());
		});
	}

	#[test]
	fn test_set_collateral_auction_maximum_size() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_collateral_auction_maximum_size());
		});
	}
}
