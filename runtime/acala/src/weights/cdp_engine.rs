//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_cdp_engine::WeightInfo for WeightInfo<T> {
	fn set_collateral_params() -> Weight {
		(132_649_000 as Weight)
			.saturating_add(DbWeight::get().reads(1 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn set_global_params() -> Weight {
		(46_103_000 as Weight)
			.saturating_add((8_000 as Weight).saturating_mul(0 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn liquidate_by_auction() -> Weight {
		(843_630_000 as Weight)
			.saturating_add(DbWeight::get().reads(26 as Weight))
			.saturating_add(DbWeight::get().writes(15 as Weight))
	}
	fn liquidate_by_dex() -> Weight {
		(847_136_000 as Weight)
			.saturating_add(DbWeight::get().reads(26 as Weight))
			.saturating_add(DbWeight::get().writes(15 as Weight))
	}
	fn settle() -> Weight {
		(336_821_000 as Weight)
			.saturating_add(DbWeight::get().reads(11 as Weight))
			.saturating_add(DbWeight::get().writes(7 as Weight))
	}
}
