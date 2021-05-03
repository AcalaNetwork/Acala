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

use super::utils::{lookup_of_account, set_ausd_balance};
use crate::{dollar, AccountId, Balance, Runtime, Tokens, KUSD};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const SEED: u32 = 0;
const MAX_DOLLARS: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_tokens }

	transfer {
		let d in 1 .. MAX_DOLLARS;

		let amount: Balance = (d as u128) * dollar(KUSD);

		let from = account("from", 0, SEED);
		set_ausd_balance(&from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, KUSD, amount)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(KUSD, &to), amount);
	}

	transfer_all {
		let d in 1 .. MAX_DOLLARS;

		let amount: Balance = (d as u128) * dollar(KUSD);

		let from = account("from", 0, SEED);
		set_ausd_balance(&from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to);
	}: _(RawOrigin::Signed(from.clone()), to_lookup, KUSD)
	verify {
		assert_eq!(<Tokens as MultiCurrency<_>>::total_balance(KUSD, &from), 0);
	}
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
	fn transfer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer());
		});
	}

	#[test]
	fn transfer_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer_all());
		});
	}
}
