//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::{convert::TryInto, vec};

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::{self as system, RawOrigin};
use sp_runtime::traits::{UniqueSaturatedInto, Zero};

use cdp_engine::Module as CdpEngine;
use honzon::Module as Honzon;
use honzon::*;
use orml_oracle::OperatorProvider;
use orml_traits::{DataProvider, MultiCurrency, MultiCurrencyExtended};
use support::{OnEmergencyShutdown, Price, Rate, Ratio};

pub struct Module<T: Trait>(honzon::Module<T>);

pub trait Trait: honzon::Trait + orml_oracle::Trait + prices::Trait {}

type CurrencyIdOf<T> = <<T as loans::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type PriceCurrencyIdOf<T> = <T as prices::Trait>::CurrencyId;

type AmountOf<T> = <<T as loans::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

const SEED: u32 = 0;

benchmarks! {
	_ { }

	authorize {
		let u in 0 .. 1000;

		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
	}: _(RawOrigin::Signed(caller), currency_id, to)

	unauthorize {
		let u in 0 .. 1000;

		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let caller: T::AccountId = account("caller", u, SEED);
		let to: T::AccountId = account("to", u, SEED);
		Honzon::<T>::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			currency_id,
			to.clone()
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, to)

	unauthorize_all {
		let u in 0 .. 1000;
		let v in 0 .. 100;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		for i in 0 .. v {
			let to: T::AccountId = account("to", i, SEED);
			Honzon::<T>::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				currency_id,
				to
			)?;
		}
	}: _(RawOrigin::Signed(caller))

	adjust_loan {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];

		// set balance
		let min_debt_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let amount: u128 = (min_debt_value * 100.into()).unique_saturated_into();
		let amount: AmountOf<T> = amount.unique_saturated_into();
		<T as loans::Trait>::Currency::update_balance(currency_id, &caller, amount)?;

		// feed price
		let oracle_operators = <T as orml_oracle::Trait>::OperatorProvider::operators();
		let currency: u16 = currency_id.try_into().map_err(|_| sp_runtime::DispatchError::Other("error"))?;
		let currency: PriceCurrencyIdOf<T> = currency.try_into().map_err(|_| sp_runtime::DispatchError::Other("error"))?;
		//let price: OracleValueOf<T> = Price::from_natural(1).deconstruct().try_into().map_err(|_| sp_runtime::DispatchError::Other("error"))?;

		<T as prices::Trait>::Source::feed_value(
			oracle_operators[0].clone(),
			currency,
			Price::from_natural(1),
		)?;

		// Oracle::<T>::feed_value(
		// 	RawOrigin::Signed(oracle_operators[0].clone()).into(),
		// 	currency.into(),
		// 	price,
		// )?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			None,
			Some(Some(Ratio::from_rational(150, 100))),
			Some(Some(Rate::from_rational(10, 100))),
			Some(Some(Ratio::from_rational(150, 100))),
			Some(min_debt_value * 100.into()),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, amount, Zero::zero())

	adjust_collateral_after_shutdown {
		let u in 0 .. 1000;

		let caller: T::AccountId = account("caller", u, SEED);
		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let amount: AmountOf<T> = 100_000_000_000_000_000.unique_saturated_into();
		let amount: AmountOf<T> = amount * u.unique_saturated_into();
		<T as loans::Trait>::Currency::update_balance(currency_id, &caller, amount)?;
		Honzon::<T>::adjust_loan(
			RawOrigin::Signed(caller.clone()).into(),
			currency_id,
			amount,
			Zero::zero()
		)?;
		Honzon::<T>::on_emergency_shutdown();
	}: _(RawOrigin::Signed(caller), currency_id, -amount)

	transfer_loan_from {
		let u in 0 .. 1000;

		let currency_id: CurrencyIdOf<T> = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let sender: T::AccountId = account("sender", u, SEED);
		let receiver: T::AccountId = account("receiver", u, SEED);
		let amount: AmountOf<T> = 100_000_000_000_000_000.unique_saturated_into();
		let amount: AmountOf<T> = amount * u.unique_saturated_into();
		<T as loans::Trait>::Currency::update_balance(currency_id, &sender, amount)?;
		Honzon::<T>::adjust_loan(
			RawOrigin::Signed(sender.clone()).into(),
			currency_id,
			amount,
			Zero::zero()
		)?;
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
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_authorize::<Runtime>());
			assert_ok!(test_benchmark_unauthorize::<Runtime>());
			assert_ok!(test_benchmark_unauthorize_all::<Runtime>());
			assert_ok!(test_benchmark_adjust_loan::<Runtime>());
		});
	}
}
