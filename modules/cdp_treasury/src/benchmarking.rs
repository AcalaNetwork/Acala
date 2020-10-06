//! DEX module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::benchmarks;
use frame_system::RawOrigin;
use primitives::TokenSymbol;
use sp_std::prelude::*;

pub fn dollar(d: u32) -> Balance {
	let d: Balance = d.into();
	d.saturating_mul(1_000_000_000_000_000_000)
}

benchmarks! {
	_ {}

	set_collateral_auction_maximum_size {
		let u in 0 .. 1000;
	}: _(RawOrigin::Root, CurrencyId::Token(TokenSymbol::DOT), dollar(100))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::{ExtBuilder, Runtime};
	use frame_support::assert_ok;

	#[test]
	fn set_collateral_auction_maximum_size() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(test_benchmark_set_collateral_auction_maximum_size::<Runtime>());
		});
	}
}
