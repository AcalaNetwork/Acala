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

use crate::{AcalaDataProvider, AcalaOracle, CollateralCurrencyIds, Origin, Price, Runtime, System};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks_instance;
use sp_runtime::traits::One;
use sp_std::vec;

runtime_benchmarks_instance! {
	{ Runtime, orml_oracle, AcalaDataProvider }

	// feed values
	feed_values {
		let c in 0 .. CollateralCurrencyIds::get().len() as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for i in 0 .. c {
			values.push((currency_ids[i as usize], Price::one()));
		}
	}: _(Origin::root(), values)

	on_finalize {
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for currency_id in currency_ids {
			values.push((currency_id, Price::one()));
		}
		System::set_block_number(1);
		AcalaOracle::feed_values(Origin::root(), values)?;
	}: {
		AcalaOracle::on_finalize(System::block_number());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
