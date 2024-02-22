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

use crate::{
	AccountId, Address, Amount, CdpEngine, CdpTreasury, CurrencyId, DefaultDebitExchangeRate, Dex, EmergencyShutdown,
	ExistentialDeposits, MinimumDebitValue, NativeTokenExistentialDeposit, Price, Rate, Ratio, Runtime, H160,
	MILLISECS_PER_BLOCK,
};

use super::{
	get_benchmarking_collateral_currency_ids,
	utils::{
		dollar, feed_price, inject_liquidity, set_balance, set_block_number_timestamp, LIQUID, NATIVE, STABLECOIN,
		STAKING,
	},
};
use frame_benchmarking::account;
use frame_support::traits::{Get, OnInitialize};
use frame_system::RawOrigin;
use module_support::DEXManager;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{Change, GetByKey};
use sp_runtime::{
	traits::{AccountIdLookup, One, StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_cdp_engine }

	on_initialize {
		let c in 0 .. get_benchmarking_collateral_currency_ids().len() as u32;
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup: Address = AccountIdLookup::unlookup(owner.clone());
		let currency_ids = get_benchmarking_collateral_currency_ids();
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = DefaultDebitExchangeRate::get();
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;

		// feed price
		let mut feed_data: Vec<(CurrencyId, Price)> = vec![];
		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let collateral_price = Price::one();
			feed_data.push((currency_id, collateral_price));
		}
		feed_price(feed_data)?;

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			if matches!(currency_id, CurrencyId::StableAssetPoolToken(_)) {
				continue;
			}
			let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

			let ed = if currency_id == NATIVE {
				NativeTokenExistentialDeposit::get()
			} else {
				ExistentialDeposits::get(&currency_id)
			};

			// set balance
			set_balance(currency_id, &owner, collateral_amount + ed);

			CdpEngine::set_collateral_params(
				RawOrigin::Root.into(),
				currency_id,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(0, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(0, 100))),
				Change::NewValue(min_debit_value * 100),
			)?;

			// adjust position
			CdpEngine::adjust_position(&owner, currency_id, collateral_amount.try_into().unwrap(), min_debit_amount)?;
		}

		set_block_number_timestamp(2, MILLISECS_PER_BLOCK);
		CdpEngine::on_initialize(2);
	}: {
		set_block_number_timestamp(3, MILLISECS_PER_BLOCK * 2);
		CdpEngine::on_initialize(3);
	}

	set_collateral_params {
	}: _(
		RawOrigin::Root,
		STAKING,
		Change::NewValue(Some(Rate::saturating_from_rational(1, 1000000))),
		Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
		Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
		Change::NewValue(Some(Ratio::saturating_from_rational(180, 100))),
		Change::NewValue(100_000 * dollar(STABLECOIN))
	)

	// `liquidate` by_auction
	liquidate_by_auction {
		let b in 1 .. <Runtime as module_cdp_treasury::Config>::MaxAuctionsCount::get();

		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(STAKING);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(STAKING), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(STAKING, &owner, collateral_amount + ExistentialDeposits::get(&STAKING));

		// feed price
		feed_price(vec![(STAKING, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;

		let auction_size = collateral_amount / b as u128;
		// adjust auction size so we hit MaxAuctionCount
		CdpTreasury::set_expected_collateral_auction_size(RawOrigin::Root.into(), STAKING, auction_size)?;
		// adjust position
		CdpEngine::adjust_position(&owner, STAKING, collateral_amount.try_into().unwrap(), min_debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		)?;
	}: liquidate(RawOrigin::None, STAKING, owner_lookup)

	// `liquidate` by dex
	liquidate_by_dex {
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let funder: AccountId = account("funder", 0, SEED);
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(LIQUID);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 2 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(LIQUID), dollar(STABLECOIN)).saturating_mul_int(collateral_value);
		let collateral_price = Price::one();		// 1 USD

		set_balance(LIQUID, &owner, (10 * collateral_amount) + ExistentialDeposits::get(&LIQUID));
		inject_liquidity(funder.clone(), LIQUID, STAKING, 10_000 * dollar(LIQUID), 10_000 * dollar(STAKING), false)?;
		inject_liquidity(funder, STAKING, STABLECOIN, 10_000 * dollar(STAKING), 10_000 * dollar(STABLECOIN), false)?;

		// feed price
		feed_price(vec![(STAKING, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			LIQUID,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;

		// adjust position
		CdpEngine::adjust_position(&owner, LIQUID, (10 * collateral_amount).try_into().unwrap(), debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			LIQUID,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		)?;
	}: liquidate(RawOrigin::None, LIQUID, owner_lookup)
	verify {
		let (_, stable_amount) = Dex::get_liquidity_pool(STAKING, STABLECOIN);
		let (_, stable_amount_mandala) = Dex::get_liquidity_pool(LIQUID, STABLECOIN);
		// paths of karura and acala are LIQUID => STAKING => STABLECOIN
		#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
		assert!(stable_amount < 10_000 * dollar(STABLECOIN));
		// path of mandala is LIQUID => STABLECOIN
		#[cfg(feature = "with-mandala-runtime")]
		assert!(stable_amount_mandala < 10_000 * dollar(STABLECOIN));
	}

	settle {
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = AccountIdLookup::unlookup(owner.clone());
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(STAKING);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount = Price::saturating_from_rational(1_000 * dollar(STAKING), 1000 * dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(STAKING, &owner, collateral_amount + ExistentialDeposits::get(&STAKING));

		// feed price
		feed_price(vec![(STAKING, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			STAKING,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::adjust_position(&owner, STAKING, collateral_amount.try_into().unwrap(), min_debit_amount)?;

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::None, STAKING, owner_lookup)

	register_liquidation_contract {
	}: _(RawOrigin::Root, H160::default())

	deregister_liquidation_contract {
		CdpEngine::register_liquidation_contract(RawOrigin::Root.into(), H160::default())?;
	}: _(RawOrigin::Root, H160::default())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
