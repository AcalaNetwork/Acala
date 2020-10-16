//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn add_liquidity() -> Weight {
		(362_483_000 as Weight)
			.saturating_add(DbWeight::get().reads(11 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn remove_liquidity() -> Weight {
		(414_245_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
	fn swap_with_exact_supply() -> Weight {
		(409_297_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn swap_with_exact_target() -> Weight {
		(409_297_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
}
