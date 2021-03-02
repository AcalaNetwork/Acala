use crate::{AcalaOracle, CollateralCurrencyIds, CurrencyId, Origin, Price, Prices, Runtime, DOT};

use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::FixedPointNumber;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_prices }

	_ {}

	lock_price {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;
	}: _(RawOrigin::Root, DOT)

	unlock_price {
		let currency_id: CurrencyId = CollateralCurrencyIds::get()[0];

		// feed price
		AcalaOracle::feed_values(RawOrigin::Root.into(), vec![(currency_id, Price::one())])?;
		Prices::lock_price(Origin::root(), DOT)?;
	}: _(RawOrigin::Root, DOT)
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
	fn test_lock_price() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_lock_price());
		});
	}

	#[test]
	fn test_unlock_price() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_unlock_price());
		});
	}
}
