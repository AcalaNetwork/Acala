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

use crate::{
	AccountId, CurrencyId, GetLiquidCurrencyId, MaxUnbondingChunks, MinCouncilBondThreshold, NominateesCount,
	NomineesElection, Runtime,
};

use super::utils::set_balance;
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use module_support::OnNewEra;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const SEED: u32 = 0;

const LIQUID: CurrencyId = GetLiquidCurrencyId::get();

runtime_benchmarks! {
	{ Runtime, module_nominees_election }

	bond {
		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
	}: _(RawOrigin::Signed(caller), MinCouncilBondThreshold::get())

	unbond {
		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinCouncilBondThreshold::get())?;
	}: _(RawOrigin::Signed(caller), MinCouncilBondThreshold::get())

	rebond {
		let c in 1 .. MaxUnbondingChunks::get();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), 2 * MinCouncilBondThreshold::get())?;
		for _ in 0..c {
			NomineesElection::unbond(RawOrigin::Signed(caller.clone()).into(), MinCouncilBondThreshold::get()/c as u128)?;
		}
	}: _(RawOrigin::Signed(caller), MinCouncilBondThreshold::get())

	withdraw_unbonded {
		let c in 1 .. MaxUnbondingChunks::get();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), 2 * MinCouncilBondThreshold::get())?;
		for _ in 0..c {
			NomineesElection::unbond(RawOrigin::Signed(caller.clone()).into(), MinCouncilBondThreshold::get()/c as u128)?;
		}
		NomineesElection::on_new_era(1);
	}: _(RawOrigin::Signed(caller))

	nominate {
		let c in 1 .. NominateesCount::get();
		let targets = (0..c).map(|c| account("nominatees", c, SEED)).collect::<Vec<_>>();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinCouncilBondThreshold::get())?;
	}: _(RawOrigin::Signed(caller), targets)

	chill {
		let c in 1 .. NominateesCount::get();
		let targets = (0..c).map(|c| account("nominatees", c, SEED)).collect::<Vec<_>>();

		let caller: AccountId = whitelisted_caller();
		set_balance(LIQUID, &caller, 2 * MinCouncilBondThreshold::get());
		NomineesElection::bond(RawOrigin::Signed(caller.clone()).into(), MinCouncilBondThreshold::get())?;
		NomineesElection::nominate(RawOrigin::Signed(caller.clone()).into(), targets)?;
	}: _(RawOrigin::Signed(caller))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
