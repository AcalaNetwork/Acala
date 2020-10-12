use crate::{
	AcalaOracle, AccountId, Amount, CdpEngine, CollateralCurrencyIds, CurrencyId, ExchangeRate, Honzon,
	MinimumDebitValue, Price, Rate, Ratio, Runtime, TokenSymbol,
};

use super::utils::set_balance;
use core::convert::TryInto;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::Change;
use sp_runtime::{
	traits::{Saturating, UniqueSaturatedInto},
	FixedPointNumber,
};
use sp_std::prelude::*;

const SEED: u32 = 0;

runtime_benchmarks! {
	{ Runtime, module_honzon }

	_ {}

	authorize {
		let caller: AccountId = account("caller", 0, SEED);
		let to: AccountId = account("to", 0, SEED);
	}: _(RawOrigin::Signed(caller), CurrencyId::Token(TokenSymbol::DOT), to)

	unauthorize {
		let caller: AccountId = account("caller", 0, SEED);
		let to: AccountId = account("to", 0, SEED);
		Honzon::authorize(
			RawOrigin::Signed(caller.clone()).into(),
			CurrencyId::Token(TokenSymbol::DOT),
			to.clone()
		)?;
	}: _(RawOrigin::Signed(caller), CurrencyId::Token(TokenSymbol::DOT), to)

	unauthorize_all {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;

		let caller: AccountId = account("caller", 0, SEED);
		let currency_ids = CollateralCurrencyIds::get();
		let to: AccountId = account("to", 0, SEED);

		for i in 0 .. c {
			Honzon::authorize(
				RawOrigin::Signed(caller.clone()).into(),
				currency_ids[i as usize],
				to.clone(),
			)?;
		}
	}: _(RawOrigin::Signed(caller))

	// `adjust_loan`, best case:
	// adjust both collateral and debit
	adjust_loan {
		let caller: AccountId = account("caller", 0, SEED);
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let collateral_price = Price::one();		// 1 USD
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_add(ExchangeRate::from_inner(1)).saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10;
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		set_balance(currency_id, &caller, collateral_amount);

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, collateral_price)])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
		)?;
	}: _(RawOrigin::Signed(caller), currency_id, collateral_amount.try_into().unwrap(), debit_amount)

	transfer_loan_from {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];
		let sender: AccountId = account("sender", 0, SEED);
		let receiver: AccountId = account("receiver", 0, SEED);
		let min_debit_value = MinimumDebitValue::get();
		let debit_exchange_rate = CdpEngine::get_debit_exchange_rate(currency_id);
		let min_debit_amount = debit_exchange_rate.reciprocal().unwrap().saturating_add(ExchangeRate::from_inner(1)).saturating_mul_int(min_debit_value);
		let min_debit_amount: Amount = min_debit_amount.unique_saturated_into();
		let debit_amount = min_debit_amount * 10;
		let collateral_amount = (min_debit_value * 10 * 2).unique_saturated_into();

		// set balance
		set_balance(currency_id, &sender, collateral_amount);

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;

		// set risk params
		CdpEngine::set_collateral_params(
			RawOrigin::Root.into(),
			currency_id,
			Change::NoChange,
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(Some(Rate::saturating_from_rational(10, 100))),
			Change::NewValue(Some(Ratio::saturating_from_rational(150, 100))),
			Change::NewValue(min_debit_value * 100),
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
			receiver.clone()
		)?;
	}: _(RawOrigin::Signed(receiver), currency_id, sender)
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
	fn test_authorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_authorize());
		});
	}

	#[test]
	fn test_unauthorize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize());
		});
	}

	#[test]
	fn test_unauthorize_all() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unauthorize_all());
		});
	}

	#[test]
	fn test_adjust_loan() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_adjust_loan());
		});
	}
}
