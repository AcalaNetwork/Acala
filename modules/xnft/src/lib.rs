// This file is part of Acala.

// Copyright (C) 2023 Unique Network.
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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use cumulus_primitives_core::ParaId;
use frame_support::{ensure, pallet_prelude::*};
use frame_system::pallet_prelude::*;
use module_nft::{ClassIdOf, TokenIdOf};
use sp_runtime::{traits::AccountIdConversion, DispatchResult};
use sp_std::boxed::Box;
use xcm::v3::{
	AssetId, AssetInstance, Error as XcmError, Fungibility, InteriorMultiLocation, Junction::*, MultiAsset,
	Result as XcmResult,
};
use xcm_executor::traits::{ConvertLocation, Error as XcmExecutorError, TransactAsset};

pub mod impl_transactor;
pub mod xcm_helpers;

pub use pallet::*;

pub type ConverterOf<T> = <T as Config>::LocationToAccountId;
pub type ModuleNftPallet<T> = module_nft::Pallet<T>;
pub type OrmlNftPallet<T> = orml_nft::Pallet<T>;

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use module_nft::WeightInfo as _;
	use primitives::nft::{ClassProperty, Properties};

	#[pallet::config]
	pub trait Config: frame_system::Config + module_nft::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type LocationToAccountId: ConvertLocation<Self::AccountId>;

		type SelfParaId: Get<ParaId>;

		type NtfPalletLocation: Get<InteriorMultiLocation>;

		type RegisterOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	/// Error for non-fungible-token module.
	#[pallet::error]
	pub enum Error<T> {
		AssetAlreadyRegistered,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		AssetRegistered {
			asset_id: Box<AssetId>,
			collection_id: ClassIdOf<T>,
		},
	}

	#[pallet::storage]
	#[pallet::getter(fn foreign_asset_to_class)]
	pub type ForeignAssetToClass<T: Config> = StorageMap<_, Twox64Concat, xcm::v3::AssetId, ClassIdOf<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn class_to_foreign_asset)]
	pub type ClassToForeignAsset<T: Config> = StorageMap<_, Twox64Concat, ClassIdOf<T>, xcm::v3::AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asset_instance_to_item)]
	pub type AssetInstanceToItem<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		ClassIdOf<T>,
		Twox64Concat,
		xcm::v3::AssetInstance,
		TokenIdOf<T>,
		OptionQuery,
	>;

	#[pallet::storage]
	#[pallet::getter(fn item_to_asset_instance)]
	pub type ItemToAssetInstance<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		ClassIdOf<T>,
		Twox64Concat,
		TokenIdOf<T>,
		xcm::v3::AssetInstance,
		OptionQuery,
	>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(1_000_000, 0)
			.saturating_add(<module_nft::weights::AcalaWeight<T>>::create_class())
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2)))]
		pub fn register_asset(origin: OriginFor<T>, foreign_asset: Box<AssetId>) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			ensure!(
				!<ForeignAssetToClass<T>>::contains_key(foreign_asset.as_ref()),
				<Error<T>>::AssetAlreadyRegistered,
			);

			let properties =
				Properties(ClassProperty::Mintable | ClassProperty::Burnable | ClassProperty::Transferable);
			let data = module_nft::ClassData {
				deposit: Default::default(),
				properties,
				attributes: Default::default(),
			};
			let collection_id = orml_nft::Pallet::<T>::create_class(&Self::account_id(), Default::default(), data)?;

			<ForeignAssetToClass<T>>::insert(foreign_asset.as_ref(), collection_id);
			<ClassToForeignAsset<T>>::insert(collection_id, foreign_asset.as_ref());

			Self::deposit_event(Event::AssetRegistered {
				asset_id: foreign_asset,
				collection_id,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		frame_support::PalletId(*b"poc_xnft").into_account_truncating()
	}
}
