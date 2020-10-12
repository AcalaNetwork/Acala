use crate::{AccumulatePeriod, CollateralCurrencyIds, Rewards, Runtime, System};

use frame_support::storage::StorageMap;
use frame_support::traits::OnInitialize;
use module_incentives::PoolId;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, orml_rewards }

	_ {}

	on_initialize {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let block_number = AccumulatePeriod::get();

		for i in 0 .. c {
			let currency_id = currency_ids[i as usize];
			let pool_id = PoolId::Loans(currency_id);

			orml_rewards::Pools::<Runtime>::mutate(pool_id, |pool_info| {
				pool_info.total_rewards += 100;
			});
		}

		Rewards::on_initialize(1);
		System::set_block_number(block_number);
	}: {
		Rewards::on_initialize(System::block_number());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::assert_ok;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap()
			.into()
	}

	#[test]
	fn test_on_initialize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_initialize());
		});
	}
}
