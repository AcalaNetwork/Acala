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

use crate as homa;
use frame_support::{
	log,
	traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
	weights::Weight,
};
use primitives::Balance;
use sp_runtime::traits::Zero;

/// Puts correct value into storage for `TotalStakingBonded`
pub fn migrate<T: homa::Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
	let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
	log::info!(
		target: "runtime::homa",
		"Running migration to v1 for homa with storage version {:?}",
		on_chain_storage_version,
	);

	if on_chain_storage_version < 1 {
		let total_staking: Balance = homa::StakingLedgers::<T>::iter()
			.fold(Zero::zero(), |total_bonded, (_, ledger)| {
				total_bonded.saturating_add(ledger.bonded)
			});
		homa::TotalStakingBonded::<T>::set(total_staking);

		StorageVersion::new(1).put::<P>();
		<T as frame_system::Config>::BlockWeights::get().max_block
	} else {
		log::warn!(
			target: "runtime::homa",
			"Attempted to apply migration to v1 but failed because storage version is {:?}",
			on_chain_storage_version,
		);
		0
	}
}

/// Ensures version is correct
///
/// Panics if anything goes wrong
pub fn pre_migrate<P: GetStorageVersion>() {
	assert!(P::on_chain_storage_version() < 1);
}

/// Some checks after the migration
///
/// Panics if anything goes wrong
pub fn post_migrate<T: homa::Config, P: GetStorageVersion>() {
	assert!(homa::TotalStakingBonded::<T>::exists());
	assert_eq!(P::on_chain_storage_version(), 1);
}
