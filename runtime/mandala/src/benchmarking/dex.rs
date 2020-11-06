use crate::{AccountId, Balance, Currencies, CurrencyId, Dex, EnabledTradingPairs, Runtime, TradingPathLimit};

use super::utils::dollars;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;

const SEED: u32 = 0;

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

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		deposit,
	)?;

	Ok(())
}

runtime_benchmarks! {
	{ Runtime, module_dex }

	_ {}

	// add liquidity but don't staking lp
	add_liquidity {
		let first_maker: AccountId = account("first_maker", 0, SEED);
		let second_maker: AccountId = account("second_maker", 0, SEED);
		let trading_pair = EnabledTradingPairs::get()[0];
		let amount_a = dollars(100u32);
		let amount_b = dollars(10000u32);

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.0, &second_maker, amount_a.unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.1, &second_maker, amount_b.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity(first_maker.clone(), trading_pair.0, trading_pair.1, amount_a, amount_b, false)?;
	}: add_liquidity(RawOrigin::Signed(second_maker), trading_pair.0, trading_pair.1, amount_a, amount_b, false)

	// worst: add liquidity and stake lp
	add_liquidity_and_deposit {
		let first_maker: AccountId = account("first_maker", 0, SEED);
		let second_maker: AccountId = account("second_maker", 0, SEED);
		let trading_pair = EnabledTradingPairs::get()[0];
		let amount_a = dollars(100u32);
		let amount_b = dollars(10000u32);

		// set balance
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.0, &second_maker, amount_a.unique_saturated_into())?;
		<Currencies as MultiCurrencyExtended<_>>::update_balance(trading_pair.1, &second_maker, amount_b.unique_saturated_into())?;

		// first maker inject liquidity
		inject_liquidity(first_maker.clone(), trading_pair.0, trading_pair.1, amount_a, amount_b, true)?;
	}: add_liquidity(RawOrigin::Signed(second_maker), trading_pair.0, trading_pair.1, amount_a, amount_b, true)

	// remove liquidity by liquid lp share
	remove_liquidity {
		let maker: AccountId = account("maker", 0, SEED);
		let trading_pair = EnabledTradingPairs::get()[0];
		inject_liquidity(maker.clone(), trading_pair.0, trading_pair.1, dollars(100u32), dollars(10000u32), false)?;
	}: remove_liquidity(RawOrigin::Signed(maker), trading_pair.0, trading_pair.1, dollars(50u32).unique_saturated_into(), false)

	// remove liquidity by withdraw staking lp share
	remove_liquidity_by_withdraw {
		let maker: AccountId = account("maker", 0, SEED);
		let trading_pair = EnabledTradingPairs::get()[0];
		inject_liquidity(maker.clone(), trading_pair.0, trading_pair.1, dollars(100u32), dollars(10000u32), true)?;
	}: remove_liquidity(RawOrigin::Signed(maker), trading_pair.0, trading_pair.1, dollars(50u32).unique_saturated_into(), true)

	swap_with_exact_supply {
		let u in 2 .. TradingPathLimit::get() as u32;

		let trading_pair = EnabledTradingPairs::get()[0];
		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				path.push(trading_pair.0);
				path.push(trading_pair.1);
			} else {
				if i % 2 == 0 {
					path.push(trading_pair.0);
				} else {
					path.push(trading_pair.1);
				}
			}
		}

		let maker: AccountId = account("maker", 0, SEED);
		let taker: AccountId = account("taker", 0, SEED);
		inject_liquidity(maker, trading_pair.0, trading_pair.1, dollars(10000u32), dollars(10000u32), false)?;

		<Currencies as MultiCurrencyExtended<_>>::update_balance(path[0], &taker, dollars(10000u32).unique_saturated_into())?;
	}: swap_with_exact_supply(RawOrigin::Signed(taker), path, dollars(100u32), 0)

	swap_with_exact_target {
		let u in 2 .. TradingPathLimit::get() as u32;

		let trading_pair = EnabledTradingPairs::get()[0];
		let mut path: Vec<CurrencyId> = vec![];
		for i in 1 .. u {
			if i == 1 {
				path.push(trading_pair.0);
				path.push(trading_pair.1);
			} else {
				if i % 2 == 0 {
					path.push(trading_pair.0);
				} else {
					path.push(trading_pair.1);
				}
			}
		}

		let maker: AccountId = account("maker", 0, SEED);
		let taker: AccountId = account("taker", 0, SEED);
		inject_liquidity(maker, trading_pair.0, trading_pair.1, dollars(10000u32), dollars(10000u32), false)?;

		<Currencies as MultiCurrencyExtended<_>>::update_balance(path[0], &taker, dollars(10000u32).unique_saturated_into())?;
	}: swap_with_exact_target(RawOrigin::Signed(taker), path, dollars(10u32), dollars(100u32))
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_add_liquidity() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity());
		});
	}

	#[test]
	fn test_add_liquidity_and_deposit() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_add_liquidity_and_deposit());
		});
	}

	#[test]
	fn test_remove_liquidity() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_remove_liquidity());
		});
	}

	#[test]
	fn test_remove_liquidity_by_withdraw() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_remove_liquidity_by_withdraw());
		});
	}

	#[test]
	fn test_swap_with_exact_supply() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_swap_with_exact_supply());
		});
	}

	#[test]
	fn test_swap_with_exact_target() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_swap_with_exact_target());
		});
	}
}
