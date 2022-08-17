// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

//! Unit tests for the Aggregated DEX module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Call as MockCall, Event, Origin, System, *};
use nutsfinance_stable_asset::traits::StableAsset as StableAssetT;
use parking_lot::RwLock;
use sp_core::offchain::{
	testing, testing::PoolState, DbExternalities, OffchainDbExt, OffchainWorkerExt, StorageKind, TransactionPoolExt,
};
use sp_io::offchain;
use sp_runtime::traits::BadOrigin;
use std::sync::Arc;

fn run_to_block_offchain(n: u64) {
	while System::block_number() < n {
		System::set_block_number(System::block_number() + 1);
		AggregatedDex::offchain_worker(System::block_number());
		// this unlocks the concurrency storage lock so offchain_worker will fire next block
		offchain::sleep_until(offchain::timestamp().add(Duration::from_millis(LOCK_DURATION + 200)));
	}
}

fn set_dex_swap_joint_list(joints: Vec<Vec<CurrencyId>>) {
	DexSwapJointList::set(joints);
}

fn inject_liquidity(
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	// set balance
	Tokens::deposit(currency_id_a, &BOB, max_amount_a)?;
	Tokens::deposit(currency_id_b, &BOB, max_amount_b)?;

	let _ = Dex::enable_trading_pair(Origin::signed(BOB.clone()), currency_id_a, currency_id_b);
	Dex::add_liquidity(
		Origin::signed(BOB),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;

	Ok(())
}

fn initial_taiga_dot_ldot_pool() -> DispatchResult {
	StableAssetWrapper::create_pool(
		STABLE_ASSET,
		vec![DOT, LDOT],
		vec![1u128, 1u128],
		0,
		0,
		0,
		3000u128,
		BOB,
		BOB,
		10_000_000_000u128,
	)?;

	let amount = 100_000_000_000u128;
	Tokens::deposit(DOT, &BOB, amount)?;
	Tokens::deposit(LDOT, &BOB, amount * 10)?;

	// The DOT and LDOT convert rate in `mock::ConvertBalanceHoma` is 1/10.
	StableAssetWrapper::mint(&BOB, 0, vec![amount, amount * 10], 0)?;
	assert_eq!(
		StableAssetWrapper::pool(0).map(|p| p.balances).unwrap(),
		vec![amount, amount]
	);

	Ok(())
}

#[test]
fn rebase_stable_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(initial_taiga_dot_ldot_pool());

		assert_eq!(
			StableAssetWrapper::get_best_route(DOT, LDOT, 100_000_000u128),
			Some((0, 0, 1, 999_983_600u128))
		);
		assert_eq!(
			StableAssetWrapper::get_best_route(LDOT, DOT, 1_000_000_000u128),
			Some((0, 1, 0, 99_998_360u128))
		);

		assert_eq!(
			StableAssetWrapper::get_swap_input_amount(0, 0, 1, 999_983_600u128).map(|r| (r.dx, r.dy)),
			Some((100_000_098u128, 999_983_600u128))
		);
		assert_eq!(
			StableAssetWrapper::get_swap_output_amount(0, 0, 1, 100_000_000u128).map(|r| (r.dx, r.dy)),
			Some((100_000_000u128, 999_983_600u128))
		);

		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);
		assert_eq!(
			StableAssetWrapper::swap(&ALICE, 0, 0, 1, 100_000_000u128, 0, 2),
			Ok((100_000_000u128, 999_983_600u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_900_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 999_983_600u128);
	});
}

#[test]
fn dex_swap_get_swap_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, AUSD, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);

		assert_ok!(inject_liquidity(
			DOT,
			AUSD,
			100_000_000_000u128,
			200_000_000_000_000u128
		));
		assert_ok!(inject_liquidity(
			LDOT,
			AUSD,
			1_000_000_000_000u128,
			200_000_000_000_000u128
		));

		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, AUSD, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 1_980_198_019_801u128))
		);

		set_dex_swap_joint_list(vec![vec![AUSD]]);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_803_921_568u128))
		);

		assert_ok!(inject_liquidity(DOT, LDOT, 100_000_000_000u128, 1_000_000_000_000u128));
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_900_990_099u128))
		);
	});
}

