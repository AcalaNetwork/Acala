//! Benchmarks for the emergency shutdown module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::traits::UniqueSaturatedInto;

use emergency_shutdown::Module as EmergencyShutdown;
use emergency_shutdown::*;
use orml_oracle::OperatorProvider;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::{Balance, CurrencyId};
use support::{CDPTreasury, Price};

pub struct Module<T: Trait>(emergency_shutdown::Module<T>);

pub trait Trait: emergency_shutdown::Trait + orml_oracle::Trait + prices::Trait + loans::Trait {}

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

	call_emergency_shutdown {
		let u in 0 .. 1000;

		let currency_id = <T as emergency_shutdown::Trait>::CollateralCurrencyIds::get()[0];
		feed_price::<T>(currency_id, Price::from_natural(1))?;
	}: emergency_shutdown(RawOrigin::Root)

	open_collateral_refund {
		let u in 0 .. 1000;

		EmergencyShutdown::<T>::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::Root)

	refund_collaterals {
		let u in 0 .. 1000;

		let funder: T::AccountId = account("funder", u, SEED);
		let caller: T::AccountId = account("caller", u, SEED);
		let currency_ids = <T as emergency_shutdown::Trait>::CollateralCurrencyIds::get();
		for currency_id in currency_ids {
			<T as loans::Trait>::Currency::update_balance(currency_id, &funder, dollar(100).unique_saturated_into())?;
			<T as emergency_shutdown::Trait>::CDPTreasury::transfer_collateral_from(currency_id, &funder, dollar(100))?;
		}
		<T as emergency_shutdown::Trait>::CDPTreasury::deposit_backed_debit_to(&caller, dollar(1000))?;
		<T as emergency_shutdown::Trait>::CDPTreasury::deposit_backed_debit_to(&funder, dollar(9000))?;

		EmergencyShutdown::<T>::emergency_shutdown(RawOrigin::Root.into())?;
		EmergencyShutdown::<T>::open_collateral_refund(RawOrigin::Root.into())?;
	}: _(RawOrigin::Signed(caller),  dollar(1000))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn call_emergency_shutdown() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_call_emergency_shutdown::<Runtime>());
		});
	}

	#[test]
	fn open_collateral_refund() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_open_collateral_refund::<Runtime>());
		});
	}

	#[test]
	fn refund_collaterals() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_refund_collaterals::<Runtime>());
		});
	}
}
