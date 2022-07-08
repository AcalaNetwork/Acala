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

pub mod v1 {
	// use super::*;
	use crate::CollateralAuctionItem as V1CollateralAuctionItem;
	use crate::*;

	use frame_support::{
		traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion},
		weights::Weight,
	};
	use primitives::{Balance, CurrencyId};

	#[cfg_attr(feature = "std", derive(PartialEq, Eq))]
	#[derive(Encode, Decode, Clone, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct CollateralAuctionItem<AccountId, BlockNumber> {
		refund_recipient: AccountId,
		currency_id: CurrencyId,
		initial_amount: Balance,
		amount: Balance,
		target: Balance,
		start_time: BlockNumber,
	}

	pub struct AuctionConverter<T>(PhantomData<T>);
	impl<T: frame_system::Config> AuctionConverter<T> {
		fn convert_collateral_auction_item_v0_to_v1(
			auction: CollateralAuctionItem<T::AccountId, T::BlockNumber>,
		) -> V1CollateralAuctionItem<T::AccountId, T::BlockNumber> {
			let CollateralAuctionItem {
				refund_recipient,
				currency_id,
				initial_amount,
				amount,
				target,
				start_time,
			} = auction;

			V1CollateralAuctionItem {
				refund_recipient,
				currency_id,
				initial_amount,
				amount,
				base: target,
				penalty: Default::default(),
				start_time,
			}
		}
	}

	/// Migrate the entire storage of previously named "module-homa-xcm" pallet to here.
	pub fn migrate<T: frame_system::Config + crate::Config, P: GetStorageVersion + PalletInfoAccess>() -> Weight {
		let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();

		log::info!(
			target: "runtime::auction-manager",
			"Running storage migration for CollateralAuctionItem. \n
			Current version: {:?}, New version: V1",
			on_chain_storage_version,
		);

		if on_chain_storage_version < 1 {
			//frame_support::storage::migration::move_pallet(old_prefix.as_bytes(), new_prefix.as_bytes());
			// Take the previous version of CollateralAuction into the new version
			let pallet_prefix: &[u8] = b"AuctionManager";
			let storage_prefix: &[u8] = b"CollateralAuctions";

			// Convert V0 auction items into V1 items.
			let v1_auctions: Vec<_> = frame_support::storage::migration::storage_key_iter::<
				AuctionId,
				CollateralAuctionItem<T::AccountId, T::BlockNumber>,
				Twox64Concat,
			>(pallet_prefix, storage_prefix)
			.into_iter()
			.map(|(id, auction)| {
				(
					id,
					AuctionConverter::<T>::convert_collateral_auction_item_v0_to_v1(auction),
				)
			})
			.collect();
			let num_items_migrated: u64 = v1_auctions.len() as u64;

			// Remove the current storage
			frame_support::storage::migration::remove_storage_prefix(pallet_prefix, storage_prefix, &[]);

			// Write the newer version into storage
			for (id, auction) in v1_auctions {
				CollateralAuctions::<T>::insert(id, auction);
			}

			// Update storage version.
			StorageVersion::new(1).put::<P>();
			log::info!(
				target: "runtime::auction-manager",
				"Storage migrated completed.",
			);

			T::DbWeight::get().reads_writes(num_items_migrated, num_items_migrated)
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