#[test]
fn dex_swap_swap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(inject_liquidity(
			DOT,
			AUSD,
			100_000_000_000u128,
			200_000_000_000_000u128
		));
		assert_ok!(inject_liquidity(
			LDOT,
			AUSD,
			1_000_000_000_000u128,
			200_000_000_000_000u128
		));

		assert_noop!(
			DexSwap::<Runtime>::swap(&ALICE, DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);

		set_dex_swap_joint_list(vec![vec![AUSD]]);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);

		assert_noop!(
			DexSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);
		assert_ok!(DexSwap::<Runtime>::swap(
			&ALICE,
			DOT,
			LDOT,
			SwapLimit::ExactSupply(1_000_000_000u128, 5_000_000_000u128)
		));
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 9_803_921_568u128);

		assert_noop!(
			DexSwap::<Runtime>::swap(
				&ALICE,
				LDOT,
				DOT,
				SwapLimit::ExactTarget(9_803_921_568u128, 1_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);
		assert_ok!(DexSwap::<Runtime>::swap(
			&ALICE,
			LDOT,
			DOT,
			SwapLimit::ExactTarget(9_803_921_568u128, 500_000_000u128)
		));
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_500_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 4_950_495_048u128);

		assert_noop!(
			DexSwap::<Runtime>::swap_by_path(
				&ALICE,
				&vec![DOT, LDOT],
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_ok!(DexSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![DOT, AUSD, LDOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_ok!(DexSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![LDOT, AUSD, DOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_noop!(
			DexSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Dex(vec![DOT, AUSD, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Error::<Runtime>::CannotSwap
		);
	});
}

#[test]
fn taiga_swap_get_swap_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactTarget(u128::MAX, 10_000_000_000u128)),
			None
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, AUSD, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 9_998_360_751u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				AUSD,
				SwapLimit::ExactTarget(10_000_000_000u128, 10_000_000_000u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 9_998_360_750u128)
			),
			Some((1_000_000_098u128, 9_998_361_730u128))
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(1_000_000_097u128, 9_998_360_750u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				LDOT,
				DOT,
				SwapLimit::ExactTarget(100_000_000_000u128, 1_000_000_000u128)
			),
			Some((10_001_640_760u128, 1_000_000_098u128))
		);
	});
}

#[test]
fn taiga_swap_swap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			TaigaSwap::<Runtime>::swap(&ALICE, DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);
		assert_noop!(
			TaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(10_000_000_000u128, 9_998_360_750u128)
			),
			Error::<Runtime>::CannotSwap
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);

		assert_eq!(
			TaigaSwap::<Runtime>::swap(&ALICE, DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Ok((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 9_998_360_750u128);

		assert_noop!(
			TaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			nutsfinance_stable_asset::Error::<Runtime>::SwapUnderMin
		);

		assert_eq!(
			TaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Ok((1_000_492_274u128, 10_000_000_980u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 97_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 19_998_361_730u128);

		assert_noop!(
			TaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(1_000_000_000u128, 10_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);

		assert_noop!(
			TaigaSwap::<Runtime>::swap_by_path(&ALICE, &vec![DOT, LDOT], SwapLimit::ExactTarget(1_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);
		assert_noop!(
			TaigaSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Dex(vec![DOT, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Error::<Runtime>::CannotSwap
		);
	});
}

#[test]
fn either_dex_or_taiga_swap_get_swap_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			None
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			None
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((1_000_164_076u128, 10_000_000_980u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((1_000_164_076u128, 10_000_000_980u128))
		);

		assert_ok!(inject_liquidity(DOT, LDOT, 1_000_000_000u128, 30_000_000_000u128));
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 15_000_000_000u128))
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 15_000_000_000u128))
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((500_000_001u128, 10_000_000_000u128))
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((1_000_164_076u128, 10_000_000_980u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((500_000_001u128, 10_000_000_000u128))
		);

		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(10_000_000_000u128, 0)),
			Some((10_000_000_000u128, 27_272_727_272u128))
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(10_000_000_000u128, 0)),
			Some((10_000_000_000u128, 99_834_740_530u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(10_000_000_000u128, 0)),
			Some((10_000_000_000u128, 99_834_740_530u128))
		);
		assert_eq!(
			DexSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(10_000_000_000u128, 30_000_000_000u128)
			),
			None
		);
		assert_eq!(
			TaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(10_000_000_000u128, 30_000_000_000u128)
			),
			Some((3_001_477_523u128, 30_000_000_980u128))
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::get_swap_amount(
				DOT,
				LDOT,
				SwapLimit::ExactTarget(10_000_000_000u128, 30_000_000_000u128)
			),
			Some((3_001_477_523u128, 30_000_000_980u128))
		);
	});
}

