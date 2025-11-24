// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

use super::*;
use frame_benchmarking::v2::*;
use frame_support::{assert_ok, traits::fungibles::Mutate};
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::vec;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<AccountId, CurrencyId, Balance> {
	fn setup_get_staking_currency_id_and_amount() -> Option<(CurrencyId, Balance)>;
	fn setup_get_treasury_account() -> Option<AccountId>;
}

impl<AccountId, CurrencyId, Balance> BenchmarkHelper<AccountId, CurrencyId, Balance> for () {
	fn setup_get_staking_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		None
	}
	fn setup_get_treasury_account() -> Option<AccountId> {
		None
	}
}

#[benchmarks(
	where
	T: Config + pallet_balances::Config<Balance = BalanceOf<T>> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = BalanceOf<T>>,
)]
mod benchmarks {
	use super::*;

	// `transfer` non-native currency
	#[benchmark]
	fn transfer_non_native_currency() {
		let from: T::AccountId = account("from", 0, 0);

		let (staking_currency_id, amount) =
			<T as Config>::BenchmarkHelper::setup_get_staking_currency_id_and_amount().unwrap();

		T::MultiCurrency::set_balance(staking_currency_id, &from, amount);

		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());

		#[extrinsic_call]
		transfer(RawOrigin::Signed(from), to_lookup, staking_currency_id, amount);

