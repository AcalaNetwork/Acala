//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 2.0.0

#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_incentives::WeightInfo for WeightInfo<T> {
	fn deposit_dex_share() -> Weight {
		(219_025_000 as Weight)
			.saturating_add(DbWeight::get().reads(7 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn withdraw_dex_share() -> Weight {
		(373_854_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn claim_rewards() -> Weight {
		(74_998_000 as Weight)
			.saturating_add(DbWeight::get().reads(3 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
	fn update_loans_incentive_rewards(c: u32) -> Weight {
		(5_081_000 as Weight)
			.saturating_add((5_495_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
	}
	fn update_dex_incentive_rewards(c: u32) -> Weight {
		(4_846_000 as Weight)
			.saturating_add((4_851_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
	}
	fn update_homa_incentive_reward() -> Weight {
		(5_934_000 as Weight).saturating_add(DbWeight::get().writes(1 as Weight))
	}
	fn update_dex_saving_rates(c: u32) -> Weight {
		(3_896_000 as Weight)
			.saturating_add((5_340_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(DbWeight::get().writes((1 as Weight).saturating_mul(c as Weight)))
	}
}
