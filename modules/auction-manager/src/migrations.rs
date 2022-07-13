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

use super::*;
pub mod v1 {
	use super::*;
	use frame_support::traits::OnRuntimeUpgrade;

	use frame_support::{
		traits::{GetStorageVersion, StorageVersion},
		weights::Weight,
	};
	use primitives::{Balance, CurrencyId};

	#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct V0CollateralAuctionItem<AccountId, BlockNumber> {
		refund_recipient: AccountId,
		currency_id: CurrencyId,
		#[codec(compact)]
		initial_amount: Balance,
		#[codec(compact)]
		amount: Balance,
		#[codec(compact)]
		target: Balance,
		start_time: BlockNumber,
	}

	impl<AccountId, BlockNumber> V0CollateralAuctionItem<AccountId, BlockNumber> {
		fn migrate_to_v1(self) -> CollateralAuctionItem<AccountId, BlockNumber> {
			CollateralAuctionItem {
				refund_recipient: self.refund_recipient,
				currency_id: self.currency_id,
				initial_amount: self.initial_amount,
				amount: self.amount,
				base: self.target,
				penalty: Default::default(),
				start_time: self.start_time,
			}
		}
	}

	pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);
	impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
		fn on_runtime_upgrade() -> Weight {
			let on_chain_storage_version = Pallet::<T>::on_chain_storage_version();

			log::info!(
				target: "runtime::auction-manager",
				"Running storage migration for CollateralAuctionItem. \n
				Current version: {:?}, New version: V1",
				on_chain_storage_version,
			);

			if on_chain_storage_version < 1 {
				let mut auctions_migrated = 0u64;
				// Migrate CollateralAuctions storage from V0 to V1
				CollateralAuctions::<T>::translate::<V0CollateralAuctionItem<T::AccountId, T::BlockNumber>, _>(
					|_id, v0_auction| {
						auctions_migrated.saturating_inc();
						Some(v0_auction.migrate_to_v1())
					},
				);

				// Update storage version.
				StorageVersion::new(1).put::<Pallet<T>>();
				log::info!(
					target: "runtime::auction-manager",
					"Storage migrated completed.",
				);

				T::DbWeight::get().reads_writes(auctions_migrated, auctions_migrated)
			} else {
				log::warn!(
					target: "runtime::auction-manager",
					"Attempted to apply migration to v1 but failed because storage version is {:?}",
					on_chain_storage_version,
				);
				0
			}
		}
	}
}