#[test]
fn either_dex_or_taiga_swap_swap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap(&ALICE, DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);
		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);

		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 9_000_000_000u128)
			),
			Ok((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 9_998_360_750u128);

		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(1_000_000_000u128, 9_998_360_750u128)
			),
			Error::<Runtime>::CannotSwap
		);
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Ok((1_000_492_274u128, 10_000_000_980u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 97_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 19_998_361_730u128);

		assert_ok!(inject_liquidity(DOT, LDOT, 100_000_000_000u128, 2_000_000_000_000u128));
		assert_eq!(
			EitherDexOrTaigaSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			Ok((1_000_000_000u128, 19_801_980_198u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 96_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 39_800_341_928u128);

		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap_by_path(
				&ALICE,
				&vec![DOT, AUSD],
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_ok!(EitherDexOrTaigaSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![DOT, LDOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_ok!(EitherDexOrTaigaSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![LDOT, DOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_noop!(
			EitherDexOrTaigaSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Dex(vec![DOT, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Error::<Runtime>::CannotSwap
		);
	});
}

#[test]
fn check_swap_paths_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![]),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Dex(vec![])]),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Dex(vec![LDOT])]),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Dex(vec![LDOT, LDOT])]),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_ok!(AggregatedDex::check_swap_paths(&vec![SwapPath::Dex(vec![LDOT, AUSD])]));

		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1)]),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 0)]),
			Error::<Runtime>::InvalidSwapPath
		);

		assert_ok!(initial_taiga_dot_ldot_pool());

		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 2, 0)]),
			Error::<Runtime>::InvalidTokenIndex
		);
		assert_noop!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![AUSD, LDOT])]),
			Error::<Runtime>::InvalidSwapPath
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1)]),
			Ok((DOT, LDOT))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])]),
			Ok((DOT, AUSD))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)]),
			Ok((AUSD, DOT))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, DOT]),]),
			Ok((DOT, DOT))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 1, 0), SwapPath::Dex(vec![DOT, LDOT]),]),
			Ok((LDOT, LDOT))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 1, 0), SwapPath::Taiga(0, 0, 1),]),
			Ok((LDOT, LDOT))
		);
		assert_eq!(
			AggregatedDex::check_swap_paths(&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Taiga(0, 1, 0),]),
			Ok((DOT, DOT))
		);
	});
}

