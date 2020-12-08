//! Benchmarks for the emergency shutdown module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::RawOrigin;
use sp_runtime::{traits::UniqueSaturatedInto, FixedPointNumber};

use emergency_shutdown::Module as EmergencyShutdown;
use emergency_shutdown::*;
use orml_traits::{DataFeeder, MultiCurrencyExtended};
use primitives::{Balance, CurrencyId};
use support::{CDPTreasury, Price};

pub struct Module<T: Config>(emergency_shutdown::Module<T>);

pub trait Config:
	emergency_shutdown::Config + orml_oracle::Config<orml_oracle::Instance1> + prices::Config + loans::Config
{
}

const SEED: u32 = 0;

fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

fn feed_price<T: Config>(currency_id: CurrencyId, price: Price) -> Result<(), &'static str> {
	let oracle_operators = orml_oracle::Module::<T, orml_oracle::Instance1>::members().0;
	for operator in oracle_operators {
		<T as prices::Config>::Source::feed_value(operator.clone(), currency_id, price)?;
	}
	Ok(())
}

benchmarks! {
	_ { }

	call_emergency_shutdown {
		let u in 0 .. 1000;

		let currency_id = <T as emergency_shutdown::Config>::CollateralCurrencyIds::get()[0];
		feed_price::<T>(currency_id, Price::one())?;
	}: emergency_shutdown(RawOrigin::Root)

	open_collateral_refund {
		let u in 0 .. 1000;

		EmergencyShutdown::<T>::emergency_shutdown(RawOrigin::Root.into())?;
	}: _(RawOrigin::Root)

	refund_collaterals {
		let u in 0 .. 1000;

		let funder: T::AccountId = account("funder", u, SEED);
		let caller: T::AccountId = account("caller", u, SEED);
		let currency_ids = <T as emergency_shutdown::Config>::CollateralCurrencyIds::get();
		for currency_id in currency_ids {
			<T as loans::Config>::Currency::update_balance(currency_id, &funder, dollar(100).unique_saturated_into())?;
			<T as emergency_shutdown::Config>::CDPTreasury::deposit_collateral(&funder, currency_id, dollar(100))?;
		}
		<T as emergency_shutdown::Config>::CDPTreasury::issue_debit(&caller, dollar(1000), true)?;
		<T as emergency_shutdown::Config>::CDPTreasury::issue_debit(&funder, dollar(9000), true)?;

		EmergencyShutdown::<T>::emergency_shutdown(RawOrigin::Root.into())?;
		EmergencyShutdown::<T>::open_collateral_refund(RawOrigin::Root.into())?;
	}: _(RawOrigin::Signed(caller),  dollar(1000))
}

#[cfg(feature = "runtime-benchmarks")]
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
