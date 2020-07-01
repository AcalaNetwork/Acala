use super::utils::{dollars, lookup_of_account, set_aca_balance};
use crate::{AccountId, Currencies, CurrencyId, NewAccountDeposit, Runtime, System, Vesting};

use sp_std::prelude::*;

use frame_benchmarking::account;
use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrency;
use orml_vesting::VestingScheduleOf;

type Schedule = VestingScheduleOf<Runtime>;

const SEED: u32 = 0;
const MAX_USER_INDEX: u32 = 1000;
const MAX_PERIOD: u32 = 1000;
const MAX_PERIOD_COUNT: u32 = 1000;
const MAX_PER_PERIOD: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_vesting }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let p in 1 .. MAX_PERIOD => ();
		let c in 1 .. MAX_PERIOD_COUNT => ();
		let a in 1 .. MAX_PER_PERIOD => ();
	}

	vested_transfer {
		let u in ...;
		let p in ...;
		let c in ...;
		let a in ...;

		let schedule = Schedule {
			start: 0,
			period: p,
			period_count: c,
			per_period: dollars(a),
		};

		let from = account("from", u, SEED);
		// extra 1 dollar to pay fees
		set_aca_balance(&from, schedule.total_amount().unwrap() + dollars(1u32));

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Signed(from), to_lookup, schedule.clone())
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::total_balance(CurrencyId::ACA, &to),
			schedule.total_amount().unwrap()
		);
	}

	claim {
		let u in ...;
		let p in ...;
		let c in ...;
		let a in ...;

		let schedule = Schedule {
			start: 0,
			period: p,
			period_count: c,
			per_period: dollars(a),
		};

		let from = account("from", u, SEED);
		// extra 1 dollar to pay fees
		set_aca_balance(&from, schedule.total_amount().unwrap() + dollars(1u32));

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());

		let _ = Vesting::vested_transfer(RawOrigin::Signed(from).into(), to_lookup, schedule.clone());
		System::set_block_number(schedule.end().unwrap() + 1u32);
	}: _(RawOrigin::Signed(to.clone()))
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(CurrencyId::ACA, &to),
			schedule.total_amount().unwrap() - NewAccountDeposit::get()
		);
	}

	// claim 10 vesting schedule at once
	claim_ten {
		let u in ...;
		let p in ...;
		let c in ...;
		let a in ...;

		let schedule = Schedule {
			start: 0,
			period: p,
			period_count: c,
			per_period: dollars(a),
		};

		let from = account("from", u, SEED);
		// extra 1 dollar to pay fees
		set_aca_balance(&from, schedule.total_amount().unwrap() * 10 + dollars(1u32));

		let to: AccountId = account("to", u, SEED);
		let to_lookup = lookup_of_account(to.clone());

		for _ in 0..10 {
			let _ = Vesting::vested_transfer(RawOrigin::Signed(from.clone()).into(), to_lookup.clone(), schedule.clone());
		}
		System::set_block_number(schedule.end().unwrap() + 1u32);
	}: claim(RawOrigin::Signed(to.clone()))
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(CurrencyId::ACA, &to),
			schedule.total_amount().unwrap() * 10 - NewAccountDeposit::get()
		);
	}

	update_vesting_schedules {
		let u in ...;
		let p in ...;
		let c in ...;
		let a in ...;

		let schedule = Schedule {
			start: 0,
			period: p,
			period_count: c,
			per_period: dollars(a),
		};

		let to: AccountId = account("to", u, SEED);
		set_aca_balance(&to, schedule.total_amount().unwrap());
		let to_lookup = lookup_of_account(to.clone());
	}: _(RawOrigin::Root, to_lookup, vec![schedule.clone()])
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(CurrencyId::ACA, &to),
			schedule.total_amount().unwrap()
		);
	}

	// update with 10 vesting schedules
	update_with_ten_vesting_schedules {
		let u in ...;
		let p in ...;
		let c in ...;
		let a in ...;

		let schedule = Schedule {
			start: 0,
			period: p,
			period_count: c,
			per_period: dollars(a),
		};

		let to: AccountId = account("to", u, SEED);
		// extra 1 dollar to pay fees
		set_aca_balance(&to, schedule.total_amount().unwrap() * 10);
		let to_lookup = lookup_of_account(to.clone());

		let mut schedules = vec![];
		for _ in 0..10 {
			schedules.push(schedule.clone());
		}
	}: update_vesting_schedules(RawOrigin::Root, to_lookup, schedules)
	verify {
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(CurrencyId::ACA, &to),
			schedule.total_amount().unwrap() * 10
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
	fn claim_ten() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_claim_ten());
		});
	}

	#[test]
	fn update_vesting_shedules() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_vesting_schedules());
		});
	}

	#[test]
	fn update_with_ten_vesting_shedules() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_update_with_ten_vesting_schedules());
		});
	}
}
