use crate::{Runtime, System, TransactionPayment};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, module_transaction_payment }

	_ {}

	on_finalize {
	}: {
		TransactionPayment::on_finalize(System::block_number());
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
	fn test_on_finalize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize());
		});
	}
}
