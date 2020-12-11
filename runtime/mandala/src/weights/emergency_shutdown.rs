//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_emergency_shutdown::WeightInfo for WeightInfo<T> {
	fn emergency_shutdown(c: u32) -> Weight {
		(564_107_000 as Weight)
			.saturating_add((32_606_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().reads(36 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
			.saturating_add(DbWeight::get().writes((3 as Weight).saturating_mul(c as Weight)))
	}
	fn open_collateral_refund() -> Weight {
		(157_252_000 as Weight)
			.saturating_add(DbWeight::get().reads(11 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn refund_collaterals(c: u32) -> Weight {
		(251_074_000 as Weight)
			.saturating_add((111_343_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().reads(7 as Weight))
			.saturating_add(DbWeight::get().reads((2 as Weight).saturating_mul(c as Weight)))
			.saturating_add(DbWeight::get().writes(2 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(c as Weight)))
	}
}
