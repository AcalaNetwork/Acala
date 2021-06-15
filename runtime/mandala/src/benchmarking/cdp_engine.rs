// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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
	dollar, AcalaOracle, AccountId, Amount, Balance, CdpEngine, CollateralCurrencyIds, CurrencyId,
	DefaultDebitExchangeRate, Dex, EmergencyShutdown, ExistentialDeposits, GetStableCurrencyId, Indices,
	MaxSlippageSwapWithDEX, MinimumDebitValue, Price, Rate, Ratio, Runtime, AUSD, DOT, MILLISECS_PER_BLOCK,
};

use super::utils::set_balance;
use core::convert::TryInto;
use frame_benchmarking::account;
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;
use module_support::DEXManager;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{Change, GetByKey};
use sp_runtime::{
	traits::{One, StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

fn inject_liquidity(
	maker: AccountId,
	currency_id: CurrencyId,
	max_amount: Balance,
	max_other_currency_amount: Balance,
) -> Result<(), &'static str> {
	let base_currency_id = GetStableCurrencyId::get();

	// set balance
	set_balance(currency_id, &maker, max_other_currency_amount.unique_saturated_into());
	set_balance(base_currency_id, &maker, max_amount.unique_saturated_into());

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id, base_currency_id);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		base_currency_id,
		currency_id,
		max_amount,
		max_other_currency_amount,
		Default::default(),
		false,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_cdp_engine }

	on_initialize {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = Indices::unlookup(owner.clone());
		let currency_ids = CollateralCurrencyIds::get();
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
		AcalaOracle::feed_values(RawOrigin::Root.into(), feed_data)?;

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(AUSD)).saturating_mul_int(collateral_value);

			// set balance
			set_balance(currency_id, &owner, collateral_amount + ExistentialDeposits::get(&currency_id));

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

		// set timestamp by set storage, this is deprecated,
		// replace it by following after https://github.com/paritytech/substrate/pull/8601 is available:
		// Timestamp::set_timestamp(MILLISECS_PER_BLOCK);
		pallet_timestamp::Now::<Runtime>::put(MILLISECS_PER_BLOCK);

		CdpEngine::on_initialize(2);
	}: {
		// set timestamp by set storage, this is deprecated,
		// replace it by following after https://github.com/paritytech/substrate/pull/8601 is available:
		// Timestamp::set_timestamp(MILLISECS_PER_BLOCK * 2);
		pallet_timestamp::Now::<Runtime>::put(MILLISECS_PER_BLOCK * 2);

		CdpEngine::on_initialize(3);
	}

	set_collateral_params {
	}: _(
		RawOrigin::Root,
		DOT,
		Change::NewValue(Some(Rate::saturating_from_rational(1, 1000000))),
		Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
		Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
		Change::NewValue(Some(Ratio::saturating_from_rational(180, 100))),
		Change::NewValue(100_000 * dollar(AUSD))
	)

	set_global_params {
	}: _(RawOrigin::Root, Rate::saturating_from_rational(1, 1000000))

	// `liquidate` by_auction
	liquidate_by_auction {
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = Indices::unlookup(owner.clone());
		let currency_id: CurrencyId = DOT;
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(DOT), dollar(AUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &owner, collateral_amount + ExistentialDeposits::get(&currency_id));

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::adjust_position(&owner, currency_id, collateral_amount.try_into().unwrap(), min_debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		)?;
	}: liquidate(RawOrigin::None, currency_id, owner_lookup)

	// `liquidate` by dex
	liquidate_by_dex {
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = Indices::unlookup(owner.clone());
		let funder: AccountId = account("funder", 0, SEED);

		let debit_value = 100 * dollar(AUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(DOT);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 2 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(DOT), dollar(AUSD)).saturating_mul_int(collateral_value);
		let collateral_price = Price::one();		// 1 USD
		let max_slippage_swap_with_dex = MaxSlippageSwapWithDEX::get();
		let collateral_amount_in_dex = max_slippage_swap_with_dex.reciprocal().unwrap().saturating_mul_int(collateral_amount);
		let base_amount_in_dex = max_slippage_swap_with_dex.reciprocal().unwrap().saturating_mul_int(debit_value * 2);

		inject_liquidity(funder.clone(), DOT, base_amount_in_dex, collateral_amount_in_dex)?;

		// set balance
		set_balance(DOT, &owner, collateral_amount + ExistentialDeposits::get(&DOT));

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(DOT, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			DOT,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;

		// adjust position
		CdpEngine::adjust_position(&owner, DOT, collateral_amount.try_into().unwrap(), debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			DOT,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(1000, 100))),
			Change::NoChange,
			Change::NoChange,
			Change::NoChange,
		)?;
	}: liquidate(RawOrigin::None, DOT, owner_lookup)
	verify {
		let (other_currency_amount, base_currency_amount) = Dex::get_liquidity_pool(DOT, AUSD);
		assert!(other_currency_amount > collateral_amount_in_dex);
		assert!(base_currency_amount < base_amount_in_dex);
	}

	settle {
		let owner: AccountId = account("owner", 0, SEED);
		let owner_lookup = Indices::unlookup(owner.clone());
		let currency_id: CurrencyId = DOT;
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let collateral_value = 2 * min_debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(DOT), dollar(AUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &owner, collateral_amount + ExistentialDeposits::get(&currency_id));

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::adjust_position(&owner, currency_id, collateral_amount.try_into().unwrap(), min_debit_amount)?;

		// shutdown
		EmergencyShutdown::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::None, currency_id, owner_lookup)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
