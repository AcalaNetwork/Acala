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

use crate::{Config, CurrencyId, ForeignAssetLocations, LocationToCurrencyIds, Weight};
use frame_support::{migration::storage_key_iter, pallet_prelude::*, traits::OnRuntimeUpgrade, StoragePrefixedMap};
use sp_std::marker::PhantomData;
use xcm::v3::prelude::*;

/// Migrate MultiLocation v2 to v3
pub struct MigrateV1MultiLocationToV3<T>(PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for MigrateV1MultiLocationToV3<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "asset-registry",
			"MigrateV1MultiLocationToV3::on_runtime_upgrade execute, will migrate the key type of LocationToCurrencyIds and value type
			of ForeignAssetLocations from old MultiLocation(v1/v2) to v3",
		);

		let mut weight: Weight = Weight::zero();

		// migrate the value type of ForeignAssetLocations
		ForeignAssetLocations::<T>::translate(|_key, old_value: xcm::v2::MultiLocation| {
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
			MultiLocation::try_from(old_value).ok()
		});

		// migrate the key type of LocationToCurrencyIds
		let module_prefix = LocationToCurrencyIds::<T>::module_prefix();
		let storage_prefix = LocationToCurrencyIds::<T>::storage_prefix();
		let old_data =
			storage_key_iter::<xcm::v2::MultiLocation, CurrencyId, Twox64Concat>(module_prefix, storage_prefix)
				.drain()
				.collect::<sp_std::vec::Vec<_>>();
		for (old_key, value) in old_data {
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));
			let new_key: MultiLocation = old_key.try_into().expect("Stored xcm::v2::MultiLocation");
			LocationToCurrencyIds::<T>::insert(new_key, value);
		}

		weight
	}
}
