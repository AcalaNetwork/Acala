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
use frame_system::RawOrigin;
use sp_runtime::traits::Hash;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper<AccountId, CurrencyId, Balance> {
	fn setup_stable_currency_id() -> Option<CurrencyId>;
	fn setup_liquid_currency_id() -> Option<CurrencyId>;
	fn setup_enable_fee_pool() -> Option<(AccountId, Balance, Balance, Balance)>;
	fn setup_enable_stable_asset();
}

impl<AccountId, CurrencyId, Balance> BenchmarkHelper<AccountId, CurrencyId, Balance> for () {
	fn setup_stable_currency_id() -> Option<CurrencyId> {
		None
	}
	fn setup_liquid_currency_id() -> Option<CurrencyId> {
		None
	}
	fn setup_enable_fee_pool() -> Option<(AccountId, Balance, Balance, Balance)> {
		None
	}
	fn setup_enable_stable_asset() {}
}

#[benchmarks(
	where T: Config + pallet_balances::Config<Balance = PalletBalanceOf<T>> + orml_tokens::Config<CurrencyId = CurrencyId, Balance = PalletBalanceOf<T>>,
	<T as Config>::RuntimeCall: From<frame_system::Call<T>>
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_alternative_fee_swap_path() {
		let caller: T::AccountId = account("caller", 0, 0);
		let stable_currency_id = <T as Config>::BenchmarkHelper::setup_stable_currency_id().unwrap();

		let native_ed = pallet_balances::Pallet::<T>::minimum_balance();
		let _ = T::Currency::deposit_creating(&caller, native_ed.saturating_mul(T::AlternativeFeeSwapDeposit::get()));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(caller.clone()),
			Some(vec![stable_currency_id, T::NativeCurrencyId::get()]),
		);

		assert_eq!(
			Pallet::<T>::alternative_fee_swap_path(&caller).unwrap().into_inner(),
			vec![stable_currency_id, T::NativeCurrencyId::get()]
		);
	}

	#[benchmark]
	fn enable_charge_fee_pool() {
		let stable_currency_id = <T as Config>::BenchmarkHelper::setup_stable_currency_id().unwrap();

		let (sub_account, stable_ed, pool_size, swap_threshold) =
			<T as Config>::BenchmarkHelper::setup_enable_fee_pool().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, stable_currency_id, pool_size, swap_threshold);

		let exchange_rate = Pallet::<T>::token_exchange_rate(stable_currency_id).unwrap();
		assert_eq!(Pallet::<T>::pool_size(stable_currency_id), pool_size);
		assert!(Pallet::<T>::token_exchange_rate(stable_currency_id).is_some());
		assert_eq!(
			orml_tokens::Pallet::<T>::free_balance(stable_currency_id, &sub_account),
			stable_ed
		);
		assert_eq!(pallet_balances::Pallet::<T>::free_balance(&sub_account), pool_size);

		frame_system::Pallet::<T>::assert_has_event(
			Event::ChargeFeePoolEnabled {
				sub_account,
				currency_id: stable_currency_id,
				exchange_rate,
				pool_size,
				swap_threshold,
			}
			.into(),
		);
	}

	#[benchmark]
	fn disable_charge_fee_pool() {
		let stable_currency_id = <T as Config>::BenchmarkHelper::setup_stable_currency_id().unwrap();

		let (_sub_account, stable_ed, pool_size, swap_threshold) =
			<T as Config>::BenchmarkHelper::setup_enable_fee_pool().unwrap();

		assert_ok!(Pallet::<T>::enable_charge_fee_pool(
			RawOrigin::Root.into(),
			stable_currency_id,
			pool_size,
			swap_threshold
		));

		#[extrinsic_call]
		_(RawOrigin::Root, stable_currency_id);

		frame_system::Pallet::<T>::assert_has_event(
			Event::ChargeFeePoolDisabled {
				currency_id: stable_currency_id,
				foreign_amount: stable_ed,
				native_amount: pool_size,
			}
			.into(),
		);
	}

	#[benchmark]
	fn with_fee_path() {
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let stable_currency_id = <T as Config>::BenchmarkHelper::setup_stable_currency_id().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let call = Box::new(frame_system::Call::remark_with_event { remark: vec![] }.into());

		let fee_swap_path: Vec<CurrencyId> = vec![stable_currency_id, T::NativeCurrencyId::get()];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), fee_swap_path.clone(), call);

		frame_system::Pallet::<T>::assert_has_event(
			frame_system::Event::<T>::Remarked {
				sender: caller,
				hash: T::Hashing::hash(&vec![]),
			}
			.into(),
		);
	}

	#[benchmark]
	fn with_fee_currency() {
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let stable_currency_id = <T as Config>::BenchmarkHelper::setup_stable_currency_id().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let call = Box::new(frame_system::Call::remark_with_event { remark: vec![] }.into());

		let (sub_account, _stable_ed, pool_size, swap_threshold) =
			<T as Config>::BenchmarkHelper::setup_enable_fee_pool().unwrap();
		assert_ok!(Pallet::<T>::enable_charge_fee_pool(
			RawOrigin::Root.into(),
			stable_currency_id,
			pool_size,
			swap_threshold
		));

		let exchange_rate = Pallet::<T>::token_exchange_rate(stable_currency_id).unwrap();

		frame_system::Pallet::<T>::assert_has_event(
			Event::ChargeFeePoolEnabled {
				sub_account,
				currency_id: stable_currency_id,
				exchange_rate,
				pool_size,
				swap_threshold,
			}
			.into(),
		);

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), stable_currency_id, call);

		frame_system::Pallet::<T>::assert_has_event(
			frame_system::Event::<T>::Remarked {
				sender: caller,
				hash: T::Hashing::hash(&vec![]),
			}
			.into(),
		);
	}

	#[benchmark]
	fn with_fee_aggregated_path() {
		frame_system::Pallet::<T>::set_block_number(1u32.into());
		let liquid_currency_id = <T as Config>::BenchmarkHelper::setup_liquid_currency_id().unwrap();

		let caller: T::AccountId = account("caller", 0, 0);
		let call = Box::new(frame_system::Call::remark_with_event { remark: vec![] }.into());
		// set_balance(STAKING, &caller, 100 * dollar(STAKING));
		// set_balance(NATIVE, &caller, 100 * dollar(NATIVE));

		<T as Config>::BenchmarkHelper::setup_enable_stable_asset();

		// Taiga(STAKING, LIQUID), Dex(LIQUID, NATIVE)
		let fee_aggregated_path = vec![
			AggregatedSwapPath::<CurrencyId>::Taiga(0, 0, 1),
			AggregatedSwapPath::<CurrencyId>::Dex(vec![liquid_currency_id, T::NativeCurrencyId::get()]),
		];

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), fee_aggregated_path, call);

		frame_system::Pallet::<T>::assert_has_event(
			frame_system::Event::<T>::Remarked {
				sender: caller,
				hash: T::Hashing::hash(&vec![]),
			}
			.into(),
		);
	}

	#[benchmark]
	fn on_finalize() {
		#[block]
		{
			Pallet::<T>::on_finalize(frame_system::Pallet::<T>::block_number());
		}
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
