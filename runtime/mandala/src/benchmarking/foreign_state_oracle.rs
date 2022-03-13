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

use super::utils::set_balance;
pub use crate::{
	dollar, AccountId, AccountTokenizer, BlockNumber, CurrencyId, GetNativeCurrencyId, Runtime, System, Weight,
};
use codec::{Decode, Encode};
use frame_benchmarking::whitelisted_caller;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use runtime_common::MAXIMUM_BLOCK_WEIGHT;
use sp_io::hashing::blake2_256;
use sp_runtime::traits::TrailingZeroInput;

const NATIVE: CurrencyId = GetNativeCurrencyId::get();

fn dummy_anonymous_account(who: &AccountId, height: BlockNumber, ext_index: u32, index: u16) -> AccountId {
	let entropy = (b"modlpy/proxy____", who, height, ext_index, &[0_u8], index).using_encoded(blake2_256);
	let derived_account: AccountId = Decode::decode(&mut TrailingZeroInput::new(entropy.as_ref()))
		.expect("infinite length input; no invalid inputs for type; qed");
	derived_account
}

runtime_benchmarks! {
	{Runtime, module_foreign_state_oracle}

	purge_expired_query{
		let caller: AccountId = whitelisted_caller();
		let anon_account = dummy_anonymous_account(&caller, 0, 0, 0);
		set_balance(NATIVE, &caller, 10_000 * dollar(NATIVE));
		AccountTokenizer::request_mint(RawOrigin::Signed(caller.clone()).into(), anon_account, caller.clone(), 0, 0, 0)?;
		System::set_block_number(100);
	}: _(RawOrigin::Signed(caller), 0)

	respond_query_request{
		let caller: AccountId = whitelisted_caller();
		let anon_account = dummy_anonymous_account(&caller, 0, 0, 0);
		set_balance(NATIVE, &caller, 10_000 * dollar(NATIVE));
		AccountTokenizer::request_mint(RawOrigin::Signed(caller.clone()).into(), anon_account, caller.clone(), 0, 0, 0)?;
	}: _(RawOrigin::Root, 0, vec![1_u8], MAXIMUM_BLOCK_WEIGHT)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
