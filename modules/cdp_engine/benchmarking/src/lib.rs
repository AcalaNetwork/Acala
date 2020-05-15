//! Benchmarks for the honzon module.
// This is separated into its own crate due to cyclic dependency issues.

#![cfg_attr(not(feature = "std"), no_std)]

mod mock;

use sp_std::prelude::*;
use sp_std::vec;

use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use frame_system::{self as system, RawOrigin};
use sp_runtime::traits::UniqueSaturatedInto;

use cdp_engine::Module as CdpEngine;
use cdp_engine::*;
use orml_oracle::OperatorProvider;
use orml_traits::{DataProviderExtended, MultiCurrencyExtended};
use primitives::{Amount, Balance, CurrencyId};
use support::{Price, Rate, Ratio};

pub struct Module<T: Trait>(cdp_engine::Module<T>);

pub trait Trait: cdp_engine::Trait + orml_oracle::Trait + prices::Trait {}

const SEED: u32 = 0;

pub fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
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
}

// pub fn liquidate(
// 	origin,
// 	currency_id: CurrencyId,
// 	who: T::AccountId,
// ) {

// 	pub fn settle(
// 		origin,
// 		currency_id: CurrencyId,
// 		who: T::AccountId,
// 	) {

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{new_test_ext, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_set_collateral_params::<Runtime>());
			assert_ok!(test_benchmark_set_global_params::<Runtime>());
		});
	}
}
