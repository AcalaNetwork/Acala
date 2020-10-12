use crate::{GraduallyUpdate, Origin, Runtime, System, UpdateFrequency};

use frame_support::traits::OnFinalize;
use orml_benchmarking::runtime_benchmarks;
use sp_std::prelude::*;

const MAX_TARGET_VALUE: u32 = 100;

runtime_benchmarks! {
	{ Runtime, orml_gradually_update }

	_ {
		let u in 2 .. MAX_TARGET_VALUE => ();
	}

	// gradually update numeric parameter
	gradually_update {
		System::set_block_number(1);
		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1],
			target_value: vec![10],
			per_block: vec![1],
		};
	}: _(Origin::root(), update)

	// cancel gradually update
	cancel_gradually_update {
		let update = orml_gradually_update::GraduallyUpdate {
			key: vec![1],
			target_value: vec![10],
			per_block: vec![1],
		};
		GraduallyUpdate::gradually_update(Origin::root(), update.clone())?;
	}: _(Origin::root(), update.key)

	// execute gradually update
	on_finalize {
		let u in ...;

		System::set_block_number(1);
		for i in 1..u {
			let update = orml_gradually_update::GraduallyUpdate {
				key: vec![1],
				target_value: vec![200],
				per_block: vec![i as u8],
			};
			GraduallyUpdate::gradually_update(Origin::root(), update)?;
		}

		System::set_block_number(1 + UpdateFrequency::get());
	}: {
		GraduallyUpdate::on_finalize(System::block_number());
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
	fn test_gradually_update() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_gradually_update());
		});
	}

	#[test]
	fn test_cancel_gradually_update() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_cancel_gradually_update());
		});
	}

	#[test]
	fn test_on_finalize() {
		new_test_ext().execute_with(|| {
			assert_ok!(test_benchmark_on_finalize());
		});
	}
}
