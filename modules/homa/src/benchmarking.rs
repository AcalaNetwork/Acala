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

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize() {
		#[block]
		{
			Pallet::<T>::on_initialize(1u32.into());
		}
	}

	#[benchmark]
	fn on_initialize_with_bump_era(n: Liner<1, 50>) {
		let minter: T::AccountId = account("minter", 0, 0);
		let sub_account_index = T::ActiveSubAccountsIndexList::get().first().unwrap().clone();
		let mint_amount = T::MintThreshold::get();
		let redeem_amount = T::RedeemThreshold::get();

		assert_ok!(T::Currency::deposit(
			T::StakingCurrencyId::get(),
			&minter,
			10 * mint_amount
		));

		for i in 0..n {
			let redeemer = account("redeemer", i, 0);
			assert_ok!(T::Currency::deposit(
				T::LiquidCurrencyId::get(),
				&redeemer,
				redeem_amount
			));
		}

		// need to process unlocking
		assert_ok!(Pallet::<T>::reset_ledgers(
			RawOrigin::Root.into(),
			vec![(
				sub_account_index,
				Some(mint_amount),
				Some(vec![UnlockChunk {
					value: redeem_amount,
					era: 10
				}])
			)]
		));
		assert_ok!(Pallet::<T>::reset_current_era(RawOrigin::Root.into(), 9));

		assert_ok!(Pallet::<T>::update_homa_params(
			RawOrigin::Root.into(),
			Some(10 * mint_amount),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(Rate::saturating_from_rational(20, 100)),
			None,
			None,
		));
		T::RelayChainBlockNumber::set_block_number(10u32.into());

		assert_ok!(Pallet::<T>::update_bump_era_params(
			RawOrigin::Root.into(),
			None,
			Some(1u32.into())
		));

		// need to process to bond
		assert_ok!(Pallet::<T>::mint(RawOrigin::Signed(minter).into(), mint_amount));

		// need to process redeem request
		for i in 0..n {
			let redeemer = account("redeemer", i, 0);

			assert_ok!(Pallet::<T>::request_redeem(
				RawOrigin::Signed(redeemer).into(),
				redeem_amount,
				false
			));
		}

		#[block]
		{
			Pallet::<T>::on_initialize(1u32.into());
		}
	}

	#[benchmark]
	fn mint() {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::MintThreshold::get();

		assert_ok!(Pallet::<T>::update_homa_params(
			RawOrigin::Root.into(),
			Some(amount * 10),
			Some(Rate::saturating_from_rational(1, 10000)),
			None,
			None,
			None,
		));

		assert_ok!(T::Currency::deposit(T::StakingCurrencyId::get(), &caller, 2 * amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount);
	}

	#[benchmark]
	fn request_redeem() {
		let caller: T::AccountId = account("caller", 0, 0);
		let amount = T::RedeemThreshold::get();

		assert_ok!(T::Currency::deposit(T::LiquidCurrencyId::get(), &caller, 2 * amount));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller.clone()), amount, true);
	}

	#[benchmark]
	fn fast_match_redeems(n: Liner<1, 50>) {
		let caller: T::AccountId = account("caller", 0, 0);
		let minter: T::AccountId = account("minter", 0, 0);
		let mint_amount = 1_000_000_000_000_000;

		assert_ok!(T::Currency::deposit(
			T::StakingCurrencyId::get(),
			&minter,
			2 * mint_amount
		));
		assert_ok!(Pallet::<T>::update_homa_params(
			RawOrigin::Root.into(),
			Some(mint_amount * 10),
			Some(Rate::saturating_from_rational(1, 10000)),
			None,
			None,
			None,
		));

		assert_ok!(Pallet::<T>::mint(RawOrigin::Signed(minter.clone()).into(), mint_amount));

		let mut redeem_request_list: Vec<T::AccountId> = vec![];
		let redeem_amount = 10_000_000_000_000;

		for i in 0..n {
			let redeemer = account("redeemer", i, 0);
			assert_ok!(T::Currency::deposit(
				T::LiquidCurrencyId::get(),
				&redeemer,
				2 * redeem_amount
			));

			assert_ok!(Pallet::<T>::request_redeem(
				RawOrigin::Signed(redeemer.clone()).into(),
				redeem_amount,
				true
			));
			redeem_request_list.push(redeemer);
		}

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), redeem_request_list);
	}

	#[benchmark]
	fn claim_redemption() {
		let caller: T::AccountId = account("caller", 0, 0);
		let redeemer: T::AccountId = account("redeemer", 0, 0);
		let redeption_amount = T::RedeemThreshold::get();

		Unbondings::<T>::insert(&redeemer, 1, redeption_amount);
		UnclaimedRedemption::<T>::put(redeption_amount);

		assert_ok!(T::Currency::deposit(
			T::StakingCurrencyId::get(),
			&Pallet::<T>::account_id(),
			redeption_amount
		));

		assert_ok!(Pallet::<T>::reset_current_era(RawOrigin::Root.into(), 1));

		#[extrinsic_call]
		_(RawOrigin::Signed(caller), redeemer);
	}

	#[benchmark]
	fn update_homa_params() {
		let amount = T::MintThreshold::get();

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			Some(amount),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(7),
		);
	}

	#[benchmark]
	fn update_bump_era_params() {
		T::RelayChainBlockNumber::set_block_number(10000u32.into());

		#[extrinsic_call]
		_(RawOrigin::Root, Some(3000u32.into()), Some(7200u32.into()));
	}

	#[benchmark]
	fn reset_ledgers(n: Liner<0, 10>) {
		let mut updates: Vec<(u16, Option<Balance>, Option<Vec<UnlockChunk>>)> = vec![];
		for i in 0..n {
			updates.push((
				i.try_into().unwrap(),
				Some(1),
				Some(vec![UnlockChunk { value: 1, era: 1 }]),
			))
		}

		#[extrinsic_call]
		_(RawOrigin::Root, updates);
	}

	#[benchmark]
	fn reset_current_era() {
		#[extrinsic_call]
		_(RawOrigin::Root, 1);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
