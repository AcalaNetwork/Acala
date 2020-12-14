//! Benchmarks for the nft module.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{AccountIdConversion, UniqueSaturatedInto};

use module_nft::*;
use orml_traits::BasicCurrencyExtended;
use primitives::Balance;

pub struct Module<T: Config>(module_nft::Module<T>);

pub trait Config: module_nft::Config + orml_nft::Config + pallet_proxy::Config + orml_currencies::Config {}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

benchmarks! {
	_ { }

	// create NFT class
	create_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let base_currency_amount = dollar(1000);

		<T as orml_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(caller), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))

	// mint NFT token
	mint {
		let i in 1 .. 1000;

		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);

		let base_currency_amount = dollar(1000);
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
	}: _(RawOrigin::Signed(module_account), to, 0u32.into(), vec![1], i)

	// transfer NFT token to another account
	transfer {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);

		let base_currency_amount = dollar(1000);
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller.clone()).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		module_nft::Module::<T>::mint(RawOrigin::Signed(module_account).into(), to.clone(), 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), caller, (0u32.into(), 0u32.into()))

	// burn NFT token
	burn {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);

		let base_currency_amount = dollar(1000);
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&module_account, base_currency_amount.unique_saturated_into())?;
		module_nft::Module::<T>::mint(RawOrigin::Signed(module_account).into(), to.clone(), 0u32.into(), vec![1], 1)?;
	}: _(RawOrigin::Signed(to), (0u32.into(), 0u32.into()))

	// destroy NFT class
	destroy_class {
		let caller: T::AccountId = account("caller", 0, SEED);
		let to: T::AccountId = account("to", 0, SEED);

		let base_currency_amount = dollar(1000);
		<T as orml_currencies::Config>::NativeCurrency::update_balance(&caller, base_currency_amount.unique_saturated_into())?;

		let module_account: T::AccountId = T::ModuleId::get().into_sub_account(orml_nft::Module::<T>::next_class_id());
		module_nft::Module::<T>::create_class(RawOrigin::Signed(caller).into(), vec![1], Properties(ClassProperty::Transferable | ClassProperty::Burnable))?;
	}: _(RawOrigin::Signed(module_account), 0u32.into(), to)
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
