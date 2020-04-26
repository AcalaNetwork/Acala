//! honzon module benchmarking.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::Get;
use system::RawOrigin;

const SEED: u32 = 0;

benchmarks! {
	_ {
		// User account seed
		let u in 0 .. 1000 => ();
	}

	unauthorize_all {
		let caller: T::AccountId = account("caller", u, SEED);
		// let who: T::AccountId = account("who", u, SEED);
		// let currency_id = <T as cdp_engine::Trait>::CollateralCurrencyIds::get()[0];
	}: _(RawOrigin::Signed(caller))
}
