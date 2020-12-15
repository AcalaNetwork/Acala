use frame_support::weights::{constants::RocksDbWeight as DbWeight, Weight};

use sp_std::marker::PhantomData;

pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_homa::WeightInfo for WeightInfo<T> {
	fn mint() -> Weight {
		(95_000_000 as Weight)
			.saturating_add(DbWeight::get().reads(9 as Weight))
			.saturating_add(DbWeight::get().writes(6 as Weight))
	}
	fn redeem(strategy: &module_homa::RedeemStrategy) -> Weight {
		match strategy {
			module_homa::RedeemStrategy::Immediately => (108_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(9 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			module_homa::RedeemStrategy::Target(_) => (83_000_000 as Weight)
				.saturating_add(DbWeight::get().reads(10 as Weight))
				.saturating_add(DbWeight::get().writes(5 as Weight)),
			module_homa::RedeemStrategy::WaitForUnbonding => (59_000_000 as Weight)
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