#[test]
fn get_aggregated_swap_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			None
		);

		assert_ok!(inject_liquidity(
			LDOT,
			AUSD,
			100_000_000_000u128,
			20_000_000_000_000u128
		));

		// dex
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 4_999_750u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 4_999_751u128)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT])],
				SwapLimit::ExactTarget(1_000_000_000u128, 4_999_750u128)
			),
			Some((999_999_998u128, 4_999_750u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT])],
				SwapLimit::ExactTarget(999_999_997u128, 4_999_750u128)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			None
		);

		// taiga
		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Some((1_000_164_076u128, 10_000_000_980u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactTarget(1_000_000_000u128, 10_000_000_000u128)
			),
			None
		);

		// taiga + dex
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 1_817_910_863_730u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 1_817_910_863_731u128)
			),
			None
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactTarget(2_000_000_000u128, 1_817_910_863_730u128)
			),
			Some((1_000_000_098u128, 1_817_911_025_719u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactTarget(1_000_000_097u128, 1_817_910_863_730u128)
			),
			None
		);

		// dex + taiga
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)],
				SwapLimit::ExactSupply(1_817_910_863_730u128, 0)
			),
			Some((1_817_910_863_730u128, 833_105_687u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)],
				SwapLimit::ExactTarget(3_000_000_000_000u128, 1_000_000_000u128)
			),
			Some((2_222_627_355_534u128, 1_000_000_098u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)],
				SwapLimit::ExactTarget(2_222_627_355_533u128, 1_000_000_000u128)
			),
			None
		);

		// more complicate aggregated dex test.
		// We've already Taiga(DOT,LDOT), and Dex(LDOT, AUSD).
		assert_ok!(inject_liquidity(DOT, AUSD, 100_000_000_000u128, 20_000_000_000_000u128));
		assert_ok!(inject_liquidity(DOT, LDOT, 100_000_000_000u128, 100_000_000_000u128));

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, DOT]),],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 9_089_554_318u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, DOT]),],
				SwapLimit::ExactTarget(1_000_000_000u128, 1_000_000_000u128)
			),
			Some((101_011_873u128, 1_000_000_960u128))
		);

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 1, 0), SwapPath::Dex(vec![DOT, LDOT]),],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 99_898_463u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 1, 0), SwapPath::Dex(vec![DOT, LDOT]),],
				SwapLimit::ExactTarget(2_000_000_000u128, 100_000_000u128)
			),
			Some((1_001_018_430u128, 100_000_098u128))
		);

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
				],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 8_332_194_934u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
				],
				SwapLimit::ExactTarget(1_000_000_000u128, 1_000_000_000u128)
			),
			Some((102_042_622u128, 1_000_000_938u128))
		);

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
				],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 970_873_785u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
				],
				SwapLimit::ExactTarget(1_000_000_000u128, 100_000_000u128)
			),
			Some((100_300_904u128, 100_000_000u128))
		);

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
				],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 9_994_493_008u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
				],
				SwapLimit::ExactTarget(1_000_000_000u128, 100_000_000u128)
			),
			Some((10_019_806u128, 100_195_498u128))
		);

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
				],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 943_396_225u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Dex(vec![DOT, LDOT]),
				],
				SwapLimit::ExactTarget(2_000_000_000u128, 1_000_000_000u128)
			),
			Some((1_063_829_789u128, 1_000_000_000u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
				],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Some((1_000_000_000u128, 99_397_727_227u128))
		);
		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
					SwapPath::Dex(vec![AUSD, DOT]),
					SwapPath::Taiga(0, 0, 1),
					SwapPath::Dex(vec![LDOT, AUSD]),
				],
				SwapLimit::ExactTarget(2_000_000_000u128, 1_000_000_000u128)
			),
			Some((10_022_406u128, 1_002_155_781u128))
		);
	});
}

#[test]
fn do_aggregated_swap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Error::<Runtime>::InvalidPoolId
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);

		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);
		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Ok((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 9_998_360_750u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 0);

		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1)],
				SwapLimit::ExactTarget(2_000_000_000u128, 10_000_000_000u128)
			),
			Ok((1_000_492_274u128, 10_000_000_980u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 97_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 19_998_361_730u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 0);

		assert_ok!(inject_liquidity(
			LDOT,
			AUSD,
			100_000_000_000u128,
			20_000_000_000_000u128
		));
		assert_noop!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 200_000_000_000u128)
			),
			Error::<Runtime>::CannotSwap
		);

		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Ok((1_000_000_000u128, 198_019_801_980u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 97_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 18_998_361_730u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 198_019_801_980u128);

		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactTarget(1_000_000_000u128, 10_000_000_000u128)
			),
			Ok((51_030_771u128, 10_000_000_090u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 97_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 18_947_330_959u128);
		// actually swap by ExactSupply, actual target amount may be slightly more than exact target amount
		// of limit
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 208_019_802_070u128);

		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Ok((1_000_000_000u128, 1_780_911_406_971u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 96_999_507_726u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 18_947_330_959u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 1_988_931_209_041u128);

		assert_eq!(
			AggregatedDex::do_aggregated_swap(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactTarget(1_000_000_000_000u128, 1_000_000_000_000u128)
			),
			Ok((653_482_016u128, 1_000_000_140_971u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 96_346_025_710u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 18_947_330_959u128);
		// actually swap by ExactSupply, actual target amount may be slightly more than exact target amount
		// of limit
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 2_988_931_350_012u128);
	});
}

