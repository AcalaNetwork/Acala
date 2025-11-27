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
use frame_support::assert_ok;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use module_support::{Price, Rate};
use primitives::orml_traits::{Change, MultiCurrency};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_runtime::FixedPointNumber;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<CurrencyId, AccountId> {
	fn setup_collateral_currency_ids() -> Vec<CurrencyId>;
	fn setup_stable_currency_id_and_amount() -> Option<(CurrencyId, Balance)>;
	fn setup_staking_currency_id_and_amount() -> Option<(CurrencyId, Balance)>;
	fn setup_liquid_currency_id_and_amount() -> Option<(CurrencyId, Balance)>;
	fn setup_dex_pools(maker: AccountId);
	fn setup_feed_price(currency_id: CurrencyId, price: Price);
}

impl<CurrencyId, AccountId> BenchmarkHelper<CurrencyId, AccountId> for () {
	fn setup_collateral_currency_ids() -> Vec<CurrencyId> {
		vec![]
	}
	fn setup_stable_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		None
	}
	fn setup_staking_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		None
	}
	fn setup_liquid_currency_id_and_amount() -> Option<(CurrencyId, Balance)> {
		None
	}
	fn setup_dex_pools(_maker: AccountId) {}
	fn setup_feed_price(_currency_id: CurrencyId, _price: Price) {}
}

