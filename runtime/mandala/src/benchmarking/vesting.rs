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

use super::utils::{lookup_of_account, set_aca_balance};
use crate::{
	dollar, AcalaTreasuryModuleId, AccountId, AccountIdConversion, Balance, BlockNumber, Currencies, MinVestedTransfer,
	Runtime, System, Vesting, ACA,
};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_vesting::VestingSchedule;

pub type Schedule = VestingSchedule<BlockNumber, Balance>;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, orml_vesting }

	_ {}

	vested_transfer {
		let schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: MinVestedTransfer::get(),
		};

		// extra 1 dollar to pay fees
		let from: AccountId = AcalaTreasuryModuleId::get().into_account();
		set_aca_balance(&from, schedule.total_amount().unwrap() + dollar(ACA));

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, schedule.clone())
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::total_balance(ACA, &to),
			schedule.total_amount().unwrap()
		);
	}

	claim {
		let i in 1 .. orml_vesting::MAX_VESTINGS as u32;

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: MinVestedTransfer::get(),
		};

		let from: AccountId = AcalaTreasuryModuleId::get().into_account();
		// extra 1 dollar to pay fees
		set_aca_balance(&from, schedule.total_amount().unwrap() * i as u128 + dollar(ACA));

		let to: AccountId = account("to", 0, SEED);
		let to_lookup = lookup_of_account(to.clone());

		for _ in 0..i {
			schedule.start = i;
			Vesting::vested_transfer(RawOrigin::Signed(from.clone()).into(), to_lookup.clone(), schedule.clone())?;
		}
		System::set_block_number(schedule.end().unwrap() + 1u32);
	}: _(RawOrigin::Signed(to.clone()))
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &to),
			schedule.total_amount().unwrap() * i as u128,
		);
	}

	update_vesting_schedules {
		let i in 1 .. orml_vesting::MAX_VESTINGS as u32;

		let mut schedule = Schedule {
			start: 0,
			period: 2,
			period_count: 3,
			per_period: MinVestedTransfer::get(),
		};

		let to: AccountId = account("to", 0, SEED);
		set_aca_balance(&to, schedule.total_amount().unwrap() * i as u128);
		let to_lookup = lookup_of_account(to.clone());

		let mut schedules = vec![];
		for _ in 0..i {
			schedule.start = i;
			schedules.push(schedule.clone());
		}
	}: _(RawOrigin::Root, to_lookup, schedules)
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &to),
			schedule.total_amount().unwrap() * i as u128
		);
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
	fn vested_transfer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_vested_transfer());
		});
	}

	#[test]
	fn claim() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_claim());
		});
	}

	#[test]
	fn update_vesting_shedules() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_vesting_schedules());
		});
	}
}
