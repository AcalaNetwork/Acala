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

extern crate alloc;

use super::*;
use crate::Pallet;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::{EventRecord, RawOrigin};
use primitives::TokenSymbol;

const NATIVE: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
const STABLECOIN: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
const LIQUID: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
const STAKING: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
const BNC: CurrencyId = CurrencyId::Token(TokenSymbol::BNC);
const VSKSM: CurrencyId = CurrencyId::Token(TokenSymbol::VSKSM);
const CURRENCY_LIST: [CurrencyId; 6] = [NATIVE, STABLECOIN, LIQUID, STAKING, BNC, VSKSM];

fn assert_last_event<T: Config>(generic_event: <T as frame_system::Config>::RuntimeEvent) {
	let events = frame_system::Pallet::<T>::events();
	let system_event: <T as frame_system::Config>::RuntimeEvent = generic_event.into();
	// compare to the last event record
	let EventRecord { event, .. } = &events[events.len() - 1];
	assert_eq!(event, &system_event);
}

fn dollar(_currency_id: CurrencyId) -> Balance {
	10u128.saturating_pow(12)
}

fn inject_liquidity<T: Config>(
	maker: T::AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
	deposit: bool,
) {
	// set balance
	assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
		currency_id_a,
		&maker,
		max_amount_a.saturated_into(),
	));
	assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
		currency_id_b,
		&maker,
		max_amount_b.saturated_into(),
	));

	assert_ok!(Pallet::<T>::enable_trading_pair(
		RawOrigin::Root.into(),
		currency_id_a,
		currency_id_b
	));

	assert_ok!(Pallet::<T>::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		deposit,
	));
}

#[benchmarks]
mod benchmarks {
	use super::*;

	// enable a Disabled trading pair
	#[benchmark]
	fn enable_trading_pair() {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, trading_pair.first(), trading_pair.second());

