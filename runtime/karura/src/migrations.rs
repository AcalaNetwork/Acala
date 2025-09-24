// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

use super::*;
use frame_support::{pallet_prelude::StorageVersion, traits::GetStorageVersion, weights::Weight};

pub mod v1 {
	use super::*;
	use frame_support::{ensure, traits::OnRuntimeUpgrade};
	use module_support::{RuntimeParametersKey, RuntimeParametersValue};
	use orml_traits::parameters::AggregratedKeyValue;
	use parity_scale_codec::EncodeLike;
	use sp_std::vec;
	use xcm::prelude::Location;

	const LOG_TARGET: &str = "parameters::v1";

	/// Migration to V1
	pub struct ParametersMigrateToV1<T>(core::marker::PhantomData<T>);
	impl<T: orml_parameters::Config> OnRuntimeUpgrade for ParametersMigrateToV1<T>
	where
		T::AggregratedKeyValue:
			AggregratedKeyValue<AggregratedKey = RuntimeParametersKey, AggregratedValue = RuntimeParametersValue>,
		RuntimeParametersKey: EncodeLike<<T::AggregratedKeyValue as AggregratedKeyValue>::AggregratedKey>,
		RuntimeParametersValue: EncodeLike<<T::AggregratedKeyValue as AggregratedKeyValue>::AggregratedValue>,
	{
		fn on_runtime_upgrade() -> Weight {
			log::info!(target: LOG_TARGET, "Running on_runtime_upgrade()");

			let version = Parameters::on_chain_storage_version();
			if version == 0 {
				let key_value = RuntimeParameters::Xtokens(XtokensParameters::ReserveLocation(
					ReserveLocation,
					Some(Location::parent()),
				));

				let (key, value) = key_value.clone().into_parts();

				orml_parameters::Parameters::<T>::set(key, value);

				StorageVersion::new(1).put::<Parameters>();

				log::info!(target: LOG_TARGET, "Migrated on Parameters to v1");
				T::DbWeight::get().reads_writes(1, 2)
			} else {
				log::info!(target: LOG_TARGET, "Parameters need to be removed");
				T::DbWeight::get().reads(1)
			}
		}

		#[cfg(feature = "try-runtime")]
		fn pre_upgrade() -> Result<vec::Vec<u8>, sp_runtime::TryRuntimeError> {
			log::info!(target: LOG_TARGET, "Running pre_upgrade()");

			let version = Parameters::on_chain_storage_version();
			ensure!(version == 0, "parameters already migrated");

			Ok(Vec::new())
		}

		#[cfg(feature = "try-runtime")]
		fn post_upgrade(_state: vec::Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
			log::info!(target: LOG_TARGET, "Running post_upgrade()");

			let version = Parameters::on_chain_storage_version();
			ensure!(version == 1, "parameters migration failed");

			Ok(())
		}
	}
}
