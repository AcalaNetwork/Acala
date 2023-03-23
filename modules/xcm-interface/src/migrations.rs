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

// This file is used for migration MultiLocation and XcmWeight storage
use crate::*;
use frame_support::{log, migration::storage_key_iter, traits::OnRuntimeUpgrade, StoragePrefixedMap};
use sp_std::marker::PhantomData;
pub use xcm::v2::{MultiLocation as OldMultiLocation, Weight as OldXcmWeight};

#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, TypeInfo)]
pub enum OldXcmInterfaceOperation {
	// XTokens
	XtokensTransfer,
	// Homa
	HomaWithdrawUnbonded,
	HomaBondExtra,
	HomaUnbond,
	// Parachain fee with location info
	ParachainFee(Box<OldMultiLocation>),
}

impl TryInto<XcmInterfaceOperation> for OldXcmInterfaceOperation {
	type Error = ();
	fn try_into(self) -> sp_std::result::Result<XcmInterfaceOperation, Self::Error> {
		let data = match self {
			OldXcmInterfaceOperation::XtokensTransfer => XcmInterfaceOperation::XtokensTransfer,
			OldXcmInterfaceOperation::HomaWithdrawUnbonded => XcmInterfaceOperation::HomaWithdrawUnbonded,
			OldXcmInterfaceOperation::HomaBondExtra => XcmInterfaceOperation::HomaBondExtra,
			OldXcmInterfaceOperation::HomaUnbond => XcmInterfaceOperation::HomaUnbond,
			OldXcmInterfaceOperation::ParachainFee(old_multilocation) => {
				let v3_multilocation: MultiLocation =
					(*old_multilocation).try_into().expect("Stored xcm::v2::MultiLocation");
				XcmInterfaceOperation::ParachainFee(Box::new(v3_multilocation))
			}
		};
		Ok(data)
	}
}

/// Migrate both key type and value type of XcmDestWeightAndFee.
pub struct MigrateXcmDestWeightAndFee<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateXcmDestWeightAndFee<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "xcm-interface",
			"MigrateXcmDestWeightAndFee::on_runtime_upgrade execute, will migrate the OldMultiLocation to v3 MultiLocation in
			XcmInterfaceOperation::ParachainFee(Box<OldMultiLocation>) key type, and migrate OldXcmWeight to v3 XcmWeight in the value tuple.",
		);

		let mut weight: Weight = Weight::zero();

		let module_prefix = XcmDestWeightAndFee::<T>::module_prefix();
		let storage_prefix = XcmDestWeightAndFee::<T>::storage_prefix();
		let old_data = storage_key_iter::<OldXcmInterfaceOperation, (OldXcmWeight, Balance), Twox64Concat>(
			module_prefix,
			storage_prefix,
		)
		.drain()
		.collect::<sp_std::vec::Vec<_>>();
		for (old_key, old_value) in old_data {
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
			let new_key: XcmInterfaceOperation = old_key.try_into().expect("Stored xcm::v2::MultiLocation");
			let new_value: (XcmWeight, Balance) = (XcmWeight::from_ref_time(old_value.0), old_value.1);
			XcmDestWeightAndFee::<T>::insert(new_key, new_value);
		}

		weight
	}
}
