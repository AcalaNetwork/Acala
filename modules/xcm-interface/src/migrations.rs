// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

// This file is used for initial migration from HomaXcm into XcmInterface, due to name change.
use frame_support::{
	traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
	weights::Weight,
};

pub mod v1 {
	use super::*;
	use crate::*;

	/// Migrate the entire storage of previously named "module-homa-xcm" pallet to here.
	pub fn migrate<T: frame_system::Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
		let old_prefix = "HomaXcm";
		let new_prefix = "XcmInterface";

		let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();

		log::info!(
			target: "runtime::xcm-interface",
			"Running migration from HomaXcm to XcmInterface. \n
			Old prefix: {:?}, New prefix: {:?} \n
			Current version: {:?}, New version: 1",
			old_prefix, new_prefix, on_chain_storage_version,
		);

		if on_chain_storage_version < 1 {
			frame_support::storage::migration::move_pallet(old_prefix.as_bytes(), new_prefix.as_bytes());
			StorageVersion::new(1).put::<P>();
			log::info!(
				target: "runtime::xcm-interface",
				"Storage migrated from HomaXcm to XcmInterface.",
			);
			<T as frame_system::Config>::BlockWeights::get().max_block
		} else {
			log::warn!(
				target: "runtime::xcm-interface",
				"Attempted to apply migration to v1 but failed because storage version is {:?}",
				on_chain_storage_version,
			);
			0
		}
	}
}