		assert_last_event::<T>(
			Event::<T>::EnableTradingPair {
				trading_pair: trading_pair,
			}
			.into(),
		);
	}

	// disable a Enabled trading pair
	#[benchmark]
	fn disable_trading_pair() {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Disabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::enable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		#[extrinsic_call]
		_(RawOrigin::Root, trading_pair.first(), trading_pair.second());

		assert_last_event::<T>(
			Event::<T>::DisableTradingPair {
				trading_pair: trading_pair,
			}
			.into(),
		);
	}

	// list a Provisioning trading pair
	#[benchmark]
	fn list_provisioning() {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			10u32.into(),
		);

		assert_last_event::<T>(
			Event::<T>::ListProvisioning {
				trading_pair: trading_pair,
			}
			.into(),
		);
	}

	// update parameters of a Provisioning trading pair
	#[benchmark]
	fn update_provisioning_parameters() {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			1000 * dollar(trading_pair.second()),
			100u32.into(),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			trading_pair.first(),
			trading_pair.second(),
			2 * dollar(trading_pair.first()),
			2 * dollar(trading_pair.second()),
			10 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
			200u32.into(),
		);
	}

	// end a Provisioning trading pair
	#[benchmark]
	fn end_provisioning() {
		let founder: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
			0u32.into(),
		));

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&founder,
			(100 * dollar(trading_pair.first())).saturated_into(),
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&founder,
			(100 * dollar(trading_pair.second())).saturated_into(),
		));

		// add enough provision
		assert_ok!(Pallet::<T>::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(founder), trading_pair.first(), trading_pair.second());

		assert_last_event::<T>(
			Event::<T>::ProvisioningToEnabled {
				trading_pair: trading_pair,
				pool_0: 100 * dollar(trading_pair.first()),
				pool_1: 100 * dollar(trading_pair.second()),
				share_amount: 200 * dollar(trading_pair.first()),
			}
			.into(),
		);
	}

	#[benchmark]
	fn add_provision() {
		let founder: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}
		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			1000 * dollar(trading_pair.second()),
			0u32.into(),
		));

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&founder,
			(10 * dollar(trading_pair.first())).saturated_into(),
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&founder,
			(10 * dollar(trading_pair.second())).saturated_into(),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(founder.clone()),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
		);

		assert_last_event::<T>(
			Event::<T>::AddProvision {
				who: founder,
				currency_0: trading_pair.first(),
				contribution_0: dollar(trading_pair.first()),
				currency_1: trading_pair.second(),
				contribution_1: dollar(trading_pair.second()),
			}
			.into(),
		);
	}

	#[benchmark]
	fn claim_dex_share() {
		let founder: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}
		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
			0u32.into(),
		));

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&founder,
			(100 * dollar(trading_pair.first())).saturated_into()
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&founder,
			(100 * dollar(trading_pair.second())).saturated_into()
		));

		assert_ok!(Pallet::<T>::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			20 * dollar(trading_pair.second())
		));
		assert_ok!(Pallet::<T>::end_provisioning(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(whitelisted_caller()),
			founder.clone(),
			trading_pair.first(),
			trading_pair.second(),
		);

		assert_eq!(
			T::Currency::free_balance(trading_pair.dex_share_currency_id(), &founder),
			2_000_000_000_000
		);
	}

	// add liquidity but don't staking lp
	#[benchmark]
	fn add_liquidity() {
		let first_maker: T::AccountId = account("first_maker", 0, 0);
		let second_maker: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		let amount_a = 100 * dollar(trading_pair.first());
		let amount_b = 10_000 * dollar(trading_pair.second());

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&second_maker,
			amount_a.saturated_into()
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&second_maker,
			amount_b.saturated_into()
		));

		// first maker inject liquidity
		inject_liquidity::<T>(
			first_maker.clone(),
			trading_pair.first(),
			trading_pair.second(),
			amount_a,
			amount_b,
			false,
		);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(second_maker),
			trading_pair.first(),
			trading_pair.second(),
			amount_a,
			amount_b,
			Default::default(),
			false,
		);
	}

	// worst: add liquidity and stake lp
	#[benchmark]
	fn add_liquidity_and_stake() {
		let first_maker: T::AccountId = account("first_maker", 0, 0);
		let second_maker: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		let amount_a = 100 * dollar(trading_pair.first());
		let amount_b = 10_000 * dollar(trading_pair.second());

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&second_maker,
			amount_a.saturated_into()
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&second_maker,
			amount_b.saturated_into()
		));

		// first maker inject liquidity
		inject_liquidity::<T>(
			first_maker.clone(),
			trading_pair.first(),
			trading_pair.second(),
			amount_a,
			amount_b,
			true,
		);

		#[extrinsic_call]
		add_liquidity(
			RawOrigin::Signed(second_maker),
			trading_pair.first(),
			trading_pair.second(),
			amount_a,
			amount_b,
			Default::default(),
			true,
		);
	}

	// remove liquidity by liquid lp share
	#[benchmark]
	fn remove_liquidity() {
		let maker: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();

		inject_liquidity::<T>(
			maker.clone(),
			trading_pair.first(),
			trading_pair.second(),
			100 * dollar(trading_pair.first()),
			10_000 * dollar(trading_pair.second()),
			false,
		);

		#[extrinsic_call]
		remove_liquidity(
			RawOrigin::Signed(maker),
			trading_pair.first(),
			trading_pair.second(),
			50 * dollar(trading_pair.first()),
			Default::default(),
			Default::default(),
			false,
		);
	}

	// remove liquidity by withdraw staking lp share
	#[benchmark]
	fn remove_liquidity_by_unstake() {
		let maker: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();

		inject_liquidity::<T>(
			maker.clone(),
			trading_pair.first(),
			trading_pair.second(),
			100 * dollar(trading_pair.first()),
			10_000 * dollar(trading_pair.second()),
			true,
		);

		#[extrinsic_call]
		remove_liquidity(
			RawOrigin::Signed(maker),
			trading_pair.first(),
			trading_pair.second(),
			50 * dollar(trading_pair.first()),
			Default::default(),
			Default::default(),
			true,
		);
	}

	#[benchmark]
	fn swap_with_exact_supply(u: Linear<2, { T::TradingPathLimit::get() }>) {
		let maker: T::AccountId = account("maker", 0, 0);
		let taker: T::AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1..u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity::<T>(
					maker.clone(),
					cur0,
					cur1,
					10_000 * dollar(cur0),
					10_000 * dollar(cur1),
					false,
				);
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity::<T>(
					maker.clone(),
					CURRENCY_LIST[i as usize - 1],
					CURRENCY_LIST[i as usize],
					10_000 * dollar(CURRENCY_LIST[i as usize - 1]),
					10_000 * dollar(CURRENCY_LIST[i as usize]),
					false,
				);
			}
		}

		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			path[0],
			&taker,
			(10_000 * dollar(path[0])).saturated_into()
		));

		#[extrinsic_call]
		swap_with_exact_supply(RawOrigin::Signed(taker.clone()), path.clone(), 100 * dollar(path[0]), 0);

		// would panic the benchmark anyways, must add new currencies to CURRENCY_LIST for benchmarking to
		// work
		assert!(T::TradingPathLimit::get() < CURRENCY_LIST.len() as u32);
	}

	#[benchmark]
	fn swap_with_exact_target(u: Linear<2, { T::TradingPathLimit::get() }>) {
		let maker: T::AccountId = account("maker", 0, 0);
		let taker: T::AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1..u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity::<T>(
					maker.clone(),
					cur0,
					cur1,
					10_000 * dollar(cur0),
					10_000 * dollar(cur1),
					false,
				);
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity::<T>(
					maker.clone(),
					CURRENCY_LIST[i as usize - 1],
					CURRENCY_LIST[i as usize],
					10_000 * dollar(CURRENCY_LIST[i as usize - 1]),
					10_000 * dollar(CURRENCY_LIST[i as usize]),
					false,
				);
			}
		}

		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			path[0],
			&taker,
			(10_000 * dollar(path[0])).saturated_into()
		));

		#[extrinsic_call]
		swap_with_exact_target(
			RawOrigin::Signed(taker.clone()),
			path.clone(),
			10 * dollar(path[path.len() - 1]),
			100 * dollar(path[0]),
		);

		// would panic the benchmark anyways, must add new currencies to CURRENCY_LIST for benchmarking to
		// work
		assert!(T::TradingPathLimit::get() < CURRENCY_LIST.len() as u32);
	}

	#[benchmark]
	fn refund_provision() {
		let founder: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}
		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
			0u32.into(),
		));

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&founder,
			(100 * dollar(trading_pair.first())).saturated_into(),
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&founder,
			(100 * dollar(trading_pair.second())).saturated_into(),
		));

		assert_ok!(Pallet::<T>::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second())
		));

		frame_system::Pallet::<T>::set_block_number(T::ExtendedProvisioningBlocks::get() + 1u32.into());
		assert_ok!(Pallet::<T>::abort_provisioning(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
		));

		#[extrinsic_call]
		_(
			RawOrigin::Signed(founder.clone()),
			founder.clone(),
			trading_pair.first(),
			trading_pair.second(),
		);
	}

	#[benchmark]
	fn abort_provisioning() {
		let founder: T::AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Pallet::<T>::trading_pair_statuses(trading_pair) {
			assert_ok!(Pallet::<T>::disable_trading_pair(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second()
			));
		}

		assert_ok!(Pallet::<T>::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
			0u32.into(),
		));

		// set balance
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.first(),
			&founder,
			(100 * dollar(trading_pair.first())).saturated_into()
		));
		assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
			trading_pair.second(),
			&founder,
			(100 * dollar(trading_pair.second())).saturated_into()
		));

		assert_ok!(Pallet::<T>::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
		));

		frame_system::Pallet::<T>::set_block_number(T::ExtendedProvisioningBlocks::get() + 1u32.into());

		#[extrinsic_call]
		_(
			RawOrigin::Signed(whitelisted_caller()),
			trading_pair.first(),
			trading_pair.second(),
		);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
