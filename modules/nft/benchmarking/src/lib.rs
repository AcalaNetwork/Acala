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

//! Benchmarks for the nft module.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{AccountIdConversion, StaticLookup, UniqueSaturatedInto};

use module_nft::*;
use orml_traits::BasicCurrencyExtended;
use primitives::Balance;

pub struct Module<T: Config>(module_nft::Module<T>);

pub trait Config: module_nft::Config + orml_nft::Config + pallet_proxy::Config + module_currencies::Config {}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

benchmarks! {
	// create NFT class
	create_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let base_currency_amount = dollar(1000);

		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(caller), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))

	// mint NFT token
	mint {
		let i in 1 .. 1000;

		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(module_account), to_lookup, 0u32.into(), vec![1], i)

	// transfer NFT token to another account
	transfer {
		let caller: T::AccountId = account("caller", 0, SEED);
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		module_nft::Module::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), caller_lookup, (0u32.into(), 0u32.into()))

	// burn NFT token
	burn {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let base_currency_amount = dollar(1000);
		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as module_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		module_nft::Module::<T>::mint(RawOrigin::Signed(module_account).into(), to_lookup, 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), (0u32.into(), 0u32.into()))

	// destroy NFT class
	destroy_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);
		let to_lookup = T::Lookup::unlookup(to);

		let base_currency_amount = dollar(1000);

		<T as module_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
	}: _(RawOrigin::Signed(module_account), 0u32.into(), to_lookup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_create_class() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_create_class::<Runtime>());
		});
	}

	#[test]
	fn test_mint() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_mint::<Runtime>());
		});
	}

	#[test]
	fn test_transfer() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_transfer::<Runtime>());
		});
	}

	#[test]
	fn test_burn() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_burn::<Runtime>());
		});
	}

	#[test]
	fn test_destroy_class() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_destroy_class::<Runtime>());
		});
	}
}