#[test]
fn update_aggregated_swap_paths_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AggregatedDex::update_aggregated_swap_paths(Origin::signed(ALICE), vec![]),
			BadOrigin
		);

		assert_noop!(
			AggregatedDex::update_aggregated_swap_paths(
				Origin::signed(BOB),
				vec![
					(
						(DOT, AUSD),
						Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
					),
					(
						(AUSD, DOT),
						Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
					)
				]
			),
			Error::<Runtime>::InvalidPoolId
		);

		assert_ok!(initial_taiga_dot_ldot_pool());

		assert_noop!(
			AggregatedDex::update_aggregated_swap_paths(
				Origin::signed(BOB),
				vec![
					(
						(DOT, AUSD),
						Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
					),
					(
						(AUSD, DOT),
						Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
					)
				]
			),
			Error::<Runtime>::InvalidSwapPath
		);

		assert_eq!(AggregatedDex::aggregated_swap_paths((DOT, AUSD)), None);
		assert_eq!(AggregatedDex::aggregated_swap_paths((AUSD, DOT)), None);
		assert_ok!(AggregatedDex::update_aggregated_swap_paths(
			Origin::signed(BOB),
			vec![
				(
					(DOT, AUSD),
					Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
				),
				(
					(AUSD, DOT),
					Some(vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)])
				)
			]
		));
		assert_eq!(
			AggregatedDex::aggregated_swap_paths((DOT, AUSD)).unwrap(),
			vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])]
		);
		assert_eq!(
			AggregatedDex::aggregated_swap_paths((AUSD, DOT)).unwrap(),
			vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)]
		);

		assert_noop!(
			AggregatedDex::update_aggregated_swap_paths(
				Origin::signed(BOB),
				vec![(
					(DOT, AUSD),
					Some(vec![
						SwapPath::Taiga(0, 0, 1),
						SwapPath::Taiga(0, 1, 0),
						SwapPath::Taiga(0, 0, 1),
						SwapPath::Dex(vec![LDOT, AUSD])
					])
				),]
			),
			Error::<Runtime>::InvalidSwapPath
		);

		assert_ok!(AggregatedDex::update_aggregated_swap_paths(
			Origin::signed(BOB),
			vec![((DOT, AUSD), None), ((AUSD, DOT), None)]
		));
		assert_eq!(AggregatedDex::aggregated_swap_paths((DOT, AUSD)), None);
		assert_eq!(AggregatedDex::aggregated_swap_paths((AUSD, DOT)), None);
	});
}

