use frame_support::weights::Weight;

pub mod v1 {
	use super::*;
	use crate::*;

	/// Migrate the entire storage of previously named "module-homa-xcm" pallet to here.
	pub fn migrate<T: frame_system::Config>() -> Weight {
		let old_prefix = "HomaXcm";
		let new_prefix = "XcmInterface";

		log::info!(
			target: "runtime::xcm-interface",
			"Running migration from HomaXcm to XcmInterface. Old prefix: {:?}, New prefix: {:?}",
			old_prefix, new_prefix,
		);

		frame_support::storage::migration::move_pallet(old_prefix.as_bytes(), new_prefix.as_bytes());

		log::info!(
			target: "runtime::xcm-interface",
			"Storage migrated from HomaXcm to XcmInterface.",
		);

		<T as frame_system::Config>::BlockWeights::get().max_block
	}
}