#[benchmarks(
	where
		T: Config + orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>,
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn authorize() {
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to);

		// set balance
		let _ = <T as Config>::Currency::deposit_creating(
			&caller,
			T::DepositPerAuthorization::get() + <T as Config>::Currency::minimum_balance(),
		);

		let currency_id = <T as Config>::BenchmarkHelper::setup_collateral_currency_ids()[0];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), currency_id, to_lookup);
	}

	#[benchmark]
	fn unauthorize() {
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to);

		// set balance
		let _ = <T as Config>::Currency::deposit_creating(
			&caller,
			T::DepositPerAuthorization::get() + <T as Config>::Currency::minimum_balance(),
		);

		let currency_id = <T as Config>::BenchmarkHelper::setup_collateral_currency_ids()[0];

		assert_ok!(Pallet::<T>::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			currency_id,
			to_lookup.clone()
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), currency_id, to_lookup);
	}

	#[benchmark]
	fn unauthorize_all(c: Linear<0, { <T as Config>::BenchmarkHelper::setup_collateral_currency_ids().len() as u32 }>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = account("to", 0, 0);
		let to_lookup = T::Lookup::unlookup(to);

		let currency_ids = <T as Config>::BenchmarkHelper::setup_collateral_currency_ids();

		// set balance
		let _ = <T as Config>::Currency::deposit_creating(
			&caller,
			T::DepositPerAuthorization::get().saturating_mul(c.into()) + <T as Config>::Currency::minimum_balance(),
		);

		for i in 0..c {
			assert_ok!(Pallet::<T>::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				currency_ids[i as usize],
				to_lookup.clone(),
			));
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(caller));
	}

	// `adjust_loan`, best case:
	// adjust both collateral and debit
	#[benchmark]
	fn adjust_loan() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(staking_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(staking_amount, stable_amount).saturating_mul_int(collateral_value);

		// set balance
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			collateral_amount * 2
		));

		// feed price
		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller),
			staking_currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount,
		);
	}

	#[benchmark]
	fn transfer_loan_from() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let to: T::AccountId = whitelisted_caller();
		let to_lookup = T::Lookup::unlookup(to.clone());

		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(staking_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(staking_amount, stable_amount).saturating_mul_int(collateral_value);

		// set balance
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			collateral_amount * 2
		));
		let _ = <T as Config>::Currency::deposit_creating(
			&caller,
			T::DepositPerAuthorization::get() + <T as Config>::Currency::minimum_balance(),
		);

		// feed price
		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// initialize sender's loan
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount,
		));

		// authorize receiver
		assert_ok!(Pallet::<T>::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			to_lookup.clone(),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(to), staking_currency_id, to_lookup);
	}

	#[benchmark]
	fn close_loan_has_debit_by_dex() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(staking_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(staking_amount, stable_amount).saturating_mul_int(collateral_value);

		// set balance and inject liquidity
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			(10 * collateral_amount) + orml_tokens::Pallet::<T>::minimum_balance(staking_currency_id)
		));

		let maker: T::AccountId = account("maker", 0, 0);
		<T as Config>::BenchmarkHelper::setup_dex_pools(maker);

		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// initialize sender's loan
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			(10 * collateral_amount).try_into().unwrap(),
			debit_amount,
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), staking_currency_id, collateral_amount);
	}

	#[benchmark]
	fn expand_position_collateral() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(staking_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let collateral_value = 10 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(staking_amount, stable_amount).saturating_mul_int(collateral_value);

		// set balance and inject liquidity for trading path
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			(10 * collateral_amount) + orml_tokens::Pallet::<T>::minimum_balance(staking_currency_id)
		));

		let maker: T::AccountId = account("maker", 0, 0);
		<T as Config>::BenchmarkHelper::setup_dex_pools(maker);

		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// initialize sender's loan
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount.try_into().unwrap(),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), staking_currency_id, debit_value, 0);
	}

	#[benchmark]
	fn shrink_position_debit() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(staking_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(1000 * staking_amount, 1000 * stable_amount)
			.saturating_mul_int(collateral_value);

		// set balance and inject liquidity for trading path
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			(10 * collateral_amount) + orml_tokens::Pallet::<T>::minimum_balance(staking_currency_id)
		));

		let maker: T::AccountId = account("maker", 0, 0);
		<T as Config>::BenchmarkHelper::setup_dex_pools(maker);

		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// initialize sender's loan
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount.try_into().unwrap(),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), staking_currency_id, collateral_amount / 5, 0);
	}

	#[benchmark]
	fn transfer_debit() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();
		let (liquid_currency_id, liquid_amount) =
			<T as Config>::BenchmarkHelper::setup_liquid_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			staking_currency_id,
			&caller,
			100_000 * staking_amount
		));
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			liquid_currency_id,
			&caller,
			100_000 * liquid_amount
		));

		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			staking_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(10_000 * stable_amount),
		));
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			liquid_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(10_000 * stable_amount),
		));

		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			staking_currency_id,
			(10_000 * staking_amount).try_into().unwrap(),
			(1_000 * stable_amount).try_into().unwrap()
		));
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			liquid_currency_id,
			(10_000 * liquid_amount).try_into().unwrap(),
			(1_000 * stable_amount).try_into().unwrap()
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller),
			liquid_currency_id,
			staking_currency_id,
			stable_amount,
		);
	}

	#[benchmark]
	fn precompile_get_current_collateral_ratio() {
		let (stable_currency_id, stable_amount) =
			<T as Config>::BenchmarkHelper::setup_stable_currency_id_and_amount().unwrap();
		let (staking_currency_id, staking_amount) =
			<T as Config>::BenchmarkHelper::setup_staking_currency_id_and_amount().unwrap();
		let (liquid_currency_id, liquid_amount) =
			<T as Config>::BenchmarkHelper::setup_liquid_currency_id_and_amount().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);

		let debit_value = 100 * stable_amount;
		let debit_exchange_rate = module_cdp_engine::Pallet::<T>::get_debit_exchange_rate(liquid_currency_id);
		let debit_amount = debit_exchange_rate
			.reciprocal()
			.unwrap()
			.saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount =
			Price::saturating_from_rational(liquid_amount, stable_amount).saturating_mul_int(collateral_value);

		// set balance and inject liquidity
		assert_ok!(orml_tokens::Pallet::<T>::deposit(
			liquid_currency_id,
			&caller,
			(10 * collateral_amount) + orml_tokens::Pallet::<T>::minimum_balance(liquid_currency_id)
		));

		let maker: T::AccountId = account("maker", 0, 0);
		<T as Config>::BenchmarkHelper::setup_dex_pools(maker);

		<T as Config>::BenchmarkHelper::setup_feed_price(staking_currency_id, Price::one());

		// set risk params
		assert_ok!(module_cdp_engine::Pallet::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			liquid_currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		));

		// initialize sender's loan
		assert_ok!(Pallet::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			liquid_currency_id,
			(10 * collateral_amount).try_into().unwrap(),
			debit_amount,
		));

		#[block]
		{
			Pallet::<T>::get_current_collateral_ratio(&caller, liquid_currency_id);
		}
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
