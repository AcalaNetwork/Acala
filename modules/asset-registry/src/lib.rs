// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

//! # Asset Registry Module
//!
//! Allow to support foreign asset without runtime upgrade

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::pallet_prelude::*;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{Currency, EnsureOrigin},
	RuntimeDebug,
};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_std::convert::TryInto;
use xcm::{v1::MultiLocation, VersionedMultiLocation};

mod mock;
mod tests;
mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// Type alias for currency balance.
pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Currency type for withdraw and balance storage.
		type Currency: Currency<Self::AccountId>;

		/// Required origin for registering asset.
		type RegisterOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
	pub struct AssetMetadata<Balance> {
		pub name: Vec<u8>,
		pub symbol: Vec<u8>,
		pub decimals: u8,
		pub minimal_balance: Balance,
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given location could not be used (e.g. because it cannot be expressed in the
		/// desired version of XCM).
		BadLocation,
		/// AssetMetadata already existed
		AssetMetadataExisted,
		/// AssetMetadata not exists
		AssetMetadataNotExists,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Registered foreign asset. \[AssetMetadata\]
		RegisteredForeignAsset(AssetMetadata<BalanceOf<T>>),
		/// Updated foreign asset. \[AssetMetadata\]
		UpdatedForeignAsset(AssetMetadata<BalanceOf<T>>),
	}

	/// The storages for AssetMetadatas.
	///
	/// AssetMetadatas: map v1::MultiLocation => AssetMetadata
	#[pallet::storage]
	#[pallet::getter(fn asset_metadatas)]
	pub type AssetMetadatas<T: Config> =
		StorageMap<_, Twox64Concat, MultiLocation, AssetMetadata<BalanceOf<T>>, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1000)]
		pub fn register_foreign_asset(
			origin: OriginFor<T>,
			location: VersionedMultiLocation,
			metadata: AssetMetadata<BalanceOf<T>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;
			Self::do_register_foreign_asset(&location, &metadata)?;

			Self::deposit_event(Event::<T>::RegisteredForeignAsset(metadata));
			Ok(())
		}

		#[pallet::weight(1000)]
		pub fn update_foreign_asset(
			origin: OriginFor<T>,
			location: VersionedMultiLocation,
			metadata: AssetMetadata<BalanceOf<T>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;
			Self::do_update_foreign_asset(&location, &metadata)?;

			Self::deposit_event(Event::<T>::UpdatedForeignAsset(metadata));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn do_register_foreign_asset(
		location: &VersionedMultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		let location: MultiLocation = location.clone().try_into().map_err(|()| Error::<T>::BadLocation)?;

		AssetMetadatas::<T>::mutate(location, |maybe_asset_metadatas| -> DispatchResult {
			ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetMetadataExisted);

			*maybe_asset_metadatas = Some(metadata.clone());
			Ok(())
		})
	}

	fn do_update_foreign_asset(
		location: &VersionedMultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		let location: MultiLocation = location.clone().try_into().map_err(|()| Error::<T>::BadLocation)?;

		AssetMetadatas::<T>::mutate(location, |maybe_asset_metadatas| -> DispatchResult {
			ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetMetadataNotExists);

			*maybe_asset_metadatas = Some(metadata.clone());
			Ok(())
		})
	}
}
