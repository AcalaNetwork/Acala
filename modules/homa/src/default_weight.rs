#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use super::RedeemStrategy;
use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

impl crate::WeightInfo for () {
	fn mint() -> Weight {
		(71_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn redeem(strategy: &RedeemStrategy) -> Weight {
		match strategy {
			RedeemStrategy::Immediately => (88_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(6 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			RedeemStrategy::Target(_) => (75_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(7 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			RedeemStrategy::WaitForUnbonding => (47_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(4 as Weight))
				.saturating_add(DbWeight::get().writes(4 as Weight)),
		}
	}
	fn withdraw_redemption() -> Weight {
		(53_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(6 as Weight))
			.saturating_add(DbWeight::get().writes(2 as Weight))
	}
}
