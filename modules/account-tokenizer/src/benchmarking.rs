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
pub use module_foreign_state_oracle::RawOrigin as OracleRawOrigin;

pub struct Module<T: Config>(crate::Pallet<T>);

const SEED: u32 = 0;

benchmarks! {
	initialize_nft_class {
		<T as module::Config>::Currency::::make_free_balance_be(&T::TreasuryAccount::get(), 1_000_000_000_000_000);
	}: {
		crate::Pallet::<T>::on_runtime_upgrade();
	}

	request_mint {
		<T as module::Config>::Currency::make_free_balance_be(&T::TreasuryAccount::get(), 1_000_000_000_000_000);
		let caller = AccountId::from([0u8; 32]);
		<T as module::Config>::Currency::make_free_balance_be(&caller, 1_000_000_000_000_000);
		let proxy = AccountId::new(hex!["7342619566cac76247062ffd59cd3fb3ffa3350dc6a5087938b9d1c46b286da3"]);
		crate::Pallet::<T>::on_runtime_upgrade();
	}: _(RawOrigin::Signed(caller), proxy, 1, 0, 0)

	confirm_mint_request {
		<T as module::Config>::Currency::make_free_balance_be(&T::TreasuryAccount::get(), 1_000_000_000_000_000);
		let caller = AccountId::from([0u8; 32]);
		<T as module::Config>::Currency::make_free_balance_be(&caller, 1_000_000_000_000_000);
		let proxy = AccountId::new(hex!["7342619566cac76247062ffd59cd3fb3ffa3350dc6a5087938b9d1c46b286da3"]);
		crate::Pallet::<T>::on_runtime_upgrade();
		crate::Pallet::<T>::request_mint(
			Origin::signed(caller.clone()),
			proxy.clone(),
			1,
			0,
			0
		);
	}: {
		crate::Pallet::<T>::accept_mint_request(caller, proxy);
	}

	request_burn {
		<T as module::Config>::Currency::make_free_balance_be(&T::TreasuryAccount::get(), 1_000_000_000_000_000);
		let caller = AccountId::from([0u8; 32]);
		<T as module::Config>::Currency::make_free_balance_be(&caller, 1_000_000_000_000_000);
		let proxy = AccountId::new(hex!["7342619566cac76247062ffd59cd3fb3ffa3350dc6a5087938b9d1c46b286da3"]);
		crate::Pallet::<T>::on_runtime_upgrade();
		crate::Pallet::<T>::request_mint(
			Origin::signed(caller.clone()),
			proxy.clone(),
			1,
			0,
			0
		);
		crate::Pallet::<T>::confirm_mint_request(
			OracleRawOrigin { vec![1] }.into(),
			caller.clone(),
			proxy.clone(),
		)
	}: _(RawOrigin::Signed(caller.clone()), proxy, caller)

	confirm_burn_account_token {
		<T as module::Config>::Currency::make_free_balance_be(&T::TreasuryAccount::get(), 1_000_000_000_000_000);
		let caller = AccountId::from([0u8; 32]);
		<T as module::Config>::Currency::make_free_balance_be(&caller, 1_000_000_000_000_000);
		let proxy = AccountId::new(hex!["7342619566cac76247062ffd59cd3fb3ffa3350dc6a5087938b9d1c46b286da3"]);
		crate::Pallet::<T>::on_runtime_upgrade();
		crate::Pallet::<T>::request_mint(
			Origin::signed(caller.clone()),
			proxy.clone(),
			1,
			0,
			0
		);
		crate::Pallet::<T>::confirm_mint_request(
			OracleRawOrigin { vec![1] }.into(),
			caller.clone(),
			proxy.clone(),
		);
		crate::Pallet::<T>::request_burn(RawOrigin::Signed(caller.clone()), proxy, caller);
	}: _(OracleRawOrigin { vec![] }.into(), proxy, caller)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::*;
	use frame_support::assert_ok;

	#[test]
	fn test_initialize_nft_class() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_initialize_nft_class());
		});
	}
	#[test]
	fn test_request_mint() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_request_mint());
		});
	}
	#[test]
	fn test_confirm_mint_request() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_confirm_mint_request());
		});
	}
	#[test]
	fn test_request_burn() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_request_burn());
		});
	}
	#[test]
	fn test_confirm_burn_account_token() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Pallet::<Runtime>::test_benchmark_confirm_burn_account_token());
		});
	}
}
