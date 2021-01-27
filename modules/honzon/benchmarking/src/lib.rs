//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::{
	traits::{Saturating, StaticLookup, UniqueSaturatedInto},
	FixedPointNumber,
};

use cdp_engine::Module as CdpEngine;
use honzon::Module as Honzon;
use honzon::*;
use orml_traits::{Change, DataFeeder, MultiCurrencyExtended};
use primitives::{Amount, CurrencyId, TokenSymbol};
use support::{ExchangeRate, Price, Rate, Ratio};

pub struct Module<T: Config>(honzon::Module<T>);

pub trait Config: honzon::Config + orml_oracle::Config<orml_oracle::Instance1> + prices::Config {}

const SEED: u32 = 0;

fn feed_price<T: Config>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = orml_oracle::Module::<T, orml_oracle::Instance1>::members().0;
	for operator in oracle_operators {
		<T as prices::Config>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

benchmarks! {
	authorize {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
		let to_lookup = T::Lookup::unlookup(to);
	}: _(RawOrigin::Signed(caller), CurrencyId::Token(TokenSymbol::DOT), to_lookup)

	unauthorize {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
		let to_lookup = T::Lookup::unlookup(to);
		Honzon::<T>::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			CurrencyId::Token(TokenSymbol::DOT),
			to_lookup.clone()
		)?;
	}: _(RawOrigin::Signed(caller), CurrencyId::Token(TokenSymbol::DOT), to_lookup)

	unauthorize_all {
		let u in 0 .. 1000;
		let v in 0 .. 100;
		let c in 0 .. <T as cdp_engine::Config>::CollateralCurrencyIds::get().len().saturating_sub(1) as u32;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_ids = <T as cdp_engine::Config>::CollateralCurrencyIds::get();

		for i in 0 .. v {
			let to: T::AccountId = account("to", i, SEED);
			let to_lookup = T::Lookup::unlookup(to);

			for j in 0 .. c {
				Honzon::<T>::authorize(
					RawOrigin::Signed(caller.clone()).into(),
					currency_ids[j as usize],
					to_lookup.clone(),
				)?;
			}
		}
	}: _(RawOrigin::Signed(caller))

	// `adjust_loan`, best case:
	// adjust both collateral and debit
	adjust_loan {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_id: CurrencyId = <T as cdp_engine::Config>::CollateralCurrencyIds::get()[0];
		let min_debit_value = <T as cdp_engine::Config>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_add(ExchangeRate::from_inner(1)).saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10;
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		<T as loans::Config>::Currency::update_balance(currency_id, &caller, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, collateral_price)?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, collateral_amount, debit_amount)

	transfer_loan_from {
		let u in 0 .. 1000;

		let currency_id: CurrencyId = <T as cdp_engine::Config>::CollateralCurrencyIds::get()[0];
		let sender: T::AccountId = account("sender", u, SEED);
		let sender_lookup = T::Lookup::unlookup(sender.clone());
		let receiver: T::AccountId = account("receiver", u, SEED);
		let receiver_lookup = T::Lookup::unlookup(receiver.clone());
		let min_debit_value = <T as cdp_engine::Config>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_add(ExchangeRate::from_inner(1)).saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10;
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		<T as loans::Config>::Currency::update_balance(currency_id, &sender, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, Price::one())?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;

		// initialize sender's loan
		Honzon::<T>::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			collateral_amount,
			debit_amount,
		)?;

		// authorize receiver
		Honzon::<T>::authorize(
			RawOrigin::Signed(sender).into(),
			currency_id,
			receiver_lookup
		)?;
	}: _(RawOrigin::Signed(receiver), currency_id, sender_lookup)
}

#[cfg(feature = "runtime-benchmarks")]
#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn authorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_authorize::<Runtime>());
		});
	}

	#[test]
	fn unauthorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize::<Runtime>());
		});
	}

	#[test]
	fn unauthorize_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize_all::<Runtime>());
		});
	}

	#[test]
	fn adjust_loan() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_adjust_loan::<Runtime>());
		});
	}
}