#[test]
fn aggregated_swap_get_swap_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			None
		);

		assert_ok!(inject_liquidity(DOT, LDOT, 1_000_000_000u128, 30_000_000_000u128));
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 15_000_000_000u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			Some((3_000_000_000u128, 22_500_000_000u128))
		);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Some((1_000_000_000u128, 15_000_000_000u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, LDOT, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			Some((3_000_000_000u128, 29_985_240_300u128))
		);

		assert_ok!(inject_liquidity(LDOT, AUSD, 30_000_000_000u128, 60_000_000_000u128));

		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, AUSD, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			None
		);

		assert_ok!(AggregatedDex::update_aggregated_swap_paths(
			Origin::signed(BOB),
			vec![(
				(DOT, AUSD),
				Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
			),]
		));
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(DOT, AUSD, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			Some((3_000_000_000u128, 29_992_618_334u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(AUSD, DOT, SwapLimit::ExactSupply(30_000_000_000u128, 0)),
			None
		);

		assert_ok!(AggregatedDex::update_aggregated_swap_paths(
			Origin::signed(BOB),
			vec![(
				(AUSD, DOT),
				Some(vec![SwapPath::Dex(vec![AUSD, LDOT]), SwapPath::Taiga(0, 1, 0)])
			),]
		));
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(AUSD, LDOT, SwapLimit::ExactSupply(30_000_000_000u128, 0)),
			Some((30_000_000_000u128, 10_000_000_000u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(LDOT, DOT, SwapLimit::ExactSupply(10_000_000_000u128, 0)),
			Some((10_000_000_000u128, 999_836_075u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(AUSD, DOT, SwapLimit::ExactSupply(30_000_000_000u128, 0)),
			Some((30_000_000_000u128, 999_836_075u128))
		);

		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(
				LDOT,
				DOT,
				SwapLimit::ExactTarget(20_000_000_000u128, 1_000_000_000u128)
			),
			Some((10_001_640_760u128, 1_000_000_098u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(
				AUSD,
				LDOT,
				SwapLimit::ExactTarget(u128::MAX, 10_000_000_000u128)
			),
			Some((30_000_000_001u128, 10_000_000_000u128))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::get_swap_amount(AUSD, DOT, SwapLimit::ExactTarget(u128::MAX, 1_000_000_000u128)),
			Some((30_007_384_026u128, 1_000_000_098u128))
		);
	});
}

#[test]
fn aggregated_swap_swap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AggregatedSwap::<Runtime>::swap(&ALICE, DOT, LDOT, SwapLimit::ExactSupply(1_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);

		assert_ok!(inject_liquidity(DOT, LDOT, 1_000_000_000u128, 30_000_000_000u128));
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 100_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 0);

		assert_noop!(
			AggregatedSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 15_000_000_001u128)
			),
			Error::<Runtime>::CannotSwap
		);
		assert_ok!(AggregatedSwap::<Runtime>::swap(
			&ALICE,
			DOT,
			LDOT,
			SwapLimit::ExactSupply(1_000_000_000u128, 15_000_000_000u128)
		));
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 99_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 15_000_000_000u128);

		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_eq!(
			AggregatedSwap::<Runtime>::swap(
				&ALICE,
				DOT,
				LDOT,
				SwapLimit::ExactSupply(1_000_000_000u128, 9_000_000_000u128)
			),
			Ok((1_000_000_000u128, 9_998_360_750u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 98_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 24_998_360_750u128);

		assert_ok!(inject_liquidity(LDOT, AUSD, 30_000_000_000u128, 60_000_000_000u128));

		assert_noop!(
			AggregatedSwap::<Runtime>::swap(&ALICE, DOT, AUSD, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			Error::<Runtime>::CannotSwap
		);

		assert_ok!(AggregatedDex::update_aggregated_swap_paths(
			Origin::signed(BOB),
			vec![(
				(DOT, AUSD),
				Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])])
			),]
		));

		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 0);
		assert_eq!(
			AggregatedSwap::<Runtime>::swap(&ALICE, DOT, AUSD, SwapLimit::ExactSupply(3_000_000_000u128, 0)),
			Ok((3_000_000_000u128, 29_987_688_109u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 95_000_000_000u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 24_998_360_750u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 29_987_688_109u128);

		assert_eq!(
			AggregatedSwap::<Runtime>::swap(&ALICE, DOT, AUSD, SwapLimit::ExactTarget(u128::MAX, 10_000_000_000u128)),
			Ok((3_002_366_414u128, 10_000_000_216u128))
		);
		assert_eq!(Tokens::free_balance(DOT, &ALICE), 91_997_633_586u128);
		assert_eq!(Tokens::free_balance(LDOT, &ALICE), 24_998_360_750u128);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 39_987_688_325u128);

		assert_noop!(
			AggregatedSwap::<Runtime>::swap_by_path(
				&ALICE,
				&vec![DOT, AUSD],
				SwapLimit::ExactSupply(1_000_000_000u128, 10_000_000_000u128)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_ok!(AggregatedSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![DOT, LDOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_ok!(AggregatedSwap::<Runtime>::swap_by_path(
			&ALICE,
			&vec![LDOT, DOT],
			SwapLimit::ExactSupply(1_000_000_000u128, 0)
		));
		assert_noop!(
			AggregatedSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Dex(vec![DOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			module_dex::Error::<Runtime>::MustBeEnabled
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Dex(vec![DOT, LDOT])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Ok((1000000000, 2951219511))
		);
		assert_eq!(
			AggregatedSwap::<Runtime>::swap_by_aggregated_path(
				&ALICE,
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, AUSD])],
				SwapLimit::ExactSupply(1_000_000_000u128, 0)
			),
			Ok((1000000000, 1997865702))
		);
	});
}

fn set_rebalance_info_works(token: CurrencyId, supply_amount: Balance, minimum_amount: Balance) {
	// set swap supply and threshold
	assert_ok!(AggregatedDex::set_rebalance_swap_info(
		Origin::signed(BOB),
		token,
		supply_amount,
		minimum_amount,
	));
	System::assert_last_event(Event::AggregatedDex(crate::Event::SetupRebalanceSwapInfo {
		currency_id: token,
		supply_amount: supply_amount,
		threshold: minimum_amount,
	}));
	let supply_threshold = RebalanceSupplyThreshold::<Runtime>::get(token).unwrap();
	assert_eq!(supply_threshold, (supply_amount, minimum_amount));
}

fn inject_liquidity_default_pairs() {
	assert_ok!(inject_liquidity(AUSD, DOT, 1_000_000u128, 2_000_000u128));
	assert_ok!(inject_liquidity(DOT, BTC, 1_000_000u128, 2_000_000u128));
	assert_ok!(inject_liquidity(AUSD, BTC, 1_000_000u128, 2_000_000u128));
}

#[test]
fn offchain_worker_max_iteration_works() {
	let (mut offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		inject_liquidity_default_pairs();

		trigger_unsigned_rebalance_dex_swap(2, pool_state.clone(), vec![], vec![], None);

		let to_be_continue = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);
		let start_key = to_be_continue.get::<Vec<u8>>().unwrap_or_default();
		assert_eq!(start_key, None);

		// sets max iterations value to 1
		offchain.local_storage_set(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS, &1u32.encode());
		trigger_unsigned_rebalance_dex_swap(3, pool_state.clone(), vec![], vec![], None);

		let to_be_continue = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);
		let start_key = to_be_continue.get::<Vec<u8>>().unwrap_or_default().unwrap();
		let to_be_continue_currency = CurrencyId::decode(&mut &*start_key).unwrap();
		assert_eq!(to_be_continue_currency, AUSD);
	});
}

