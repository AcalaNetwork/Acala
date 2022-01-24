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

//! Benchmarks for the Homa Lite module.

#![cfg(feature = "runtime-benchmarks")]

pub use crate::*;
pub use frame_benchmarking::{account, benchmarks};
pub use frame_support::traits::Get;
pub use frame_system::RawOrigin;

pub struct Module<T: Config>(crate::Pallet<T>);

const SEED: u32 = 0;

benchmarks! {
	on_initialize {
		let _ = crate::Pallet::<T>::set_staking_interest_rate_per_update(
			RawOrigin::Root.into(),
			Permill::from_percent(1)
		);
		let _ = crate::Pallet::<T>::set_total_staking_currency(RawOrigin::Root.into(), 1_000_000_000_000_000_000);
	}: {
		let _ = crate::Pallet::<T>::on_initialize(<T as frame_system::Config>::BlockNumber::default());
	}

	on_initialize_without_work {}: {
		// interest rate is not calculated becasue `set_staking_interest_rate_per_update` is not called.
		let _ = crate::Pallet::<T>::on_initialize(<T as frame_system::Config>::BlockNumber::default());
	}

	mint {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		<T as module::Config>::Currency::deposit(T::StakingCurrencyId::get(), &caller, amount)?;
		let _ = crate::Pallet::<T>::set_minting_cap(RawOrigin::Root.into(), amount)?;
	}: _(RawOrigin::Signed(caller), amount)

	mint_for_requests {
		let amount = 1_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller1: T::AccountId = account("callera", 0, SEED);
		let caller2: T::AccountId = account("callerb", 0, SEED);
		let caller3: T::AccountId = account("callerc", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller1, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller2, amount)?;
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller3, amount)?;
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller1).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller2.clone()).into(), amount, Permill::default());
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller3.clone()).into(), amount, Permill::default());

		<T as module::Config>::Currency::deposit(T::StakingCurrencyId::get(), &caller, amount*2)?;
		crate::Pallet::<T>::set_minting_cap(RawOrigin::Root.into(), amount*2)?;
	}: _(RawOrigin::Signed(caller), amount*2, vec![caller2, caller3])

	set_total_staking_currency {}: _(RawOrigin::Root, 1_000_000_000_000)

	adjust_total_staking_currency {}: _(RawOrigin::Root, AmountOf::<T>::max_value())

	adjust_available_staking_balance_with_no_matches {}: {
		let _ = crate::Pallet::<T>::adjust_available_staking_balance(RawOrigin::Root.into(), AmountOf::<T>::max_value(), 0);
	}

	set_minting_cap {
	}: _(RawOrigin::Root, 1_000_000_000_000_000_000)

	set_xcm_dest_weight {
	}: _(RawOrigin::Root, 1_000_000_000)

	request_redeem {
		let amount = 1_000_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller, amount)?;
	}: _(RawOrigin::Signed(caller), amount, Permill::default())

	schedule_unbond {}: _(RawOrigin::Root, 1_000_000_000_000, <T as frame_system::Config>::BlockNumber::default())

	replace_schedule_unbond {}: _(RawOrigin::Root, vec![(1_000_000, <T as frame_system::Config>::BlockNumber::default()), (1_000_000_000, <T as frame_system::Config>::BlockNumber::default())])

	set_staking_interest_rate_per_update {}: _(RawOrigin::Root, Permill::default())
	redeem_with_available_staking_balance {
		let amount = 1_000_000_000_000_000;
		let caller: T::AccountId = account("caller", 0, SEED);
		<T as module::Config>::Currency::deposit(T::LiquidCurrencyId::get(), &caller, amount)?;
		let _ = crate::Pallet::<T>::adjust_available_staking_balance(RawOrigin::Root.into(), AmountOf::<T>::max_value(), 1);
		let _ = crate::Pallet::<T>::request_redeem(RawOrigin::Signed(caller.clone()).into(), amount, Permill::default());
	}: {
		let _ = crate::Pallet::<T>::process_redeem_requests_with_available_staking_balance(&caller);
	}

	xcm_unbond {}: {
		let _ = crate::Pallet::<T>::process_scheduled_unbond(1_000_000_000_000_000);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_support::assert_ok;

	#[test]
	fn test_on_initialize() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_on_initialize());
		});
	}
	#[test]
	fn test_on_initialize_without_work() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_on_initialize_without_work());
		});
	}
	#[test]
	fn test_mint() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_mint());
		});
	}
	#[test]
	fn test_mint_for_requests() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_mint_for_requests());
		});
	}
	#[test]
	fn test_set_total_staking_currency() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_total_staking_currency());
		});
	}
	#[test]
	fn test_adjust_total_staking_currency() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_adjust_total_staking_currency());
		});
	}
	#[test]
	fn test_adjust_available_staking_balance_with_no_matches() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_adjust_available_staking_balance_with_no_matches());
		});
	}

	#[test]
	fn test_set_minting_cap() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_minting_cap());
		});
	}
	#[test]
	fn test_set_xcm_dest_weight() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_xcm_dest_weight());
		});
	}
	#[test]
	fn test_request_redeem() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_request_redeem());
		});
	}
	#[test]
	fn test_schedule_unbond() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_schedule_unbond());
		});
	}
	#[test]
	fn test_replace_schedule_unbond() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_replace_schedule_unbond());
		});
	}

	#[test]
	fn test_set_staking_interest_rate_per_update() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_set_staking_interest_rate_per_update());
		});
	}
	#[test]
	fn test_redeem_with_available_staking_balance() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_redeem_with_available_staking_balance());
		});
	}
	#[test]
	fn test_xcm_unbond() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_xcm_unbond());
		});
	}
}
