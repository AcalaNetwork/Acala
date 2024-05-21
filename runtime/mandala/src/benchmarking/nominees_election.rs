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
	AccountId, Balance, BondingDuration, Homa, HomaValidatorList, MinNomineesElectionBondThreshold, NomineesElection,
	Runtime, ValidatorInsuranceThreshold,
};

use super::utils::{set_balance, LIQUID};
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::{traits::Get, BoundedVec};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::SaturatedConversion;
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_nominees_election }

	bond {
		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get());
	}: _(RawOrigin::Signed(caller), MinNomineesElectionBondThreshold::get())

	unbond {
		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinNomineesElectionBondThreshold::get())?;
	}: _(RawOrigin::Signed(caller), MinNomineesElectionBondThreshold::get())

	rebond {
		let c in 1 .. <Runtime as module_nominees_election::Config>::MaxUnbondingChunks::get();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), 2 * MinNomineesElectionBondThreshold::get())?;
		for _ in 0..c {
			NomineesElection::unbond(RawOrigin::Signed(caller.clone()).into(), MinNomineesElectionBondThreshold::get()/c as u128)?;
		}
	}: _(RawOrigin::Signed(caller), MinNomineesElectionBondThreshold::get())

	withdraw_unbonded {
		let c in 1 .. <Runtime as module_nominees_election::Config>::MaxUnbondingChunks::get();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), 2 * MinNomineesElectionBondThreshold::get())?;
		for _ in 0..c {
			NomineesElection::unbond(RawOrigin::Signed(caller.clone()).into(), MinNomineesElectionBondThreshold::get()/c as u128)?;
		}
		Homa::force_bump_current_era(RawOrigin::Root.into(), BondingDuration::get())?;
	}: _(RawOrigin::Signed(caller))

	nominate {
		let c in 1 .. <Runtime as module_nominees_election::Config>::MaxNominateesCount::get();
		let targets: Vec<AccountId> = (0..c).map(|c| account("nominatees", c, SEED)).collect();
		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get() + ValidatorInsuranceThreshold::get() * targets.len().saturated_into::<Balance>());

		for validator in targets.iter() {
			HomaValidatorList::bond(RawOrigin::Signed(caller.clone()).into(), validator.clone(), ValidatorInsuranceThreshold::get())?;
		}

		let caller: AccountId = whitelisted_caller();
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinNomineesElectionBondThreshold::get())?;
	}: _(RawOrigin::Signed(caller), targets)

	chill {
		let c in 1 .. <Runtime as module_nominees_election::Config>::MaxNominateesCount::get();
		let targets: Vec<AccountId> = (0..c).map(|c| account("nominatees", c, SEED)).collect();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinNomineesElectionBondThreshold::get() + ValidatorInsuranceThreshold::get() * targets.len().saturated_into::<Balance>());

		for validator in targets.iter() {
			HomaValidatorList::bond(RawOrigin::Signed(caller.clone()).into(), validator.clone(), ValidatorInsuranceThreshold::get())?;
		}

		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinNomineesElectionBondThreshold::get())?;
		NomineesElection::nominate(RawOrigin::Signed(caller.clone()).into(), targets)?;
	}: _(RawOrigin::Signed(caller))

	reset_reserved_nominees {
		let c in 1 .. 4;
		let updates: Vec<(u16, BoundedVec<AccountId, <Runtime as module_nominees_election::Config>::MaxNominateesCount>)> = (0..c).map(|c| {
			let reserved: BoundedVec<AccountId, <Runtime as module_nominees_election::Config>::MaxNominateesCount> =
				(0..<Runtime as module_nominees_election::Config>::MaxNominateesCount::get()).map(|c| account("nominatees", c, SEED)).collect::<Vec<AccountId>>().try_into().unwrap();
			(c.saturated_into(), reserved)
		}).collect();
	}: _(RawOrigin::Root, updates)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