#[test]
fn offchain_worker_unsigned_rebalance_dex_swap() {
	let (offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		inject_liquidity_default_pairs();

		set_rebalance_info_works(AUSD, 1000, 1960);

		assert_ok!(Tokens::deposit(
			AUSD,
			&Pallet::<Runtime>::treasury_account(),
			1_000_000u128
		));

		// offchain worker execution trigger dex swap: AUSD->DOT-BTC->AUSD
		trigger_unsigned_rebalance_dex_swap(
			2,
			pool_state.clone(),
			vec![1000, 1998, 3988],
			vec![3988, 1990],
			Some(1990),
		);
		// treasury account use 1000 to swap 1990, that's gain 990.
		assert_eq!(
			Tokens::free_balance(AUSD, &Pallet::<Runtime>::treasury_account()),
			1000990
		);

		// treasury account use 1000 to swap 1970, that's gain 970.
		trigger_unsigned_rebalance_dex_swap(
			3,
			pool_state.clone(),
			vec![1000, 1994, 3964],
			vec![3964, 1970],
			Some(1970),
		);
		assert_eq!(
			Tokens::free_balance(AUSD, &Pallet::<Runtime>::treasury_account()),
			1001960
		);

		trigger_unsigned_rebalance_dex_swap(4, pool_state.clone(), vec![], vec![], None);
		assert_eq!(
			Tokens::free_balance(AUSD, &Pallet::<Runtime>::treasury_account()),
			1001960
		);

		// AUSD-DOT: AUSD+(1000+1000), DOT-(1998+1994)
		assert_eq!(Dex::get_liquidity_pool(AUSD, DOT), (1002000, 1996008));
		// DOT-BTC: DOT+(1998+1994), BTC-(3988+3964)
		assert_eq!(Dex::get_liquidity_pool(DOT, BTC), (1003992, 1992048));
		// BTC-AUSD: BTC+(3988+3964), AUSD-(1990+1970)
		assert_eq!(Dex::get_liquidity_pool(AUSD, BTC), (996040, 2007952));
	});
}

