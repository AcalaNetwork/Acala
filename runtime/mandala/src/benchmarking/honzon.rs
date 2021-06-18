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
	dollar, AcalaOracle, AccountId, Amount, CdpEngine, CollateralCurrencyIds, CurrencyId, DepositPerAuthorization, Dex,
	ExistentialDeposits, Honzon, Indices, Price, Rate, Ratio, Runtime, TradingPathLimit, ACA, AUSD, DOT,
};

use super::utils::set_balance;
use core::convert::TryInto;
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{Change, GetByKey};
use sp_runtime::{
	traits::{One, StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_honzon }

	authorize {
		let caller: AccountId = whitelisted_caller();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);

		// set balance
		set_balance(ACA, &caller, DepositPerAuthorization::get());
	}: _(RawOrigin::Signed(caller), DOT, to_lookup)

	unauthorize {
		let caller: AccountId = whitelisted_caller();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);

		// set balance
		set_balance(ACA, &caller, DepositPerAuthorization::get());
		Honzon::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			DOT,
			to_lookup.clone()
		)?;
	}: _(RawOrigin::Signed(caller), DOT, to_lookup)

	unauthorize_all {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;

		let caller: AccountId = whitelisted_caller();
		let currency_ids = CollateralCurrencyIds::get();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = Indices::unlookup(to);

		// set balance
		set_balance(ACA, &caller, DepositPerAuthorization::get().saturating_mul(c.into()));
		for i in 0 .. c {
			Honzon::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				currency_ids[i as usize],
				to_lookup.clone(),
			)?;
		}
	}: _(RawOrigin::Signed(caller))

	// `adjust_loan`, best case:
	// adjust both collateral and debit
	adjust_loan {
		let caller: AccountId = whitelisted_caller();
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let collateral_price = Price::one();		// 1 USD
		let debit_value = 100 * dollar(AUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(AUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &caller, collateral_amount + ExistentialDeposits::get(&currency_id));

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
			Change::NewValue(debit_value * 100),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, collateral_amount.try_into().unwrap(), debit_amount)

	transfer_loan_from {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let sender: AccountId = account("sender", 0, SEED);
		let sender_lookup = Indices::unlookup(sender.clone());
		let receiver: AccountId = whitelisted_caller();
		let receiver_lookup = Indices::unlookup(receiver.clone());

		let debit_value = 100 * dollar(AUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(AUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &sender, collateral_amount + ExistentialDeposits::get(&currency_id));
		set_balance(ACA, &sender, DepositPerAuthorization::get());

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;

		// initialize sender's loan
		Honzon::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount,
		)?;

		// authorize receiver
		Honzon::authorize(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			receiver_lookup,
		)?;
	}: _(RawOrigin::Signed(receiver), currency_id, sender_lookup)

	close_loan_has_debit_by_dex {
		let u in 2 .. TradingPathLimit::get() as u32;
		let collateral_currency_ids = CollateralCurrencyIds::get();
		let currency_id: CurrencyId = *collateral_currency_ids.last().unwrap();
		let sender: AccountId = whitelisted_caller();
		let maker: AccountId = account("maker", 0, SEED);
		let debit_value = 100 * dollar(AUSD);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(AUSD)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &sender, collateral_amount + ExistentialDeposits::get(&currency_id));
		set_balance(currency_id, &maker, collateral_amount * 2);
		set_balance(ACA, &maker, collateral_amount * 2);
		set_balance(AUSD, &maker, debit_value * 200);

		// inject liquidity
		let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id, AUSD);
		let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id, ACA);
		let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), ACA, AUSD);
		Dex::add_liquidity(
			RawOrigin::Signed(maker.clone()).into(),
			currency_id,
			AUSD,
			collateral_amount,
			debit_value * 100,
			Default::default(),
			false,
		)?;
		Dex::add_liquidity(
			RawOrigin::Signed(maker.clone()).into(),
			currency_id,
			ACA,
			collateral_amount,
			collateral_amount,
			Default::default(),
			false,
		)?;
		Dex::add_liquidity(
			RawOrigin::Signed(maker.clone()).into(),
			ACA,
			AUSD,
			collateral_amount,
			debit_value * 100,
			Default::default(),
			false,
		)?;

		let mut path = vec![currency_id];
		for i in 2 .. u {
			if i % 2 == 0 {
				path.push(ACA);
			} else {
				path.push(currency_id);
			}
		}
		path.push(AUSD);

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(debit_value * 100),
		)?;

		// initialize sender's loan
		Honzon::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			collateral_amount.try_into().unwrap(),
			debit_amount,
		)?;
	}: _(RawOrigin::Signed(sender), currency_id, Some(path))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
