//! DEX module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Module as Dex;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;
use sp_std::vec;

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn inject_liquidity<T: Trait>(
	maker: T::AccountId,
	currency_id: CurrencyId,
	max_amount: Balance,
	max_other_currency_amount: Balance,
) -> Result<(), &'static str> {
	let base_currency_id = T::GetBaseCurrencyId::get();

	// set balance
	T::Currency::update_balance(currency_id, &maker, max_amount.unique_saturated_into())?;
	T::Currency::update_balance(
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
	_ {}

	set_liquidity_incentive_rate {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, CurrencyId::DOT, Rate::from_rational(1, 10000000))

	add_liquidity {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let base_currency_id = T::GetBaseCurrencyId::get();
		let other_currency_amount = dollar(100);
		let base_currency_amount = dollar(10000);

		// set balance
		T::Currency::update_balance(currency_id, &maker, other_currency_amount.unique_saturated_into())?;
		T::Currency::update_balance(base_currency_id, &maker, base_currency_amount.unique_saturated_into())?;

	}: _(RawOrigin::Signed(maker), currency_id, other_currency_amount, base_currency_amount)

	withdraw_liquidity_without_interest {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
	}: withdraw_liquidity(RawOrigin::Signed(maker), currency_id, dollar(50).unique_saturated_into())

	swap_currency {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let trader: T::AccountId = account("trader", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let base_currency_id = T::GetBaseCurrencyId::get();

		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
		T::Currency::update_balance(currency_id, &trader,  dollar(100).unique_saturated_into())?;
	}: _(RawOrigin::Signed(trader), currency_id, dollar(100), base_currency_id, 0.unique_saturated_into())

	withdraw_incentive_interest {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;

		// set incentive rate
		Dex::<T>::set_liquidity_incentive_rate(
			RawOrigin::Root.into(),
			currency_id,
			Rate::from_rational(1, 10),
		)?;

		// accumulate incentive interest
		Dex::<T>::accumulate_interest(currency_id);

	}: _(RawOrigin::Signed(maker), currency_id)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn set_liquidity_incentive_rate() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_liquidity_incentive_rate::<Runtime>());
		});
	}

	#[test]
	fn add_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity::<Runtime>());
		});
	}

	#[test]
	fn withdraw_liquidity_without_interest() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_withdraw_liquidity_without_interest::<Runtime>());
		});
	}

	#[test]
	fn swap_currency() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_swap_currency::<Runtime>());
		});
	}

	#[test]
	fn withdraw_incentive_interest() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_withdraw_incentive_interest::<Runtime>());
		});
	}
}
