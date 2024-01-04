// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use super::utils::{dollar, inject_liquidity, LIQUID, NATIVE, STABLECOIN, STAKING};
use crate::{AccountId, Currencies, CurrencyId, Dex, ExtendedProvisioningBlocks, Runtime, RuntimeEvent, System};
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use module_dex::TradingPairStatus;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::TradingPair;
use runtime_common::{BNC, VSKSM};
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;

const SEED: u32 = 0;

const CURRENCY_LIST: [CurrencyId; 6] = [NATIVE, STABLECOIN, LIQUID, STAKING, BNC, VSKSM];

fn assert_last_event(generic_event: RuntimeEvent) {
	System::assert_last_event(generic_event.into());
}

runtime_benchmarks! {
	{ Runtime, module_dex }

	// enable a Disabled trading pair
	enable_trading_pair {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			let _ = Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second());
		}
	}: _(RawOrigin::Root, trading_pair.first(), trading_pair.second())
	verify {
		assert_last_event(module_dex::Event::EnableTradingPair{trading_pair: trading_pair}.into());
	}

	// disable a Enabled trading pair
	disable_trading_pair {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Disabled = Dex::trading_pair_statuses(trading_pair) {
			let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second());
		}
	}: _(RawOrigin::Root, trading_pair.first(), trading_pair.second())
	verify {
		assert_last_event(module_dex::Event::DisableTradingPair{trading_pair}.into());
	}

	// list a Provisioning trading pair
	list_provisioning {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
	}: _(RawOrigin::Root, trading_pair.first(), trading_pair.second(), dollar(trading_pair.first()), dollar(trading_pair.second()), dollar(trading_pair.first()), dollar(trading_pair.second()), 10)
	verify {
		assert_last_event(module_dex::Event::ListProvisioning{trading_pair: trading_pair}.into());
	}

	// update parameters of a Provisioning trading pair
	update_provisioning_parameters {
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			1000 * dollar(trading_pair.second()),
			100
		)?;
	}: _(RawOrigin::Root, trading_pair.first(), trading_pair.second(), 2 * dollar(trading_pair.first()), 2 * dollar(trading_pair.second()), 10 * dollar(trading_pair.first()), 100 * dollar(trading_pair.second()), 200)

	// end a Provisioning trading pair
	end_provisioning {
		let founder: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
			0
		)?;

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &founder, (100 * dollar(trading_pair.first())).unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &founder, (100 * dollar(trading_pair.second())).unique_saturated_into())?;

		// add enough provision
		Dex::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
		)?;
	}: _(RawOrigin::Signed(founder), trading_pair.first(), trading_pair.second())
	verify {
		assert_last_event(module_dex::Event::ProvisioningToEnabled{trading_pair, pool_0: 100 * dollar(trading_pair.first()), pool_1: 100 * dollar(trading_pair.second()), share_amount: 200 * dollar(trading_pair.first())}.into())
	}

	add_provision {
		let founder: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			1000 * dollar(trading_pair.second()),
			0
		)?;

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &founder, (10 * dollar(trading_pair.first())).unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &founder, (10 * dollar(trading_pair.second())).unique_saturated_into())?;
	}: _(RawOrigin::Signed(founder.clone()), trading_pair.first(), trading_pair.second(), dollar(trading_pair.first()), dollar(trading_pair.second()))
	verify{
		assert_last_event(module_dex::Event::AddProvision{who: founder, currency_0: trading_pair.first(), contribution_0: dollar(trading_pair.first()), currency_1: trading_pair.second(), contribution_1: dollar(trading_pair.second())}.into());
	}

	claim_dex_share {
		let founder: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
			0
		)?;

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &founder, (100 * dollar(trading_pair.first())).unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &founder, (100 * dollar(trading_pair.second())).unique_saturated_into())?;

		Dex::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			20 * dollar(trading_pair.second())
		)?;
		Dex::end_provisioning(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
		)?;
	}: _(RawOrigin::Signed(whitelisted_caller()), founder.clone(), trading_pair.first(), trading_pair.second())
	verify {
		assert_eq!(Currencies::free_balance(trading_pair.dex_share_currency_id(), &founder), 2_000_000_000_000);
	}

	// add liquidity but don't staking lp
	add_liquidity {
		let first_maker: AccountId = account("first_maker", 0, SEED);
		let second_maker: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		let amount_a = 100 * dollar(trading_pair.first());
		let amount_b = 10_000 * dollar(trading_pair.second());

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &second_maker, amount_a.unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &second_maker, amount_b.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity(first_maker.clone(), trading_pair.first(), trading_pair.second(), amount_a, amount_b, false)?;
	}: add_liquidity(RawOrigin::Signed(second_maker), trading_pair.first(), trading_pair.second(), amount_a, amount_b, Default::default(), false)

	// worst: add liquidity and stake lp
	add_liquidity_and_stake {
		let first_maker: AccountId = account("first_maker", 0, SEED);
		let second_maker: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		let amount_a = 100 * dollar(trading_pair.first());
		let amount_b = 10_000 * dollar(trading_pair.second());

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &second_maker, amount_a.unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &second_maker, amount_b.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity(first_maker.clone(), trading_pair.first(), trading_pair.second(), amount_a, amount_b, true)?;
	}: add_liquidity(RawOrigin::Signed(second_maker), trading_pair.first(), trading_pair.second(), amount_a, amount_b, Default::default(), true)

	// remove liquidity by liquid lp share
	remove_liquidity {
		let maker: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		inject_liquidity(maker.clone(), trading_pair.first(), trading_pair.second(), 100 * dollar(trading_pair.first()), 10_000 * dollar(trading_pair.second()), false)?;
	}: remove_liquidity(RawOrigin::Signed(maker), trading_pair.first(), trading_pair.second(), 50 * dollar(trading_pair.first()), Default::default(), Default::default(), false)

	// remove liquidity by withdraw staking lp share
	remove_liquidity_by_unstake {
		let maker: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		inject_liquidity(maker.clone(), trading_pair.first(), trading_pair.second(), 100 * dollar(trading_pair.first()), 10_000 * dollar(trading_pair.second()), true)?;
	}: remove_liquidity(RawOrigin::Signed(maker), trading_pair.first(), trading_pair.second(), 50 * dollar(trading_pair.first()), Default::default(), Default::default(), true)

	swap_with_exact_supply {
		let u in 2 .. <Runtime as module_dex::Config>::TradingPathLimit::get();

		let maker: AccountId = account("maker", 0, SEED);
		let taker: AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity(maker.clone(), cur0, cur1, 10_000 * dollar(cur0), 10_000 * dollar(cur1), false)?;
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity(maker.clone(), CURRENCY_LIST[i as usize - 1], CURRENCY_LIST[i as usize], 10_000 * dollar(CURRENCY_LIST[i as usize - 1]), 10_000 * dollar(CURRENCY_LIST[i as usize]), false)?;
			}
		}

		<Currencies as MultiCurrencyExtended<_>>::update_balance(path[0], &taker, (10_000 * dollar(path[0])).unique_saturated_into())?;
	}: swap_with_exact_supply(RawOrigin::Signed(taker.clone()), path.clone(), 100 * dollar(path[0]), 0)
	verify {
		let path_limit: u32 = <Runtime as module_dex::Config>::TradingPathLimit::get();
		// would panic the benchmark anyways, must add new currencies to CURRENCY_LIST for benchmarking to work
		assert!( path_limit < CURRENCY_LIST.len() as u32);
	}

	swap_with_exact_target {
		let u in 2 .. <Runtime as module_dex::Config>::TradingPathLimit::get();

		let maker: AccountId = account("maker", 0, SEED);
		let taker: AccountId = whitelisted_caller();

		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				let cur0 = CURRENCY_LIST[0];
				let cur1 = CURRENCY_LIST[1];
				path.push(cur0);
				path.push(cur1);
				inject_liquidity(maker.clone(), cur0, cur1, 10_000 * dollar(cur0), 10_000 * dollar(cur1), false)?;
			} else {
				path.push(CURRENCY_LIST[i as usize]);
				inject_liquidity(maker.clone(), CURRENCY_LIST[i as usize - 1], CURRENCY_LIST[i as usize], 10_000 * dollar(CURRENCY_LIST[i as usize - 1]), 10_000 * dollar(CURRENCY_LIST[i as usize]), false)?;
			}
		}

		<Currencies as MultiCurrencyExtended<_>>::update_balance(path[0], &taker, (10_000 * dollar(path[0])).unique_saturated_into())?;
	}: swap_with_exact_target(RawOrigin::Signed(taker.clone()), path.clone(), 10 * dollar(path[path.len() - 1]), 100 * dollar(path[0]))
	verify {
		let path_limit: u32 = <Runtime as module_dex::Config>::TradingPathLimit::get();
		// would panic the benchmark anyways, must add new currencies to CURRENCY_LIST for benchmarking to work
		assert!(path_limit < CURRENCY_LIST.len() as u32);
	}

	refund_provision {
		let founder: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
			0
		)?;

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &founder, (100 * dollar(trading_pair.first())).unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &founder, (100 * dollar(trading_pair.second())).unique_saturated_into())?;

		Dex::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second())
		)?;

		System::set_block_number(ExtendedProvisioningBlocks::get() + 1);
		Dex::abort_provisioning(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
		)?;
	}: _(RawOrigin::Signed(founder.clone()), founder.clone(), trading_pair.first(), trading_pair.second())

	abort_provisioning {
		let founder: AccountId = whitelisted_caller();
		let trading_pair = TradingPair::from_currency_ids(STABLECOIN, NATIVE).unwrap();
		if let TradingPairStatus::Enabled = Dex::trading_pair_statuses(trading_pair) {
			Dex::disable_trading_pair(RawOrigin::Root.into(), trading_pair.first(), trading_pair.second())?;
		}
		Dex::list_provisioning(
			RawOrigin::Root.into(),
			trading_pair.first(),
			trading_pair.second(),
			dollar(trading_pair.first()),
			dollar(trading_pair.second()),
			100 * dollar(trading_pair.first()),
			100 * dollar(trading_pair.second()),
			0
		)?;

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.first(), &founder, (100 * dollar(trading_pair.first())).unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.second(), &founder, (100 * dollar(trading_pair.second())).unique_saturated_into())?;

		Dex::add_provision(
			RawOrigin::Signed(founder.clone()).into(),
			trading_pair.first(),
			trading_pair.second(),
			10 * dollar(trading_pair.first()),
			10 * dollar(trading_pair.second()),
		)?;

		System::set_block_number(ExtendedProvisioningBlocks::get() + 1);
	}: _(RawOrigin::Signed(whitelisted_caller()), trading_pair.first(), trading_pair.second())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
