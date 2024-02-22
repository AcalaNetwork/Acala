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

use super::{
	get_vesting_account,
	utils::{lookup_of_account, set_balance, NATIVE},
};
use crate::{dollar, AccountId, Balance, BlockNumber, Currencies, Runtime, System, Vesting};

use sp_std::prelude::*;

use frame_benchmarking::{account, whitelisted_caller};
use frame_support::traits::Get;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_vesting::VestingSchedule;

pub type Schedule = VestingSchedule<BlockNumber, Balance>;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, orml_vesting }

	vested_transfer {
		let schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: <Runtime as orml_vesting::Config>::MinVestedTransfer::get(),
		};

		// extra 1 dollar to pay fees
		let from: AccountId = get_vesting_account();
		set_balance(NATIVE, &from, schedule.total_amount().unwrap() + dollar(NATIVE));

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, schedule.clone())
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::total_balance(NATIVE, &to),
			schedule.total_amount().unwrap()
		);
	}

	claim {
		let i in 1 .. <Runtime as orml_vesting::Config>::MaxVestingSchedules::get();

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: <Runtime as orml_vesting::Config>::MinVestedTransfer::get(),
		};

		let from: AccountId = get_vesting_account();
		// extra 1 dollar to pay fees
		set_balance(NATIVE, &from, schedule.total_amount().unwrap() * i as u128 + dollar(NATIVE));

		let to: AccountId = whitelisted_caller();
		let to_lookup = lookup_of_account(to.clone());

		for _ in 0..i {
			schedule.start = i;
			Vesting::vested_transfer(RawOrigin::Signed(from.clone()).into(), to_lookup.clone(), schedule.clone())?;
		}
		System::set_block_number(schedule.end().unwrap() + 1u32);
	}: _(RawOrigin::Signed(to.clone()))
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(NATIVE, &to),
			schedule.total_amount().unwrap() * i as u128,
		);
	}

	update_vesting_schedules {
		let i in 1 .. <Runtime as orml_vesting::Config>::MaxVestingSchedules::get();

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: <Runtime as orml_vesting::Config>::MinVestedTransfer::get(),
		};

		let to: AccountId = account("to", 0, SEED);
		set_balance(NATIVE, &to, schedule.total_amount().unwrap() * i as u128);
		let to_lookup = lookup_of_account(to.clone());

		let mut schedules = vec![];
		for _ in 0..i {
			schedule.start = i;
			schedules.push(schedule.clone());
		}
	}: _(RawOrigin::Root, to_lookup, schedules)
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(NATIVE, &to),
			schedule.total_amount().unwrap() * i as u128
		);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
