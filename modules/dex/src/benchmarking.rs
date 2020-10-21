//! DEX module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Module as Dex;
use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::vec;

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn inject_liquidity<T: Trait>(
	maker: T::AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	// set balance
	T::Currency::update_balance(currency_id_a, &maker, max_amount_a.unique_saturated_into())?;
	T::Currency::update_balance(currency_id_b, &maker, max_amount_b.unique_saturated_into())?;

	Dex::<T>::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
	)?;

	Ok(())
}

benchmarks! {
	_ {}

	// `add_liquidity`, worst case:
	// already have other makers
	add_liquidity {
		let first_maker: T::AccountId = account("first_maker", 0, SEED);
		let second_maker: T::AccountId = account("second_maker", 0, SEED);
		let trading_pair = T::EnabledTradingPairs::get()[0];
		let amount_a = dollar(100);
		let amount_b = dollar(10000);

		// set balance
		T::Currency::update_balance(trading_pair.0, &second_maker, amount_a.unique_saturated_into())?;
		T::Currency::update_balance(trading_pair.1, &second_maker, amount_b.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity::<T>(first_maker.clone(), trading_pair.0, trading_pair.1, dollar(100), dollar(10000))?;
	}: add_liquidity(RawOrigin::Signed(second_maker), trading_pair.0, trading_pair.1, amount_a, amount_b)

	remove_liquidity {
		let maker: T::AccountId = account("maker", 0, SEED);
		let trading_pair = T::EnabledTradingPairs::get()[0];
		inject_liquidity::<T>(maker.clone(), trading_pair.0, trading_pair.1, dollar(100), dollar(10000))?;
	}: remove_liquidity(RawOrigin::Signed(maker), trading_pair.0, trading_pair.1, dollar(50).unique_saturated_into())
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
	fn remove_liquidity() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_remove_liquidity::<Runtime>());
		});
	}
}
