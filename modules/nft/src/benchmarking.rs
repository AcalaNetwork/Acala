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

//! Benchmarks for the nft module.

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_support::{dispatch::DispatchClass, traits::Get};
use frame_system::RawOrigin;
use sp_runtime::traits::{AccountIdConversion, StaticLookup, UniqueSaturatedInto};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;

use primitives::Balance;

pub struct Module<T: Config>(crate::Pallet<T>);

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn test_attr() -> Attributes {
	let mut attr: Attributes = BTreeMap::new();
	for i in 0..30 {
		attr.insert(vec![i], vec![0; 64]);
	}
	attr
}

fn create_token_class<T: Config>(caller: T::AccountId) -> T::AccountId {
	let base_currency_amount = dollar(1000);
	<T as Config>::Currency::make_free_balance_be(&caller, base_currency_amount.unique_saturated_into());

	let module_account: T::AccountId =
		T::PalletId::get().into_sub_account_truncating(orml_nft::Pallet::<T>::next_class_id());

	assert_ok!(Pallet::<T>::create_class(
		RawOrigin::Signed(caller).into(),
		vec![1],
		Properties(
			ClassProperty::Transferable
				| ClassProperty::Burnable
				| ClassProperty::Mintable
				| ClassProperty::ClassPropertiesMutable,
		),
		test_attr(),
	));

	<T as Config>::Currency::make_free_balance_be(&module_account, base_currency_amount.unique_saturated_into());

	module_account
}

#[benchmarks]
mod benchmarks {
	use super::*;

	// create NFT class
	#[benchmark]
	fn create_class() {
		let caller: T::AccountId = account("caller", 0, 0);
		let base_currency_amount = dollar(1000);

		<T as Config>::Currency::make_free_balance_be(&caller, base_currency_amount.unique_saturated_into());

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable),
			test_attr(),
		);
	}

	// mint NFT token
	#[benchmark]
	fn mint(i: Linear<1, 1000>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to);

		let module_account = create_token_class::<T>(caller);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(module_account),
			to_lookup,
			0u32.into(),
			vec![1],
			test_attr(),
			i,
		);
	}

	// transfer NFT token to another account
	#[benchmark]
	fn transfer() {
		let caller: T::AccountId = account("caller", 0, 0);
		let caller_lookup = T::Lookup::unlookup(caller.clone());
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller);

		assert_ok!(Pallet::<T>::mint(
			RawOrigin::Signed(module_account).into(),
			to_lookup,
			0u32.into(),
			vec![1],
			test_attr(),
			1
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(to), caller_lookup, (0u32.into(), 0u32.into()));
	}

	// burn NFT token
	#[benchmark]
	fn burn() {
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller);

		assert_ok!(Pallet::<T>::mint(
			RawOrigin::Signed(module_account).into(),
			to_lookup,
			0u32.into(),
			vec![1],
			test_attr(),
			1
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(to), (0u32.into(), 0u32.into()));
	}

	// burn NFT token with remark
	#[benchmark]
	fn burn_with_remark(b: Linear<0, { *T::BlockLength::get().max.get(DispatchClass::Normal) as u32 }>) {
		let remark_message = vec![1; b as usize];
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to.clone());

		let module_account = create_token_class::<T>(caller);

		assert_ok!(Pallet::<T>::mint(
			RawOrigin::Signed(module_account).into(),
			to_lookup,
			0u32.into(),
			vec![1],
			test_attr(),
			1
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(to), (0u32.into(), 0u32.into()), remark_message);
	}

	// destroy NFT class
	#[benchmark]
	fn destroy_class() {
		let caller: T::AccountId = account("caller", 0, 0);
		let caller_lookup = T::Lookup::unlookup(caller.clone());

		let module_account = create_token_class::<T>(caller);

		#[extrinsic_call]
		_(RawOrigin::Signed(module_account), 0u32.into(), caller_lookup);
	}

	#[benchmark]
	fn update_class_properties() {
		let caller: T::AccountId = account("caller", 0, 0);

		let module_account = create_token_class::<T>(caller);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(module_account),
			0u32.into(),
			Properties(ClassProperty::Transferable.into()),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
