//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Trait> module_accounts::WeightInfo for WeightInfo<T> {
	fn close_account(c: u32) -> Weight {
		(353_067_000 as Weight)
			.saturating_add((93_982_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().reads((1 as Weight).saturating_mul(c as Weight)))
			.saturating_add(DbWeight::get().writes(1 as Weight))
			.saturating_add(DbWeight::get().writes((2 as Weight).saturating_mul(c as Weight)))
	}
	fn on_finalize() -> Weight {
		(39_708_000 as Weight)
			.saturating_add(DbWeight::get().reads(2 as Weight))
			.saturating_add(DbWeight::get().writes(1 as Weight))
	}
}
