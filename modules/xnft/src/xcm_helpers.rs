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

use crate::*;
use module_nft::BalanceOf;
use primitives::nft::Attributes;
use xcm::v3::AssetId::Concrete;
use xcm_executor::traits::Error as MatchError;

pub enum ClassLocality<T: Config> {
	Local(ClassIdOf<T>),
	Foreign(ClassIdOf<T>),
}

impl<T: Config> Pallet<T>
where
	TokenIdOf<T>: TryFrom<u128>,
	ClassIdOf<T>: TryFrom<u128>,
{
	pub fn asset_to_collection(asset: &AssetId) -> Result<ClassLocality<T>, MatchError> {
		Self::foreign_asset_to_class(asset)
			.map(ClassLocality::Foreign)
			.or_else(|| Self::local_asset_to_class(asset).map(ClassLocality::Local))
			.ok_or(MatchError::AssetIdConversionFailed)
	}

	fn local_asset_to_class(asset: &AssetId) -> Option<ClassIdOf<T>> {
		let Concrete(asset_location) = asset else {
			return None;
		};

		let prefix = if asset_location.parents == 0 {
			T::NtfPalletLocation::get()
		} else if asset_location.parents == 1 {
			T::NtfPalletLocation::get()
				.pushed_front_with(Parachain(T::SelfParaId::get().into()))
				.ok()?
		} else {
			return None;
		};

		match asset_location.interior.match_and_split(&prefix) {
			Some(GeneralIndex(index)) => {
				let class_id = (*index).try_into().ok()?;
				Self::class_to_foreign_asset(class_id).is_none().then_some(class_id)
			}
			_ => None,
		}
	}

	pub fn deposit_foreign_asset(to: &T::AccountId, asset: ClassIdOf<T>, asset_instance: &AssetInstance) -> XcmResult {
		match Self::asset_instance_to_item(asset, asset_instance) {
			Some(token_id) => <ModuleNftPallet<T>>::do_transfer(&Self::account_id(), to, (asset, token_id))
				.map_err(|_| XcmError::FailedToTransactAsset("non-fungible foreign item deposit failed")),
			None => {
				let token_id = <OrmlNftPallet<T>>::mint(
					to,
					asset,
					Vec::new(),
					module_nft::TokenData::<BalanceOf<T>> {
						deposit: 0u32.into(),
						attributes: Attributes::new(),
					},
				)
				.map_err(|_| XcmError::FailedToTransactAsset("non-fungible new foreign item deposit failed"))?;

				<AssetInstanceToItem<T>>::insert(asset, asset_instance, token_id);
				<ItemToAssetInstance<T>>::insert(asset, token_id, asset_instance);

				Ok(())
			}
		}
	}

	pub fn deposit_local_asset(to: &T::AccountId, asset: ClassIdOf<T>, asset_instance: &AssetInstance) -> XcmResult {
		let token_id = Self::convert_asset_instance(asset_instance)?;
		<ModuleNftPallet<T>>::do_transfer(&Self::account_id(), to, (asset, token_id))
			.map_err(|_| XcmError::FailedToTransactAsset("non-fungible local item deposit failed"))
	}

	pub fn asset_instance_to_token(
		class_locality: ClassLocality<T>,
		asset_instance: &AssetInstance,
	) -> Option<(ClassIdOf<T>, TokenIdOf<T>)> {
		match class_locality {
			ClassLocality::Foreign(class_id) => {
				Self::asset_instance_to_item(class_id, asset_instance).map(|token_id| (class_id, token_id))
			}
			ClassLocality::Local(class_id) => Self::convert_asset_instance(asset_instance)
				.map(|token_id| (class_id, token_id))
				.ok(),
		}
	}

	fn convert_asset_instance(asset: &AssetInstance) -> Result<TokenIdOf<T>, MatchError> {
		let AssetInstance::Index(index) = asset else {
			return Err(MatchError::InstanceConversionFailed);
		};

		(*index).try_into().map_err(|_| MatchError::InstanceConversionFailed)
	}
}
