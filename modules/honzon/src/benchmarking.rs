//! honzon module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use crate::Module as Honzon;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::prelude::*;
use system::RawOrigin;

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

		let amount: AmountOf<T> = 100_000_000_000.unique_saturated_into();
		let amount: AmountOf<T> = amount * u.unique_saturated_into();

		<T as loans::Trait>::Currency::update_balance(currency_id, &caller, amount)?;
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
