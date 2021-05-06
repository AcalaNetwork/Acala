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

use super::utils::{lookup_of_account, set_balance};
use crate::{dollar, AccountId, Amount, Balance, Currencies, NativeTokenExistentialDeposit, Runtime, ACA, DOT};

use sp_std::prelude::*;

use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_currencies }

	// `transfer` non-native currency
	transfer_non_native_currency {
		let currency_id = DOT;
		let amount: Balance = 1_000 * dollar(currency_id);
		let from: AccountId = whitelisted_caller();
		set_balance(currency_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(currency_id, &to), amount);
	}

	// `transfer` native currency and in worst case
	#[extra]
	transfer_native_currency_worst_case {
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let amount: Balance = existential_deposit.saturating_mul(1000);
		let native_currency_id = ACA;
		let from: AccountId = whitelisted_caller();
		set_balance(native_currency_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: transfer(RawOrigin::Signed(from), to_lookup, native_currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &to), amount);
	}

	// `transfer_native_currency` in worst case
	// * will create the `to` account.
	// * will kill the `from` account.
	transfer_native_currency {
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let amount: Balance = existential_deposit.saturating_mul(1000);
		let native_currency_id = ACA;
		let from: AccountId = whitelisted_caller();
		set_balance(native_currency_id, &from, amount);

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &to), amount);
	}

	// `update_balance` for non-native currency
	update_balance_non_native_currency {
		let currency_id = DOT;
		let balance: Balance = 2 * dollar(currency_id);
		let amount: Amount = balance.unique_saturated_into();
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
	}: update_balance(RawOrigin::Root, who_lookup, currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(currency_id, &who), balance);
	}

	// `update_balance` for native currency
	// * will create the `who` account.
	update_balance_native_currency_creating {
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let balance: Balance = existential_deposit.saturating_mul(1000);
		let amount: Amount = balance.unique_saturated_into();
		let native_currency_id = ACA;
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
	}: update_balance(RawOrigin::Root, who_lookup, native_currency_id, amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::total_balance(native_currency_id, &who), balance);
	}

	// `update_balance` for native currency
	// * will kill the `who` account.
	update_balance_native_currency_killing {
		let existential_deposit = NativeTokenExistentialDeposit::get();
		let balance: Balance = existential_deposit.saturating_mul(1000);
		let amount: Amount = balance.unique_saturated_into();
		let native_currency_id = ACA;
		let who: AccountId = account("who", 0, SEED);
		let who_lookup = lookup_of_account(who.clone());
		set_balance(native_currency_id, &who, balance);
	}: update_balance(RawOrigin::Root, who_lookup, native_currency_id, -amount)
	verify {
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(native_currency_id, &who), 0);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
