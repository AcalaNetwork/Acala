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

use super::utils::{dollar, set_balance};
use crate::{
	AccountId, CurrencyId, DispatchResult, Earning, Get, GetNativeCurrencyId, NativeTokenExistentialDeposit, Origin,
	Runtime, System,
};
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;

const NATIVE: CurrencyId = GetNativeCurrencyId::get();

fn make_max_unbonding_chunk(who: AccountId) -> DispatchResult {
	System::set_block_number(0);
	set_balance(NATIVE, &who, 100 * dollar(NATIVE));
	let max_unlock_chunk: u32 = <Runtime as module_earning::Config>::MaxUnbondingChunks::get();
	Earning::bond(Origin::signed(who.clone()), 10 * dollar(NATIVE))?;
	for _ in 0..(max_unlock_chunk) {
		System::set_block_number(System::block_number() + 1);
		Earning::unbond(Origin::signed(who.clone()), NativeTokenExistentialDeposit::get())?;
	}

	Ok(())
}

runtime_benchmarks! {
	{Runtime, module_earning}

	bond {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVE, &caller, dollar(NATIVE));
	}: _(RawOrigin::Signed(caller), NativeTokenExistentialDeposit::get())

	unbond_instant {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVE, &caller, dollar(NATIVE));
		Earning::bond(Origin::signed(caller.clone()), 2 * NativeTokenExistentialDeposit::get())?;
	}: _(RawOrigin::Signed(caller), NativeTokenExistentialDeposit::get())

	unbond {
		let caller: AccountId = whitelisted_caller();
		set_balance(NATIVE, &caller, dollar(NATIVE));
		Earning::bond(Origin::signed(caller.clone()), dollar(NATIVE))?;
	}: _(RawOrigin::Signed(caller), NativeTokenExistentialDeposit::get())

	rebond {
		let caller: AccountId = whitelisted_caller();
		make_max_unbonding_chunk(caller.clone())?;
	}: _(RawOrigin::Signed(caller), 10 * dollar(NATIVE))

	withdraw_unbonded {
		let caller: AccountId = whitelisted_caller();
		make_max_unbonding_chunk(caller.clone())?;
		// large number to unlock all chunks
		System::set_block_number(1_000_000);
	}: _(RawOrigin::Signed(caller))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
