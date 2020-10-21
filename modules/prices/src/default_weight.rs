//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn lock_price() -> Weight {
		(228_103_000 as Weight)
			.saturating_add(DbWeight::get().reads(11 as Weight))
			.saturating_add(DbWeight::get().writes(3 as Weight))
	}
	fn unlock_price() -> Weight {
		(48_900_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
