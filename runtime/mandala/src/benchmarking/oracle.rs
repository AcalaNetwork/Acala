use crate::{AcalaDataProvider, AcalaOracle, CollateralCurrencyIds, FixedPointNumber, Origin, Price, Runtime, System};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks_instance;
use sp_std::prelude::*;

runtime_benchmarks_instance! {
	{ Runtime, orml_oracle, AcalaDataProvider }

	_ {}

	// feed values
	feed_values {
		let c in 0 .. CollateralCurrencyIds::get().len().saturating_sub(1) as u32;
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for i in 0 .. c {
			values.push((currency_ids[i as usize], Price::one()));
		}
	}: _(Origin::root(), values)

	on_finalize {
		let currency_ids = CollateralCurrencyIds::get();
		let mut values = vec![];

		for currency_id in currency_ids {
			values.push((currency_id, Price::one()));
		}
		System::set_block_number(1);
		AcalaOracle::feed_values(Origin::root(), values)?;
	}: {
		AcalaOracle::on_finalize(System::block_number());
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
	fn test_feed_values() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_feed_values());
		});
	}

	#[test]
	fn test_on_finalize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize());
		});
	}
}
