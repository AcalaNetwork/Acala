//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn transfer_non_native_currency() -> Weight {
		(172_011_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn transfer_native_currency() -> Weight {
		(43_023_000 as Weight)
	}
	fn update_balance_non_native_currency() -> Weight {
		(137_440_000 as Weight)
			.saturating_add(DbWeight::get().reads(5 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn update_balance_native_currency_creating() -> Weight {
		(64_432_000 as Weight)
	}
	fn update_balance_native_currency_killing() -> Weight {
		(62_595_000 as Weight)
	}
}
