// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

//! Unit tests for xcm interface module.

#![cfg(test)]

use super::*;
use crate::migrations::{MigrateXcmDestWeightAndFee, OldMultiLocation, OldXcmInterfaceOperation, OldXcmWeight};
use crate::mock::{ExtBuilder, Runtime};
use frame_support::{
	storage::migration::{get_storage_value, put_storage_value},
	traits::OnRuntimeUpgrade,
	StorageHasher, StoragePrefixedMap,
};

#[test]
fn simulate_migrate_xcm_dest_weight_and_fee() {
	ExtBuilder::default().build().execute_with(|| {
		let module_prefix = XcmDestWeightAndFee::<Runtime>::module_prefix();
		let storage_prefix = XcmDestWeightAndFee::<Runtime>::storage_prefix();

		let old_key_1: OldXcmInterfaceOperation = OldXcmInterfaceOperation::XtokensTransfer;
		let old_value_1: (OldXcmWeight, Balance) = (1_000_000_000, 200_000_000);
		let old_key_2: OldXcmInterfaceOperation = OldXcmInterfaceOperation::ParachainFee(Box::new(
			OldMultiLocation::new(1, xcm::v2::Junctions::X1(xcm::v2::Junction::Parachain(1000))),
		));
		let old_value_2: (OldXcmWeight, Balance) = (2_000_000_000, 500_000_000);
		let new_key_1: XcmInterfaceOperation = XcmInterfaceOperation::XtokensTransfer;
		let new_value_1: (XcmWeight, Balance) = (XcmWeight::from_parts(1_000_000_000, 1024 * 128), 200_000_000);
		let new_key_2: XcmInterfaceOperation =
			XcmInterfaceOperation::ParachainFee(Box::new(MultiLocation::new(1, X1(Parachain(1000)))));
		let new_value_2: (XcmWeight, Balance) = (XcmWeight::from_parts(2_000_000_000, 1024 * 128), 500_000_000);

		// put old raw storage
		put_storage_value(
			module_prefix,
			storage_prefix,
			&Twox64Concat::hash(&old_key_1.encode()),
			old_value_1,
		);
		put_storage_value(
			module_prefix,
			storage_prefix,
			&Twox64Concat::hash(&old_key_2.encode()),
			old_value_2,
		);
		assert_eq!(
			get_storage_value::<(OldXcmWeight, Balance)>(
				module_prefix,
				storage_prefix,
				&Twox64Concat::hash(&old_key_1.encode()),
			),
			Some(old_value_1)
		);
		assert_eq!(
			get_storage_value::<(OldXcmWeight, Balance)>(
				module_prefix,
				storage_prefix,
				&Twox64Concat::hash(&old_key_2.encode()),
			),
			Some(old_value_2)
		);

		// Run migration
		assert_eq!(
			MigrateXcmDestWeightAndFee::<Runtime>::on_runtime_upgrade(),
			<<Runtime as frame_system::Config>::DbWeight as Get<frame_support::weights::RuntimeDbWeight>>::get()
				.reads_writes(2, 2)
		);
		assert_eq!(
			get_storage_value::<(XcmWeight, Balance)>(
				module_prefix,
				storage_prefix,
				&Twox64Concat::hash(&new_key_1.encode()),
			),
			Some(new_value_1)
		);
		assert_eq!(
			get_storage_value::<(XcmWeight, Balance)>(
				module_prefix,
				storage_prefix,
				&Twox64Concat::hash(&new_key_2.encode()),
			),
			Some(new_value_2)
		);
	});
}

// TODO: other unit tests
