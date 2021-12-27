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

use super::utils::set_balance;
use crate::{
	AccountId, CurrencyId, GetNativeCurrencyId, GetStableCurrencyId, NativeTokenExistentialDeposit, Runtime, System,
	TransactionPayment,
};
use frame_benchmarking::whitelisted_caller;
use frame_support::traits::OnFinalize;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const NATIVECOIN: CurrencyId = GetNativeCurrencyId::get();

runtime_benchmarks! {
	{ Runtime, module_transaction_payment }

	set_alternative_fee_swap_path {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVECOIN, &caller, NativeTokenExistentialDeposit::get());
	}: _(RawOrigin::Signed(caller.clone()), Some(vec![STABLECOIN, NATIVECOIN]))
	verify {
		assert_eq!(TransactionPayment::alternative_fee_swap_path(&caller).unwrap().into_inner(), vec![STABLECOIN, NATIVECOIN]);
	}

	on_finalize {
	}: {
		TransactionPayment::on_finalize(System::block_number());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
