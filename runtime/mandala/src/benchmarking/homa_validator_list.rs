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

use crate::{
	AccountId, Balance, BondingDuration, Homa, HomaValidatorList, MinBondAmount, Runtime, ValidatorInsuranceThreshold,
};

use super::utils::{set_balance, LIQUID};
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use module_homa_validator_list::SlashInfo;
use orml_benchmarking::runtime_benchmarks;
use sp_std::{prelude::*, vec};

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_homa_validator_list }

	bond {
		let caller: AccountId = whitelisted_caller();
		let validator: AccountId = account("validator", 0, SEED);
		set_balance(LIQUID, &caller, MinBondAmount::get() * 10);
	}: _(RawOrigin::Signed(caller), validator, MinBondAmount::get())

	unbond {
		let caller: AccountId = whitelisted_caller();
		let validator: AccountId = account("validator", 0, SEED);
		let amount: Balance = MinBondAmount::get() * 10;

		set_balance(LIQUID, &caller, amount);
		HomaValidatorList::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			amount
		)?;
	}: _(RawOrigin::Signed(caller), validator, amount)

	rebond {
		let caller: AccountId = whitelisted_caller();
		let validator: AccountId = account("validator", 0, SEED);

		set_balance(LIQUID, &caller, MinBondAmount::get() * 10);
		HomaValidatorList::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			MinBondAmount::get() * 10
		)?;
		HomaValidatorList::unbond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			MinBondAmount::get() * 5
		)?;
	}: _(RawOrigin::Signed(caller), validator, MinBondAmount::get() * 5)

	withdraw_unbonded {
		let caller: AccountId = whitelisted_caller();
		let validator: AccountId = account("validator", 0, SEED);

		set_balance(LIQUID, &caller, MinBondAmount::get() * 10);
		HomaValidatorList::bond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			MinBondAmount::get() * 10
		)?;
		HomaValidatorList::unbond(
			RawOrigin::Signed(caller.clone()).into(),
			validator.clone(),
			MinBondAmount::get() * 5
		)?;
		Homa::force_bump_current_era(RawOrigin::Root.into(), BondingDuration::get())?;
	}: _(RawOrigin::Signed(caller), validator)

	freeze {
		let n in 1 .. 10;
		let caller: AccountId = whitelisted_caller();
		let mut validators: Vec<AccountId> = vec![];

		set_balance(LIQUID, &caller, MinBondAmount::get() * 100);
		for i in 0 .. n {
			let validator: AccountId = account("validator", i, SEED);
			HomaValidatorList::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				MinBondAmount::get()
			)?;
			validators.push(validator);
		}
	}: _(RawOrigin::Root, validators)

	thaw {
		let n in 1 .. 10;
		let caller: AccountId = whitelisted_caller();
		let mut validators: Vec<AccountId> = vec![];

		set_balance(LIQUID, &caller, MinBondAmount::get() * 100);
		for i in 0 .. n {
			let validator: AccountId = account("validator", i, SEED);
			HomaValidatorList::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				MinBondAmount::get()
			)?;
			validators.push(validator);
		}
		HomaValidatorList::freeze(RawOrigin::Root.into(), validators.clone())?;
	}: _(RawOrigin::Root, validators)

	slash {
		let n in 1 .. 10;
		let caller: AccountId = whitelisted_caller();
		let mut slashes: Vec<SlashInfo<Balance, AccountId>> = vec![];

		set_balance(LIQUID, &caller, ValidatorInsuranceThreshold::get() * 100);
		for i in 0 .. n {
			let validator: AccountId = account("validator", i, SEED);
			HomaValidatorList::bond(
				RawOrigin::Signed(caller.clone()).into(),
				validator.clone(),
				ValidatorInsuranceThreshold::get() * 10
			)?;
			slashes.push(SlashInfo{
				validator,
				relaychain_token_amount: ValidatorInsuranceThreshold::get() * 9
			});
		}
	}: _(RawOrigin::Root, slashes)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
