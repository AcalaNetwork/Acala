//! DEX module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;

pub fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

benchmarks! {
	_ {}

	set_debit_and_surplus_handle_params {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, Some(dollar(100)), Some(dollar(100)), Some(dollar(100)), Some(dollar(100)))

	set_collateral_auction_maximum_size {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, CurrencyId::DOT, dollar(100))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_debit_and_surplus_handle_params::<Runtime>());
			assert_ok!(test_benchmark_set_collateral_auction_maximum_size::<Runtime>());
		});
	}
}
