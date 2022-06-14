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

use crate::{Event, Runtime, System};
use frame_system::RawOrigin;
use module_fees::PoolPercent;
use orml_benchmarking::runtime_benchmarks;
use primitives::{AccountId, Balance, IncomeSource};
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::prelude::*;

fn assert_last_event(generic_event: Event) {
	System::assert_last_event(generic_event.into());
}

runtime_benchmarks! {
	{ Runtime, module_fees }

	set_income_fee {
		let pool = PoolPercent {
			pool: runtime_common::NetworkTreasuryPool::get(),
			rate: FixedU128::saturating_from_rational(1, 1),
		};
		let pools = vec![pool];
	}: _(RawOrigin::Root, IncomeSource::TxFee, pools.clone())
	verify {
		assert_last_event(module_fees::Event::IncomeFeeSet {
			income: IncomeSource::TxFee,
			pools,
		}.into());
	}

	set_treasury_pool {
		let pool = PoolPercent {
			pool: runtime_common::NetworkTreasuryPool::get(),
			rate: FixedU128::saturating_from_rational(1, 1),
		};
		let threshold: Balance = 100;
		let treasury: AccountId = runtime_common::NetworkTreasuryPool::get();
		let pools = vec![pool];
	}: _(RawOrigin::Root, treasury.clone(), threshold, pools.clone())
	verify {
		assert_last_event(module_fees::Event::TreasuryPoolSet {
			treasury,
			pools,
		}.into());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
