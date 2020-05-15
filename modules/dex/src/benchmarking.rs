//! DEX module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use sp_runtime::traits::Bounded;

use crate::Module as Dex;

const SEED: u32 = 0;

benchmarks! {
	_ {}

	set_liquidity_incentive_rate {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, CurrencyId::DOT, Rate::from_rational(1, 10000000))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn test_benchmarks() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_liquidity_incentive_rate::<Runtime>());
		});
	}
}
