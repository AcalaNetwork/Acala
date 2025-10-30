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
	// assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
	// 	currency_id_a,
	// 	&maker,
	// 	max_amount_a.saturated_into(),
	// ));
	// assert_ok!(<T::Currency as MultiCurrencyExtended<_>>::update_balance(
	// 	currency_id_b,
	// 	&maker,
	// 	max_amount_b.saturated_into(),
	// ));

	// assert_ok!(module_dex::Pallet::<T>::enable_trading_pair(
	// 	RawOrigin::Root.into(),
	// 	currency_id_a,
	// 	currency_id_b
	// ));

	// assert_ok!(module_dex::Pallet::<T>::add_liquidity(
	// 	RawOrigin::Signed(maker.clone()).into(),
	// 	currency_id_a,
	// 	currency_id_b,
	// 	max_amount_a,
	// 	max_amount_b,
	// 	Default::default(),
	// 	deposit,
	// ));
}

// pub fn set_block_number_timestamp(block_number: u32, timestamp: u64) {
// 	let slot = timestamp / Aura::slot_duration();
// 	let digest = Digest {
// 		logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
// 	};
// 	System::initialize(&block_number, &Default::default(), &digest);
// 	Aura::on_initialize(block_number);
// 	Timestamp::set_timestamp(timestamp);
// }

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn on_initialize_with_update_average_prices(n: Linear<0, 3>, u: Linear<0, 3>) {
		let caller: T::AccountId = whitelisted_caller();
		let trading_pair_list = vec![
			TradingPair::from_currency_ids(NATIVE, STABLECOIN).unwrap(),
			TradingPair::from_currency_ids(NATIVE, STAKING).unwrap(),
			TradingPair::from_currency_ids(STAKING, STABLECOIN).unwrap(),
		];

		for i in 0..n {
			let trading_pair = trading_pair_list[i as usize];
			crate::mock::set_pool(
				&TradingPair::from_currency_ids(trading_pair.first(), trading_pair.second()).unwrap(),
				dollar(trading_pair.first()) * 100,
				dollar(trading_pair.second()) * 1000,
			);

			// inject_liquidity::<T>(
			// 	caller.clone(),
			// 	trading_pair.first(),
			// 	trading_pair.second(),
			// 	dollar(trading_pair.first()) * 100,
			// 	dollar(trading_pair.second()) * 1000,
			// 	false,
			// );
			assert_ok!(Pallet::<T>::enable_average_price(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second(),
				240000u32.into(),
			));
		}
		for j in 0..u.min(n) {
			let update_pair = trading_pair_list[j as usize];
			assert_ok!(Pallet::<T>::update_average_price_interval(
				RawOrigin::Root.into(),
				update_pair.first(),
				update_pair.second(),
				24000u32.into(),
			));
		}
		// set_block_number_timestamp(1, 24000);

		#[block]
		{
			Pallet::<T>::on_initialize(1u32.into());
		}
	}

	// 	use super::utils::{dollar, inject_liquidity, set_block_number_timestamp, NATIVE, STABLECOIN,
	// STAKING}; use crate::{AccountId, DexOracle, Runtime};
	// use frame_benchmarking::whitelisted_caller;
	// use frame_support::traits::OnInitialize;
	// use frame_system::RawOrigin;
	// use orml_benchmarking::runtime_benchmarks;
	// use primitives::TradingPair;
	// use sp_std::prelude::*;

	// 	on_initialize_with_update_average_prices {
	// 		let n in 0 .. 3;
	// 		let u in 0 .. 3;
	// 		let caller: AccountId = whitelisted_caller();
	// 		let trading_pair_list = vec![
	// 			TradingPair::from_currency_ids(NATIVE, STABLECOIN).unwrap(),
	// 			TradingPair::from_currency_ids(NATIVE, STAKING).unwrap(),
	// 			TradingPair::from_currency_ids(STAKING, STABLECOIN).unwrap(),
	// 		];

	// 		for i in 0 .. n {
	// 			let trading_pair = trading_pair_list[i as usize];
	// 			inject_liquidity(caller.clone(), trading_pair.first(), trading_pair.second(),
	// dollar(trading_pair.first()) * 100, dollar(trading_pair.second()) * 1000, false)?;
	// 			DexOracle::enable_average_price(RawOrigin::Root.into(), trading_pair.first(),
	// trading_pair.second(), 240000)?; 		}
	// 		for j in 0 .. u.min(n) {
	// 			let update_pair = trading_pair_list[j as usize];
	// 			DexOracle::update_average_price_interval(RawOrigin::Root.into(), update_pair.first(),
	// update_pair.second(), 24000)?; 		}
	// 	}: {
	// 		set_block_number_timestamp(1, 24000);
	// 		DexOracle::on_initialize(1)
	// 	}

	// 	enable_average_price {
	// 		let caller: AccountId = whitelisted_caller();
	// 		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE), dollar(STABLECOIN), false)?;
	// 	}: _(RawOrigin::Root, NATIVE, STABLECOIN, 24000)

	// 	disable_average_price {
	// 		let caller: AccountId = whitelisted_caller();
	// 		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE) * 100, dollar(STABLECOIN) * 1000,
	// false)?; 		DexOracle::enable_average_price(RawOrigin::Root.into(), NATIVE, STABLECOIN, 24000)?;
	// 	}: _(RawOrigin::Root, NATIVE, STABLECOIN)

	// 	update_average_price_interval {
	// 		let caller: AccountId = whitelisted_caller();
	// 		inject_liquidity(caller, NATIVE, STABLECOIN, dollar(NATIVE) * 100, dollar(STABLECOIN) * 1000,
	// false)?; 		DexOracle::enable_average_price(RawOrigin::Root.into(), NATIVE, STABLECOIN, 24000)?;
	// 	}: _(RawOrigin::Root, NATIVE, STABLECOIN, 240000)
	// }

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
