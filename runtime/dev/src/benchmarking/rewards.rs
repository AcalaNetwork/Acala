use crate::{Rewards, Runtime, System};

use sp_std::prelude::*;

use frame_support::traits::OnInitialize;

use orml_benchmarking::runtime_benchmarks;

const MAX_BLOCK_NUMBER: u32 = 1000;

runtime_benchmarks! {
	{ Runtime, orml_rewards }

	_ {
		let u in 1 .. MAX_BLOCK_NUMBER => ();
	}

	on_initialize {
		let u in ...;

		System::set_block_number(u);
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
