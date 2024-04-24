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

use crate::{xcm_helpers::ClassLocality, *};

const LOG_TARGET: &str = "xcm::module_xnft::transactor";

impl<T: Config> TransactAsset for Pallet<T>
where
	TokenIdOf<T>: TryFrom<u128>,
	ClassIdOf<T>: TryFrom<u128>,
{
	fn can_check_in(_origin: &Location, _what: &Asset, _context: &XcmContext) -> XcmResult {
		Err(XcmError::Unimplemented)
	}

	fn check_in(_origin: &Location, _what: &Asset, _context: &XcmContext) {}

	fn can_check_out(_dest: &Location, _what: &Asset, _context: &XcmContext) -> XcmResult {
		Err(XcmError::Unimplemented)
	}

	fn check_out(_dest: &Location, _what: &Asset, _context: &XcmContext) {}

	fn deposit_asset(what: &Asset, who: &Location, context: Option<&XcmContext>) -> XcmResult {
		log::trace!(
			target: LOG_TARGET,
			"deposit_asset what: {:?}, who: {:?}, context: {:?}",
			what,
			who,
			context,
		);

		let Fungibility::NonFungible(asset_instance) = what.fun else {
			return Err(XcmExecutorError::AssetNotHandled.into());
		};

		let class_locality = Self::asset_to_collection(&what.id)?;

		let to = <ConverterOf<T>>::convert_location(who).ok_or(XcmExecutorError::AccountIdConversionFailed)?;

		match class_locality {
			ClassLocality::Foreign(class_id) => Self::deposit_foreign_asset(&to, class_id, &asset_instance),
			ClassLocality::Local(class_id) => Self::deposit_local_asset(&to, class_id, &asset_instance),
		}
	}

	fn withdraw_asset(
		what: &Asset,
		who: &Location,
		maybe_context: Option<&XcmContext>,
	) -> Result<AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"withdraw_asset what: {:?}, who: {:?}, maybe_context: {:?}",
			what,
			who,
			maybe_context,
		);

		let Fungibility::NonFungible(asset_instance) = what.fun else {
			return Err(XcmExecutorError::AssetNotHandled.into());
		};

		let class_locality = Self::asset_to_collection(&what.id)?;

		let from = <ConverterOf<T>>::convert_location(who).ok_or(XcmExecutorError::AccountIdConversionFailed)?;

		let token = Self::asset_instance_to_token(class_locality, &asset_instance)
			.ok_or(XcmExecutorError::InstanceConversionFailed)?;

		<ModuleNftPallet<T>>::do_transfer(&from, &Self::account_id(), token)
			.map(|_| what.clone().into())
			.map_err(|_| XcmError::FailedToTransactAsset("non-fungible item withdraw failed"))
	}

	fn internal_transfer_asset(
		asset: &Asset,
		from: &Location,
		to: &Location,
		context: &XcmContext,
	) -> Result<AssetsInHolding, XcmError> {
		log::trace!(
			target: LOG_TARGET,
			"internal_transfer_asset: {:?}, from: {:?}, to: {:?}, context: {:?}",
			asset,
			from,
			to,
			context
		);

		let Fungibility::NonFungible(asset_instance) = asset.fun else {
			return Err(XcmExecutorError::AssetNotHandled.into());
		};

		let class_locality = Self::asset_to_collection(&asset.id)?;

		let from = <ConverterOf<T>>::convert_location(from).ok_or(XcmExecutorError::AccountIdConversionFailed)?;
		let to = <ConverterOf<T>>::convert_location(to).ok_or(XcmExecutorError::AccountIdConversionFailed)?;

		let token = Self::asset_instance_to_token(class_locality, &asset_instance)
			.ok_or(XcmExecutorError::InstanceConversionFailed)?;

		<ModuleNftPallet<T>>::do_transfer(&from, &to, token)
			.map(|_| asset.clone().into())
			.map_err(|_| XcmError::FailedToTransactAsset("non-fungible item internal transfer failed"))
	}
}
