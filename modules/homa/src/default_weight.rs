#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use super::RedeemStrategy;
use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn mint() -> Weight {
		(95_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn redeem(strategy: &RedeemStrategy) -> Weight {
		match strategy {
			RedeemStrategy::Immediately => (108_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(9 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			RedeemStrategy::Target(_) => (83_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(10 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			RedeemStrategy::WaitForUnbonding => (59_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(8 as Weight))
				.saturating_add(DbWeight::get().writes(4 as Weight)),
		}
	}
	fn withdraw_redemption() -> Weight {
		(65_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(4 as Weight))
	}
}