#[test]
fn offchain_worker_unsigned_rebalance_aggregated_dex_swap() {
	let (offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);

		// Taiga(DOT, LDOT) + Dex(LDOT, DOT)
		assert_ok!(initial_taiga_dot_ldot_pool());
		assert_ok!(inject_liquidity(DOT, LDOT, 11_000_000_000u128, 100_000_000_000u128));

		assert_eq!(
			AggregatedDex::get_aggregated_swap_amount(
				&vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, DOT])],
				SwapLimit::ExactSupply(1_000_000u128, 0)
			),
			Some((1_000_000u128, 1_099_888u128))
		);

		set_rebalance_info_works(DOT, 1_000_000, 1_050_000);

		let swap_path = vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LDOT, DOT])];
		assert_ok!(AggregatedDex::update_rebalance_swap_paths(
			Origin::signed(BOB),
			vec![(DOT, Some(swap_path.clone()))]
		));
		let swap_paths = RebalanceSwapPaths::<Runtime>::get(DOT).unwrap();
		assert_eq!(swap_paths.into_inner(), swap_path);

		assert_ok!(Tokens::deposit(
			DOT,
			&Pallet::<Runtime>::treasury_account(),
			10_000_000_000u128
		));

		System::reset_events();
		run_to_block_offchain(2);
		assert_unsigned_call_executed(pool_state);

		System::assert_last_event(Event::AggregatedDex(crate::Event::RebalanceTrading {
			currency_id: DOT,
			supply_amount: 1000000,
			target_amount: 1099888,
			swap_path,
		}));
		assert!(System::events().iter().any(|r| {
			matches!(
				r.event,
				crate::mock::Event::Dex(module_dex::Event::Swap { .. })
					| crate::mock::Event::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped { .. })
			)
		}));
	});
}

fn assert_unsigned_call_executed(pool_state: Arc<RwLock<PoolState>>) {
	let tx = pool_state.write().transactions.pop().unwrap();
	let tx = Extrinsic::decode(&mut &*tx).unwrap();

	if let MockCall::AggregatedDex(crate::Call::force_rebalance_swap { currency_id, swap_path }) = tx.call {
		assert_ok!(AggregatedDex::force_rebalance_swap(
			Origin::none(),
			currency_id,
			swap_path
		));
	}
	assert!(pool_state.write().transactions.pop().is_none());
}

fn trigger_unsigned_rebalance_dex_swap(
	n: u64,
	pool_state: Arc<RwLock<PoolState>>,
	dex_lp1: Vec<Balance>,
	dex_lp2: Vec<Balance>,
	actual_target_amount: Option<u128>,
) {
	System::reset_events();
	run_to_block_offchain(n);
	assert_unsigned_call_executed(pool_state);

	let swap_path = vec![
		AggregatedSwapPath::Dex(vec![AUSD, DOT, BTC]),
		AggregatedSwapPath::Dex(vec![BTC, AUSD]),
	];

	// if target amount is less than threshold, then rebalance swap not triggered.
	if let Some(target_amount) = actual_target_amount {
		System::assert_has_event(crate::mock::Event::Dex(module_dex::Event::Swap {
			trader: Pallet::<Runtime>::treasury_account(),
			path: vec![AUSD, DOT, BTC],
			liquidity_changes: dex_lp1,
		}));
		System::assert_has_event(crate::mock::Event::Dex(module_dex::Event::Swap {
			trader: Pallet::<Runtime>::treasury_account(),
			path: vec![BTC, AUSD],
			liquidity_changes: dex_lp2,
		}));
		System::assert_last_event(Event::AggregatedDex(crate::Event::RebalanceTrading {
			currency_id: AUSD,
			supply_amount: 1000,
			target_amount,
			swap_path,
		}));
	} else {
		assert!(System::events()
			.iter()
			.all(|r| { !matches!(r.event, Event::AggregatedDex(crate::Event::RebalanceTrading { .. })) }));
	}
}
