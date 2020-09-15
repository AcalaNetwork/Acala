use super::utils::{dollars, set_ausd_balance};
use crate::{AcalaDataProvider, AcalaOracle, AccountId, Runtime};

use frame_system::{self as frame_system, RawOrigin};
use sp_runtime::DispatchError;
use sp_std::prelude::*;

use frame_benchmarking::account;
use orml_benchmarking::runtime_benchmarks_instance;

const MAX_DOLLARS: u32 = 1000;

runtime_benchmarks_instance! {
	{ Runtime, orml_oracle, AcalaDataProvider }

	_ {
		let u in 1 .. MAX_DOLLARS => ();
	}

	feed_values {
		let u in ...;
	}: _(Origin::root(), vec![])
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
			assert_ok!(test_benchmark_test_feed_values());
		});
	}
}
