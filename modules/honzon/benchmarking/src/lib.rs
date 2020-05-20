//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::{Saturating, UniqueSaturatedInto};

use cdp_engine::Module as CdpEngine;
use honzon::Module as Honzon;
use honzon::*;
use orml_oracle::OperatorProvider;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::CurrencyId;
use support::{ExchangeRate, Price, Rate, Ratio};

pub struct Module<T: Trait>(honzon::Module<T>);

pub trait Trait: honzon::Trait + orml_oracle::Trait + prices::Trait {}

const SEED: u32 = 0;

fn feed_price<T: Trait>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = <T as orml_oracle::Trait>::OperatorProvider::operators();
	for operator in oracle_operators {
		<T as prices::Trait>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

benchmarks! {
	_ { }

	authorize {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
	}: _(RawOrigin::Signed(caller), CurrencyId::DOT, to)

	unauthorize {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
		Honzon::<T>::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			CurrencyId::DOT,
			to.clone()
		)?;
	}: _(RawOrigin::Signed(caller), CurrencyId::DOT, to)

	unauthorize_all {
		let u in 0 .. 1000;
		let v in 0 .. 100;

		let caller: T::AccountId = account("caller", u, SEED);
		for i in 0 .. v {
			let to: T::AccountId = account("to", i, SEED);
			Honzon::<T>::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				CurrencyId::DOT,
				to
			)?;
		}
	}: _(RawOrigin::Signed(caller))

	adjust_loan {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_id: CurrencyId = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let min_debit_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::from_natural(1);		// 1 USD
		let min_debit_amount = ExchangeRate::from_natural(1).checked_div(&debit_exchange_rate).unwrap().saturating_add(ExchangeRate::from_parts(1)).saturating_mul_int(&min_debit_value);
		let min_debit_amount: T::DebitAmount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10.into();
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		<T as loans::Trait>::Currency::update_balance(currency_id, &caller, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, collateral_price)?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			None,
			Some(Some(Ratio::from_rational(150, 100))),
			Some(Some(Rate::from_rational(10, 100))),
			Some(Some(Ratio::from_rational(150, 100))),
			Some(min_debit_value * 100),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, collateral_amount, debit_amount)

	transfer_loan_from {
		let u in 0 .. 1000;

		let currency_id: CurrencyId = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let sender: T::AccountId = account("sender", u, SEED);
		let receiver: T::AccountId = account("receiver", u, SEED);
		let min_debit_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::from_natural(1);		// 1 USD
		let min_debit_amount = ExchangeRate::from_natural(1).checked_div(&debit_exchange_rate).unwrap().saturating_add(ExchangeRate::from_parts(1)).saturating_mul_int(&min_debit_value);
		let min_debit_amount: T::DebitAmount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10.into();
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		<T as loans::Trait>::Currency::update_balance(currency_id, &sender, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, Price::from_natural(1))?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			None,
			Some(Some(Ratio::from_rational(150, 100))),
			Some(Some(Rate::from_rational(10, 100))),
			Some(Some(Ratio::from_rational(150, 100))),
			Some(min_debit_value * 100),
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
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			receiver.clone()
		)?;
	}: _(RawOrigin::Signed(receiver), currency_id, sender)
}

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
