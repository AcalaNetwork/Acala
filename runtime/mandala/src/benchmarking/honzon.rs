// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
	AccountId, Amount, Balance, CdpEngine, CollateralCurrencyIds, Currencies, CurrencyId, DepositPerAuthorization, Dex,
	ExistentialDeposits, GetLiquidCurrencyId, GetNativeCurrencyId, GetStableCurrencyId, GetStakingCurrencyId, Honzon,
	Price, Rate, Ratio, Runtime,
};

use super::utils::{dollar, feed_price, set_balance};
use frame_benchmarking::{account, whitelisted_caller};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{Change, GetByKey, MultiCurrencyExtended};
use sp_runtime::{
	traits::{AccountIdLookup, One, StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

const NATIVE: CurrencyId = GetNativeCurrencyId::get();
const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const STAKING: CurrencyId = GetStakingCurrencyId::get();
const LIQUID: CurrencyId = GetLiquidCurrencyId::get();

fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
	deposit: bool,
) -> Result<(), &'static str> {
	// set balance
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_a,
		&maker,
		max_amount_a.unique_saturated_into(),
	)?;
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_b,
		&maker,
		max_amount_b.unique_saturated_into(),
	)?;

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		deposit,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_honzon }

	authorize {
		let caller: AccountId = whitelisted_caller();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = AccountIdLookup::unlookup(to);

		// set balance
		set_balance(NATIVE, &caller, DepositPerAuthorization::get());
	}: _(RawOrigin::Signed(caller), STAKING, to_lookup)

	unauthorize {
		let caller: AccountId = whitelisted_caller();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = AccountIdLookup::unlookup(to);

		// set balance
		set_balance(NATIVE, &caller, DepositPerAuthorization::get());
		Honzon::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			STAKING,
			to_lookup.clone()
		)?;
	}: _(RawOrigin::Signed(caller), STAKING, to_lookup)

	unauthorize_all {
		let c in 0 .. CollateralCurrencyIds::get().len() as u32;

		let caller: AccountId = whitelisted_caller();
		let currency_ids = CollateralCurrencyIds::get();
		let to: AccountId = account("to", 0, SEED);
		let to_lookup = AccountIdLookup::unlookup(to);

		// set balance
		set_balance(NATIVE, &caller, DepositPerAuthorization::get().saturating_mul(c.into()));
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
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &caller, collateral_amount * 2);

		// feed price
		feed_price(vec![(currency_id, collateral_price)])?;

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
		let sender_lookup = AccountIdLookup::unlookup(sender.clone());
		let receiver: AccountId = whitelisted_caller();
		let receiver_lookup = AccountIdLookup::unlookup(receiver.clone());

		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance
		set_balance(currency_id, &sender, collateral_amount * 2);
		set_balance(NATIVE, &sender, DepositPerAuthorization::get());

		// feed price
		feed_price(vec![(currency_id, Price::one())])?;

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
		let currency_id: CurrencyId = LIQUID;
		let sender: AccountId = whitelisted_caller();
		let maker: AccountId = account("maker", 0, SEED);
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(LIQUID);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let debit_amount: Amount = debit_amount.unique_saturated_into();
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(LIQUID), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance and inject liquidity
		set_balance(LIQUID, &sender, (10 * collateral_amount) + ExistentialDeposits::get(&LIQUID));
		inject_liquidity(maker.clone(), LIQUID, STAKING, 10_000 * dollar(LIQUID), 10_000 * dollar(STAKING), false)?;
		inject_liquidity(maker, STAKING, STABLECOIN, 10_000 * dollar(STAKING), 10_000 * dollar(STABLECOIN), false)?;

		feed_price(vec![(STAKING, Price::one())])?;

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

		// initialize sender's loan
		Honzon::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			LIQUID,
			(10 * collateral_amount).try_into().unwrap(),
			debit_amount,
		)?;

	}: _(RawOrigin::Signed(sender), LIQUID, collateral_amount)

	expand_position_collateral {
		let currency_id: CurrencyId = STAKING;
		let sender: AccountId = whitelisted_caller();
		let maker: AccountId = account("maker", 0, SEED);
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance and inject liquidity
		set_balance(currency_id, &sender, (10 * collateral_amount) + ExistentialDeposits::get(&currency_id));
		inject_liquidity(maker, currency_id, STABLECOIN, 10_000 * dollar(currency_id), 10_000 * dollar(STABLECOIN), false)?;

		feed_price(vec![(currency_id, Price::one())])?;

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
			debit_amount.try_into().unwrap(),
		)?;
	}: _(RawOrigin::Signed(sender), currency_id, debit_value, 0)

	shrink_position_debit {
		let currency_id: CurrencyId = STAKING;
		let sender: AccountId = whitelisted_caller();
		let maker: AccountId = account("maker", 0, SEED);
		let debit_value = 100 * dollar(STABLECOIN);
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_mul_int(debit_value);
		let collateral_value = 10 * debit_value;
		let collateral_amount = Price::saturating_from_rational(dollar(currency_id), dollar(STABLECOIN)).saturating_mul_int(collateral_value);

		// set balance and inject liquidity
		set_balance(currency_id, &sender, (10 * collateral_amount) + ExistentialDeposits::get(&currency_id));
		inject_liquidity(maker, currency_id, STABLECOIN, 10_000 * dollar(currency_id), 10_000 * dollar(STABLECOIN), false)?;

		feed_price(vec![(currency_id, Price::one())])?;

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
			debit_amount.try_into().unwrap(),
		)?;
	}: _(RawOrigin::Signed(sender), currency_id, collateral_amount / 5, 0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