		assert_eq!(Pallet::<T>::total_balance(staking_currency_id, &to), amount);
	}

	// `transfer_native_currency` in worst case
	// * will create the `to` account.
	// * will kill the `from` account.
	#[benchmark]
	fn transfer_native_currency() {
		let from: T::AccountId = account("from", 0, 0);
		let who_lookup = T::Lookup::unlookup(from.clone());

		let existential_deposit = pallet_balances::Pallet::<T>::minimum_balance();
		let amount = existential_deposit.saturating_mul(1000u32.into());
		let update_amount: AmountOf<T> = amount.unique_saturated_into();

		assert_ok!(Pallet::<T>::update_balance(
			RawOrigin::Root.into(),
			who_lookup.clone(),
			T::GetNativeCurrencyId::get(),
			update_amount
		));

		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());

		#[extrinsic_call]
		transfer(
			RawOrigin::Signed(from),
			to_lookup,
			T::GetNativeCurrencyId::get(),
			amount,
		);

		assert_eq!(Pallet::<T>::total_balance(T::GetNativeCurrencyId::get(), &to), amount);
	}

	// `update_balance` for non-native currency
	#[benchmark]
	fn update_balance_non_native_currency() {
		let who: T::AccountId = account("who", 0, 0);
		let who_lookup = T::Lookup::unlookup(who.clone());

		let (staking_currency_id, amount) =
			<T as Config>::BenchmarkHelper::setup_get_staking_currency_id_and_amount().unwrap();

		let update_amount = amount.unique_saturated_into();

		#[extrinsic_call]
		update_balance(RawOrigin::Root, who_lookup, staking_currency_id, update_amount);

		assert_eq!(Pallet::<T>::total_balance(staking_currency_id, &who), amount);
	}

	// `update_balance` for native currency
	// * will create the `who` account.
	#[benchmark]
	fn update_balance_native_currency_creating() {
		let who: T::AccountId = account("who", 0, 0);
		let who_lookup = T::Lookup::unlookup(who.clone());

		let existential_deposit = pallet_balances::Pallet::<T>::minimum_balance();
		let balance = existential_deposit.saturating_mul(1000u32.into());
		let update_amount = balance.unique_saturated_into();

		#[extrinsic_call]
		update_balance(
			RawOrigin::Root,
			who_lookup,
			T::GetNativeCurrencyId::get(),
			update_amount,
		);

		assert_eq!(Pallet::<T>::total_balance(T::GetNativeCurrencyId::get(), &who), balance);
	}

	// `update_balance` for native currency
	// * will kill the `who` account.
	#[benchmark]
	fn update_balance_native_currency_killing() {
		let who: T::AccountId = account("who", 0, 0);
		let who_lookup = T::Lookup::unlookup(who.clone());

		let existential_deposit = pallet_balances::Pallet::<T>::minimum_balance();
		let balance = existential_deposit.saturating_mul(1000u32.into());
		let update_amount: AmountOf<T> = balance.unique_saturated_into();

		assert_ok!(Pallet::<T>::update_balance(
			RawOrigin::Root.into(),
			who_lookup.clone(),
			T::GetNativeCurrencyId::get(),
			update_amount
		));

		#[extrinsic_call]
		update_balance(
			RawOrigin::Root,
			who_lookup,
			T::GetNativeCurrencyId::get(),
			-update_amount,
		);

		assert_eq!(
			Pallet::<T>::total_balance(T::GetNativeCurrencyId::get(), &who),
			0u32.into()
		);
	}

	#[benchmark]
	fn sweep_dust(c: Linear<1, 3>) {
		let (staking_currency_id, amount) =
			<T as Config>::BenchmarkHelper::setup_get_staking_currency_id_and_amount().unwrap();

		let treasury: T::AccountId = <T as Config>::BenchmarkHelper::setup_get_treasury_account().unwrap();
		let accounts: Vec<T::AccountId> = vec!["alice", "bob", "charlie"]
			.into_iter()
			.map(|x| account(x, 0, 0))
			.collect();

		let staking_currency_id_ed = orml_tokens::Pallet::<T>::minimum_balance(staking_currency_id);
		let dust_balance = staking_currency_id_ed.saturating_sub(1u32.into());

		accounts.iter().for_each(|account| {
			orml_tokens::Accounts::<T>::insert(
				account,
				staking_currency_id,
				orml_tokens::AccountData {
					free: dust_balance,
					frozen: 0u32.into(),
					reserved: 0u32.into(),
				},
			);
		});

		T::MultiCurrency::set_balance(staking_currency_id, &treasury, amount);

		#[extrinsic_call]
		_(RawOrigin::Root, staking_currency_id, (&accounts[..c as usize]).to_vec());

		(&accounts[..c as usize]).iter().for_each(|account| {
			assert_eq!(
				orml_tokens::Accounts::<T>::contains_key(account, staking_currency_id),
				false
			);
		});
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(staking_currency_id, &treasury),
			amount + (dust_balance * c.into())
		);
	}

	#[benchmark]
	fn force_set_lock() {
		let who: T::AccountId = account("who", 0, 0);
		let who_lookup = T::Lookup::unlookup(who.clone());
		let lock_id: LockIdentifier = *b"aca/test";

		let (staking_currency_id, amount) =
			<T as Config>::BenchmarkHelper::setup_get_staking_currency_id_and_amount().unwrap();

		T::MultiCurrency::set_balance(staking_currency_id, &who, amount);

		#[extrinsic_call]
		_(RawOrigin::Root, who_lookup, staking_currency_id, amount, lock_id);

		assert_eq!(
			orml_tokens::Pallet::<T>::locks(&who, staking_currency_id),
			vec![orml_tokens::BalanceLock {
				id: lock_id,
				amount: amount
			}]
		);
	}

	#[benchmark]
	fn force_remove_lock() {
		let who: T::AccountId = account("who", 0, 0);
		let who_lookup = T::Lookup::unlookup(who.clone());
		let lock_id: LockIdentifier = *b"aca/test";

		let (staking_currency_id, amount) =
			<T as Config>::BenchmarkHelper::setup_get_staking_currency_id_and_amount().unwrap();

		T::MultiCurrency::set_balance(staking_currency_id, &who, amount);

		assert_ok!(Pallet::<T>::force_set_lock(
			RawOrigin::Root.into(),
			who_lookup.clone(),
			staking_currency_id,
			amount,
			lock_id
		));
		assert_eq!(
			orml_tokens::Pallet::<T>::locks(&who, staking_currency_id),
			vec![orml_tokens::BalanceLock {
				id: lock_id,
				amount: amount
			}]
		);

		#[extrinsic_call]
		_(RawOrigin::Root, who_lookup, staking_currency_id, lock_id);

		assert_eq!(orml_tokens::Pallet::<T>::locks(&who, staking_currency_id), vec![]);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
