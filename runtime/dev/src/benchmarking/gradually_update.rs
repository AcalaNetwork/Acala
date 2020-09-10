use crate::{GraduallyUpdate, Origin, Runtime, System};

use sp_std::prelude::*;

use frame_system::RawOrigin;

use orml_benchmarking::runtime_benchmarks;

const MAX_USER_INDEX: u32 = 1000;
const MAX_DOLLARS: u32 = 100;

runtime_benchmarks! {
	{ Runtime, orml_gradually_update }

	_ {
		let u in 1 .. MAX_USER_INDEX => ();
		let d in 1 .. MAX_DOLLARS => ();
	}

	gradually_update {
		let u in ...;
		let d in ...;

		System::set_block_number(1);

		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1],
			target_value: vec![9],
			per_block: vec![1],
		};
	}: _(RawOrigin::Root, update.clone())

	cancel_gradually_update {
		let u in ...;
		let d in ...;

		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1],
			target_value: vec![9],
			per_block: vec![1],
		};
		let _ = GraduallyUpdate::gradually_update(Origin::root(), update.clone());
	}: _(Origin::root(), update.key.clone())
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
	fn test_dispatch_as() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_gradually_update());
		});
	}

	#[test]
	fn test_scheduled_dispatch() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_gradually_update());
		});
	}
}
