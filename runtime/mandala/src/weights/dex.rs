//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Trait> module_dex::WeightInfo for WeightInfo<T> {
	fn add_liquidity(deposit: bool) -> Weight {
		if deposit {
			(127_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(16 as Weight))
				.saturating_add(DbWeight::get().writes(12 as Weight))
		} else {
			(82_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(10 as Weight))
				.saturating_add(DbWeight::get().writes(7 as Weight))
		}
	}
	fn remove_liquidity(by_withdraw: bool) -> Weight {
		if by_withdraw {
			(139_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(14 as Weight))
				.saturating_add(DbWeight::get().writes(12 as Weight))
		} else {
			(83_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(9 as Weight))
				.saturating_add(DbWeight::get().writes(7 as Weight))
		}
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
