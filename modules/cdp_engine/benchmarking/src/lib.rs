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
use orml_oracle::OperatorProvider;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::{Balance, CurrencyId};
use support::{ExchangeRate, OnEmergencyShutdown, Price, Rate, Ratio};

pub struct Module<T: Trait>(cdp_engine::Module<T>);

pub trait Trait: cdp_engine::Trait + orml_oracle::Trait + prices::Trait {}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn feed_price<T: Trait>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = <T as orml_oracle::Trait>::OperatorProvider::operators();
	for operator in oracle_operators {
		<T as prices::Trait>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

benchmarks! {
	_ { }

	set_collateral_params {
		let u in 0 .. 1000;
	}: _(
		RawOrigin::Root,
		CurrencyId::DOT,
		Some(Some(Rate::from_rational(1, 1000000))),
		Some(Some(Ratio::from_rational(150, 100))),
		Some(Some(Rate::from_rational(20, 100))),
		Some(Some(Ratio::from_rational(180, 100))),
		Some(dollar(100000))
	)

	set_global_params {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, Rate::from_rational(1, 1000000))

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
			None,
			Some(Some(Ratio::from_rational(150, 100))),
			Some(Some(Rate::from_rational(10, 100))),
			Some(Some(Ratio::from_rational(150, 100))),
			Some(min_debit_value * 100),
		)?;

		// adjust position
		CdpEngine::<T>::adjust_position(&owner, currency_id, collateral_amount, min_debit_amount)?;

		// modify liquidation rate to make the cdp unsafe
		CdpEngine::<T>::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			None,
			Some(Some(Ratio::from_rational(1000, 100))),
			None,
			None,
			None,
		)?;
	}: liquidate(RawOrigin::None, currency_id, owner)

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
			None,
			Some(Some(Ratio::from_rational(150, 100))),
			Some(Some(Rate::from_rational(10, 100))),
			Some(Some(Ratio::from_rational(150, 100))),
			Some(min_debit_value * 100),
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
	fn settle() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_settle::<Runtime>());
		});
	}
}
