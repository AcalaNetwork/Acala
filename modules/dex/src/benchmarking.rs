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

	let lp_share_currency_id = match (currency_id, base_currency_id) {
		(CurrencyId::Token(other_currency_symbol), CurrencyId::Token(base_currency_symbol)) => {
			CurrencyId::DEXShare(other_currency_symbol, base_currency_symbol)
		}
		_ => return Err("invalid currency id"),
	};

	Dex::<T>::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		lp_share_currency_id,
		max_amount,
		max_other_currency_amount,
	)?;

	Ok(())
}

benchmarks! {
	_ {}

	// `add_liquidity`, worst case:
	// already have other makers
	add_liquidity {
		let u in 0 .. 1000;

		let first_maker: T::AccountId = account("first_maker", u, SEED);
		let second_maker: T::AccountId = account("second_maker", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let base_currency_id = T::GetBaseCurrencyId::get();
		let other_currency_amount = dollar(100);
		let base_currency_amount = dollar(10000);

		// set balance
		T::Currency::update_balance(currency_id, &second_maker, other_currency_amount.unique_saturated_into())?;
		T::Currency::update_balance(base_currency_id, &second_maker, base_currency_amount.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity::<T>(first_maker.clone(), currency_id, dollar(100), dollar(10000))?;
	}: add_liquidity(RawOrigin::Signed(second_maker), currency_id, other_currency_amount, base_currency_amount)

	withdraw_liquidity {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
		let lp_share_currency_id = match (currency_id, T::GetBaseCurrencyId::get()) {
			(CurrencyId::Token(other_currency_symbol), CurrencyId::Token(base_currency_symbol)) => CurrencyId::DEXShare(other_currency_symbol, base_currency_symbol),
			_ => return Err("invalid currency id"),
		};
	}: withdraw_liquidity(RawOrigin::Signed(maker), lp_share_currency_id, dollar(50).unique_saturated_into())

	// `swap_currency`, best case:
	// swap other currency to base currency
	swap_other_to_base {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let trader: T::AccountId = account("trader", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let base_currency_id = T::GetBaseCurrencyId::get();

		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
		T::Currency::update_balance(currency_id, &trader,  dollar(100).unique_saturated_into())?;
	}: swap_currency(RawOrigin::Signed(trader), currency_id, dollar(100), base_currency_id, 0.unique_saturated_into())

	// `swap_currency`, best case:
	// swap base currency to other currency
	swap_base_to_other {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let trader: T::AccountId = account("trader", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let base_currency_id = T::GetBaseCurrencyId::get();

		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
		T::Currency::update_balance(base_currency_id, &trader, dollar(10000).unique_saturated_into())?;
	}: swap_currency(RawOrigin::Signed(trader), base_currency_id, dollar(10000), currency_id, 0.unique_saturated_into())

	// `swap_currency`, worst case:
	// swap other currency to another currency
	swap_other_to_other {
		let u in 0 .. 1000;

		let maker: T::AccountId = account("maker", u, SEED);
		let trader: T::AccountId = account("trader", u, SEED);
		let currency_id = T::EnabledCurrencyIds::get()[0];
		let another_currency_id = T::EnabledCurrencyIds::get()[1];

		inject_liquidity::<T>(maker.clone(), currency_id, dollar(100), dollar(10000))?;
		inject_liquidity::<T>(maker.clone(), another_currency_id, dollar(200), dollar(20000))?;
		T::Currency::update_balance(currency_id, &trader, dollar(10000).unique_saturated_into())?;
	}: swap_currency(RawOrigin::Signed(trader), currency_id, dollar(100), another_currency_id, 0.unique_saturated_into())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn add_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity::<Runtime>());
		});
	}

	#[test]
	fn withdraw_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_withdraw_liquidity::<Runtime>());
		});
	}

	#[test]
	fn swap_other_to_base() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_swap_other_to_base::<Runtime>());
		});
	}

	#[test]
	fn swap_base_to_other() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_swap_base_to_other::<Runtime>());
		});
	}

	#[test]
	fn swap_other_to_other() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_swap_other_to_other::<Runtime>());
		});
	}
}
