//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;

use cdp_engine::Module as CdpEngine;
use cdp_engine::*;
use dex::Module as Dex;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::{Balance, CurrencyId};
use support::{ExchangeRate, OnEmergencyShutdown, Price, Rate, Ratio};

pub struct Module<T: Trait>(cdp_engine::Module<T>);

pub trait Trait: cdp_engine::Trait + orml_oracle::Trait + prices::Trait + dex::Trait {}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn feed_price<T: Trait>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = orml_oracle::Module::<T>::members().0;
	for operator in oracle_operators {
		<T as prices::Trait>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

fn inject_liquidity<T: Trait>(
	maker: T::AccountId,
	currency_id: CurrencyId,
	max_amount: Balance,
	max_other_currency_amount: Balance,
) -> Result<(), &'static str> {
	let base_currency_id = <T as dex::Trait>::GetBaseCurrencyId::get();

	// set balance
	<T as dex::Trait>::Currency::update_balance(currency_id, &maker, max_amount.unique_saturated_into())?;
	<T as dex::Trait>::Currency::update_balance(
		base_currency_id,
		&maker,
		max_other_currency_amount.unique_saturated_into(),
	)?;

	Dex::<T>::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id,
		max_amount,
		max_other_currency_amount,
	)?;

	Ok(())
}

benchmarks! {
	_ { }

	set_collateral_params {
		let u in 0 .. 1000;
	}: _(
		RawOrigin::Root,
		CurrencyId::DOT,
		CollateralParamChange::New(Some(Rate::from_rational(1, 1000000))),
		CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
		CollateralParamChange::New(Some(Rate::from_rational(20, 100))),
		CollateralParamChange::New(Some(Ratio::from_rational(180, 100))),
		CollateralParamChange::New(dollar(100000))
	)

	set_global_params {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, Rate::from_rational(1, 1000000))

	// `liquidate` by_auction
	liquidate_by_auction {
		let u in 0 .. 1000;

		let owner: T::AccountId = account("owner", u, SEED);
		let currency_id: CurrencyId = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let min_debit_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::from_natural(1);		// 1 USD
		let min_debit_amount = ExchangeRate::from_natural(1).checked_div(&debit_exchange_rate).unwrap().saturating_mul_int(&min_debit_value);
		let min_debit_amount: T::DebitAmount = min_debit_amount.unique_saturated_into();
		let collateral_amount = (min_debit_value * 2).unique_saturated_into();

		// set balance
		<T as loans::Trait>::Currency::update_balance(currency_id, &owner, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, collateral_price)?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(Some(Rate::from_rational(10, 100))),
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::<T>::adjust_position(&owner, currency_id, collateral_amount, min_debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::from_rational(1000, 100))),
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
		)?;
	}: liquidate(RawOrigin::None, currency_id, owner)

	// `liquidate` by dex
	liquidate_by_dex {
		let u in 0 .. 1000;

		let owner: T::AccountId = account("owner", u, SEED);
		let funder: T::AccountId = account("funder", u, SEED);
		let currency_id: CurrencyId = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let min_debit_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::from_natural(1);		// 1 USD
		let min_debit_amount = ExchangeRate::from_natural(1).checked_div(&debit_exchange_rate).unwrap().saturating_mul_int(&min_debit_value);
		let min_debit_amount: T::DebitAmount = min_debit_amount.unique_saturated_into();
		let collateral_amount = (min_debit_value * 2).unique_saturated_into();

		let max_slippage_swap_with_dex = <T as cdp_engine::Trait>::MaxSlippageSwapWithDEX::get();
		let collateral_amount_in_dex = Ratio::from_natural(1).checked_div(&max_slippage_swap_with_dex).unwrap().saturating_mul_int(&(min_debit_value * 10));
		let base_amount_in_dex = collateral_amount_in_dex * 2;

		inject_liquidity::<T>(funder.clone(), currency_id, base_amount_in_dex, collateral_amount_in_dex)?;

		// set balance
		<T as loans::Trait>::Currency::update_balance(currency_id, &owner, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, collateral_price)?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(Some(Rate::from_rational(10, 100))),
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::<T>::adjust_position(&owner, currency_id, collateral_amount, min_debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::from_rational(1000, 100))),
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
			CollateralParamChange::NoChange,
		)?;
	}: liquidate(RawOrigin::None, currency_id, owner)
	verify {
		let (other_currency_amount, base_currency_amount) = Dex::<T>::liquidity_pool(currency_id);
		assert!(other_currency_amount > collateral_amount_in_dex);
		assert!(base_currency_amount < base_amount_in_dex);
	}

	settle {
		let u in 0 .. 1000;

		let owner: T::AccountId = account("owner", u, SEED);
		let currency_id: CurrencyId = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
		let min_debit_value = <T as cdp_engine::Trait>::MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::<T>::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::from_natural(1);		// 1 USD
		let min_debit_amount = ExchangeRate::from_natural(1).checked_div(&debit_exchange_rate).unwrap().saturating_mul_int(&min_debit_value);
		let min_debit_amount: T::DebitAmount = min_debit_amount.unique_saturated_into();
		let collateral_amount = (min_debit_value * 2).unique_saturated_into();

		// set balance
		<T as loans::Trait>::Currency::update_balance(currency_id, &owner, collateral_amount)?;

		// feed price
		feed_price::<T>(currency_id, collateral_price)?;

		// set risk params
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			CollateralParamChange::NoChange,
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(Some(Rate::from_rational(10, 100))),
			CollateralParamChange::New(Some(Ratio::from_rational(150, 100))),
			CollateralParamChange::New(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::<T>::adjust_position(&owner, currency_id, collateral_amount, min_debit_amount)?;

		// shutdown
		CdpEngine::<T>::on_emergency_shutdown();
	}: _(RawOrigin::None, currency_id, owner)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn set_collateral_params() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_collateral_params::<Runtime>());
		});
	}

	#[test]
	fn set_global_params() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_global_params::<Runtime>());
		});
	}

	#[test]
	fn liquidate_by_auction() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_liquidate_by_auction::<Runtime>());
		});
	}

	#[test]
	fn liquidate_by_dex() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_liquidate_by_dex::<Runtime>());
		});
	}

	#[test]
	fn settle() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_settle::<Runtime>());
		});
	}
}
