// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
//! Local and foreign assets management. The foreign assets can be updated without runtime upgrade.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	traits::{Currency, EnsureOrigin},
};
use frame_system::pallet_prelude::*;
use module_support::{AssetIdMapping, BuyWeightRate, EVMBridge, Erc20InfoMapping, InvokeContext, Ratio};
use primitives::{
	currency::{
		AssetIds, AssetMetadata, CurrencyIdType, DexShare, DexShareType, Erc20Id, ForeignAssetId, Lease,
		StableAssetPoolId, TokenInfo,
	},
	evm::{
		is_system_contract, EvmAddress, H160_POSITION_CURRENCY_ID_TYPE, H160_POSITION_DEXSHARE_LEFT_FIELD,
		H160_POSITION_DEXSHARE_LEFT_TYPE, H160_POSITION_DEXSHARE_RIGHT_FIELD, H160_POSITION_DEXSHARE_RIGHT_TYPE,
		H160_POSITION_FOREIGN_ASSET, H160_POSITION_LIQUID_CROADLOAN, H160_POSITION_STABLE_ASSET, H160_POSITION_TOKEN,
	},
	CurrencyId,
};
use scale_info::prelude::format;
use sp_runtime::{traits::One, ArithmeticError, FixedPointNumber, FixedU128};
use sp_std::{boxed::Box, vec::Vec};

use xcm::{v3, v4::prelude::*, VersionedLocation};

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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency type for withdraw and balance storage.
		type Currency: Currency<Self::AccountId>;

		/// The Currency ID for the staking currency
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// Evm Bridge for getting info of contracts from the EVM.
		type EVMBridge: EVMBridge<Self::AccountId, BalanceOf<Self>>;

		/// Required origin for registering asset.
		type RegisterOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The given location could not be used (e.g. because it cannot be expressed in the
		/// desired version of XCM).
		BadLocation,
		/// Location existed
		LocationExisted,
		/// AssetId not exists
		AssetIdNotExists,
		/// AssetId exists
		AssetIdExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// The foreign asset registered.
		ForeignAssetRegistered {
			asset_id: ForeignAssetId,
			asset_address: Location,
			metadata: AssetMetadata<BalanceOf<T>>,
		},
		/// The foreign asset updated.
		ForeignAssetUpdated {
			asset_id: ForeignAssetId,
			asset_address: Location,
			metadata: AssetMetadata<BalanceOf<T>>,
		},
		/// The asset registered.
		AssetRegistered {
			asset_id: AssetIds,
			metadata: AssetMetadata<BalanceOf<T>>,
		},
		/// The asset updated.
		AssetUpdated {
			asset_id: AssetIds,
			metadata: AssetMetadata<BalanceOf<T>>,
		},
	}

	/// Next available Foreign AssetId ID.
	///
	/// NextForeignAssetId: ForeignAssetId
	#[pallet::storage]
	#[pallet::getter(fn next_foreign_asset_id)]
	pub type NextForeignAssetId<T: Config> = StorageValue<_, ForeignAssetId, ValueQuery>;

	/// Next available Stable AssetId ID.
	///
	/// NextStableAssetId: StableAssetPoolId
	#[pallet::storage]
	#[pallet::getter(fn next_stable_asset_id)]
	pub type NextStableAssetId<T: Config> = StorageValue<_, StableAssetPoolId, ValueQuery>;

	/// The storages for Locations.
	///
	/// ForeignAssetLocations: map ForeignAssetId => Option<Location>
	#[pallet::storage]
	#[pallet::getter(fn foreign_asset_locations)]
	pub type ForeignAssetLocations<T: Config> = StorageMap<_, Twox64Concat, ForeignAssetId, v3::Location, OptionQuery>;

	/// The storages for CurrencyIds.
	///
	/// LocationToCurrencyIds: map Location => Option<CurrencyId>
	#[pallet::storage]
	#[pallet::getter(fn location_to_currency_ids)]
	pub type LocationToCurrencyIds<T: Config> = StorageMap<_, Twox64Concat, v3::Location, CurrencyId, OptionQuery>;

	/// The storages for EvmAddress.
	///
	/// Erc20IdToAddress: map Erc20Id => Option<EvmAddress>
	#[pallet::storage]
	#[pallet::getter(fn erc20_id_to_address)]
	pub type Erc20IdToAddress<T: Config> = StorageMap<_, Twox64Concat, Erc20Id, EvmAddress, OptionQuery>;

	/// The storages for AssetMetadatas.
	///
	/// AssetMetadatas: map AssetIds => Option<AssetMetadata>
	#[pallet::storage]
	#[pallet::getter(fn asset_metadatas)]
	pub type AssetMetadatas<T: Config> =
		StorageMap<_, Twox64Concat, AssetIds, AssetMetadata<BalanceOf<T>>, OptionQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub assets: Vec<(CurrencyId, BalanceOf<T>)>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			self.assets.iter().for_each(|(asset, ed)| {
				frame_support::assert_ok!(Pallet::<T>::do_register_native_asset(
					*asset,
					&AssetMetadata {
						name: asset.name().unwrap().as_bytes().to_vec(),
						symbol: asset.symbol().unwrap().as_bytes().to_vec(),
						decimals: asset.decimals().unwrap(),
						minimal_balance: *ed,
					}
				));
			});
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::register_foreign_asset())]
		pub fn register_foreign_asset(
			origin: OriginFor<T>,
			location: Box<VersionedLocation>,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let location: Location = (*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			let foreign_asset_id = Self::do_register_foreign_asset(&location, &metadata)?;

			Self::deposit_event(Event::<T>::ForeignAssetRegistered {
				asset_id: foreign_asset_id,
				asset_address: location,
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::update_foreign_asset())]
		pub fn update_foreign_asset(
			origin: OriginFor<T>,
			foreign_asset_id: ForeignAssetId,
			location: Box<VersionedLocation>,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let location: Location = (*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			Self::do_update_foreign_asset(foreign_asset_id, &location, &metadata)?;

			Self::deposit_event(Event::<T>::ForeignAssetUpdated {
				asset_id: foreign_asset_id,
				asset_address: location,
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::register_stable_asset())]
		pub fn register_stable_asset(
			origin: OriginFor<T>,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let stable_asset_id = Self::do_register_stable_asset(&metadata)?;

			Self::deposit_event(Event::<T>::AssetRegistered {
				asset_id: AssetIds::StableAssetId(stable_asset_id),
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::update_stable_asset())]
		pub fn update_stable_asset(
			origin: OriginFor<T>,
			stable_asset_id: StableAssetPoolId,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			Self::do_update_stable_asset(&stable_asset_id, &metadata)?;

			Self::deposit_event(Event::<T>::AssetUpdated {
				asset_id: AssetIds::StableAssetId(stable_asset_id),
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::register_erc20_asset())]
		pub fn register_erc20_asset(
			origin: OriginFor<T>,
			contract: EvmAddress,
			minimal_balance: BalanceOf<T>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let metadata = Self::do_register_erc20_asset(contract, minimal_balance)?;

			Self::deposit_event(Event::<T>::AssetRegistered {
				asset_id: AssetIds::Erc20(contract),
				metadata,
			});
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::update_erc20_asset())]
		pub fn update_erc20_asset(
			origin: OriginFor<T>,
			contract: EvmAddress,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			Self::do_update_erc20_asset(contract, &metadata)?;

			Self::deposit_event(Event::<T>::AssetUpdated {
				asset_id: AssetIds::Erc20(contract),
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::register_native_asset())]
		pub fn register_native_asset(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			Self::do_register_native_asset(currency_id, &metadata)?;

			Self::deposit_event(Event::<T>::AssetRegistered {
				asset_id: AssetIds::NativeAssetId(currency_id),
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(T::WeightInfo::update_native_asset())]
		pub fn update_native_asset(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			Self::do_update_native_asset(currency_id, &metadata)?;

			Self::deposit_event(Event::<T>::AssetUpdated {
				asset_id: AssetIds::NativeAssetId(currency_id),
				metadata: *metadata,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn get_next_stable_asset_id() -> Result<StableAssetPoolId, DispatchError> {
		NextStableAssetId::<T>::try_mutate(|current| -> Result<StableAssetPoolId, DispatchError> {
			let id = *current;
			*current = current.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	fn get_next_foreign_asset_id() -> Result<ForeignAssetId, DispatchError> {
		NextForeignAssetId::<T>::try_mutate(|current| -> Result<ForeignAssetId, DispatchError> {
			let id = *current;
			*current = current.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	fn do_register_foreign_asset(
		location: &Location,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> Result<ForeignAssetId, DispatchError> {
		let foreign_asset_id = Self::get_next_foreign_asset_id()?;
		let v3_location = v3::Location::try_from(location.clone()).map_err(|()| Error::<T>::BadLocation)?;
		LocationToCurrencyIds::<T>::try_mutate(v3_location, |maybe_currency_ids| -> DispatchResult {
			ensure!(maybe_currency_ids.is_none(), Error::<T>::LocationExisted);
			*maybe_currency_ids = Some(CurrencyId::ForeignAsset(foreign_asset_id));

			ForeignAssetLocations::<T>::try_mutate(foreign_asset_id, |maybe_location| -> DispatchResult {
				ensure!(maybe_location.is_none(), Error::<T>::LocationExisted);
				*maybe_location = Some(v3_location);

				AssetMetadatas::<T>::try_mutate(
					AssetIds::ForeignAssetId(foreign_asset_id),
					|maybe_asset_metadatas| -> DispatchResult {
						ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetIdExisted);

						*maybe_asset_metadatas = Some(metadata.clone());
						Ok(())
					},
				)
			})
		})?;

		Ok(foreign_asset_id)
	}

	fn do_update_foreign_asset(
		foreign_asset_id: ForeignAssetId,
		location: &Location,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		let v3_location = v3::Location::try_from(location.clone()).map_err(|()| Error::<T>::BadLocation)?;
		ForeignAssetLocations::<T>::try_mutate(foreign_asset_id, |maybe_locations| -> DispatchResult {
			let old_locations = maybe_locations.as_mut().ok_or(Error::<T>::AssetIdNotExists)?;

			AssetMetadatas::<T>::try_mutate(
				AssetIds::ForeignAssetId(foreign_asset_id),
				|maybe_asset_metadatas| -> DispatchResult {
					ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetIdNotExists);

					// modify location
					if v3_location != *old_locations {
						LocationToCurrencyIds::<T>::remove(*old_locations);
						LocationToCurrencyIds::<T>::try_mutate(v3_location, |maybe_currency_ids| -> DispatchResult {
							ensure!(maybe_currency_ids.is_none(), Error::<T>::LocationExisted);
							*maybe_currency_ids = Some(CurrencyId::ForeignAsset(foreign_asset_id));
							Ok(())
						})?;
					}
					*maybe_asset_metadatas = Some(metadata.clone());
					*old_locations = v3_location;
					Ok(())
				},
			)
		})
	}

	fn do_register_stable_asset(metadata: &AssetMetadata<BalanceOf<T>>) -> Result<StableAssetPoolId, DispatchError> {
		let stable_asset_id = Self::get_next_stable_asset_id()?;
		AssetMetadatas::<T>::try_mutate(
			AssetIds::StableAssetId(stable_asset_id),
			|maybe_asset_metadatas| -> DispatchResult {
				ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetIdExisted);

				*maybe_asset_metadatas = Some(metadata.clone());
				Ok(())
			},
		)?;

		Ok(stable_asset_id)
	}

	fn do_update_stable_asset(
		stable_asset_id: &StableAssetPoolId,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		AssetMetadatas::<T>::try_mutate(
			AssetIds::StableAssetId(*stable_asset_id),
			|maybe_asset_metadatas| -> DispatchResult {
				ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetIdNotExists);

				*maybe_asset_metadatas = Some(metadata.clone());
				Ok(())
			},
		)
	}

	fn do_register_erc20_asset(
		contract: EvmAddress,
		minimal_balance: BalanceOf<T>,
	) -> Result<AssetMetadata<BalanceOf<T>>, DispatchError> {
		let invoke_context = InvokeContext {
			contract,
			sender: Default::default(),
			origin: Default::default(),
		};

		let metadata = AssetMetadata {
			name: T::EVMBridge::name(invoke_context)?,
			symbol: T::EVMBridge::symbol(invoke_context)?,
			decimals: T::EVMBridge::decimals(invoke_context)?,
			minimal_balance,
		};

		let erc20_id = Into::<Erc20Id>::into(DexShare::Erc20(contract));

		AssetMetadatas::<T>::try_mutate(AssetIds::Erc20(contract), |maybe_asset_metadatas| -> DispatchResult {
			ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetIdExisted);

			Erc20IdToAddress::<T>::try_mutate(erc20_id, |maybe_address| -> DispatchResult {
				ensure!(maybe_address.is_none(), Error::<T>::AssetIdExisted);
				*maybe_address = Some(contract);

				Ok(())
			})?;

			*maybe_asset_metadatas = Some(metadata.clone());
			Ok(())
		})?;

		Ok(metadata)
	}

	fn do_update_erc20_asset(contract: EvmAddress, metadata: &AssetMetadata<BalanceOf<T>>) -> DispatchResult {
		AssetMetadatas::<T>::try_mutate(AssetIds::Erc20(contract), |maybe_asset_metadatas| -> DispatchResult {
			ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetIdNotExists);

			*maybe_asset_metadatas = Some(metadata.clone());
			Ok(())
		})
	}

	fn do_register_native_asset(asset: CurrencyId, metadata: &AssetMetadata<BalanceOf<T>>) -> DispatchResult {
		AssetMetadatas::<T>::try_mutate(
			AssetIds::NativeAssetId(asset),
			|maybe_asset_metadatas| -> DispatchResult {
				ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetIdExisted);

				*maybe_asset_metadatas = Some(metadata.clone());
				Ok(())
			},
		)?;

		Ok(())
	}

	fn do_update_native_asset(currency_id: CurrencyId, metadata: &AssetMetadata<BalanceOf<T>>) -> DispatchResult {
		AssetMetadatas::<T>::try_mutate(
			AssetIds::NativeAssetId(currency_id),
			|maybe_asset_metadatas| -> DispatchResult {
				ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetIdNotExists);

				*maybe_asset_metadatas = Some(metadata.clone());
				Ok(())
			},
		)
	}
}

pub struct AssetIdMaps<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> AssetIdMapping<ForeignAssetId, Location, AssetMetadata<BalanceOf<T>>> for AssetIdMaps<T> {
	fn get_asset_metadata(asset_ids: AssetIds) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(asset_ids)
	}

	fn get_location(foreign_asset_id: ForeignAssetId) -> Option<Location> {
		Pallet::<T>::foreign_asset_locations(foreign_asset_id).map(|l| l.try_into().ok())?
	}

	fn get_currency_id(location: Location) -> Option<CurrencyId> {
		let v3_location = v3::Location::try_from(location).ok()?;
		Pallet::<T>::location_to_currency_ids(v3_location)
	}
}

fn key_to_currency(location: Location) -> Option<CurrencyId> {
	match location.unpack() {
		(0, [Junction::GeneralKey { data, length }]) => {
			let key = &data[..data.len().min(*length as usize)];
			CurrencyId::decode(&mut &*key).ok()
		}
		_ => None,
	}
}

pub struct BuyWeightRateOfForeignAsset<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> BuyWeightRate for BuyWeightRateOfForeignAsset<T>
where
	BalanceOf<T>: Into<u128>,
{
	fn calculate_rate(location: Location) -> Option<Ratio> {
		let v3_location = v3::Location::try_from(location).ok()?;
		if let Some(CurrencyId::ForeignAsset(foreign_asset_id)) = Pallet::<T>::location_to_currency_ids(v3_location) {
			if let Some(asset_metadata) = Pallet::<T>::asset_metadatas(AssetIds::ForeignAssetId(foreign_asset_id)) {
				let minimum_balance = asset_metadata.minimal_balance.into();
				let rate = FixedU128::saturating_from_rational(minimum_balance, T::Currency::minimum_balance().into());
				log::debug!(target: "asset-registry::weight", "ForeignAsset: {}, MinimumBalance: {}, rate:{:?}", foreign_asset_id, minimum_balance, rate);
				return Some(rate);
			}
		}
		None
	}
}

pub struct BuyWeightRateOfLiquidCrowdloan<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> BuyWeightRate for BuyWeightRateOfLiquidCrowdloan<T>
where
	BalanceOf<T>: Into<u128>,
{
	fn calculate_rate(location: Location) -> Option<Ratio> {
		let currency = key_to_currency(location);
		match currency {
			Some(CurrencyId::LiquidCrowdloan(lease)) => {
				if let Some(asset_metadata) =
					Pallet::<T>::asset_metadatas(AssetIds::NativeAssetId(CurrencyId::LiquidCrowdloan(lease)))
				{
					let minimum_balance = asset_metadata.minimal_balance.into();
					let rate =
						FixedU128::saturating_from_rational(minimum_balance, T::Currency::minimum_balance().into());
					log::debug!(target: "asset-registry::weight", "LiquidCrowdloan: {}, MinimumBalance: {}, rate:{:?}", lease, minimum_balance, rate);
					Some(rate)
				} else {
					None
				}
			}
			_ => None,
		}
	}
}

pub struct BuyWeightRateOfStableAsset<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> BuyWeightRate for BuyWeightRateOfStableAsset<T>
where
	BalanceOf<T>: Into<u128>,
{
	fn calculate_rate(location: Location) -> Option<Ratio> {
		let currency = key_to_currency(location);
		match currency {
			Some(CurrencyId::StableAssetPoolToken(pool_id)) => {
				if let Some(asset_metadata) = Pallet::<T>::asset_metadatas(AssetIds::StableAssetId(pool_id)) {
					let minimum_balance = asset_metadata.minimal_balance.into();
					let rate =
						FixedU128::saturating_from_rational(minimum_balance, T::Currency::minimum_balance().into());
					log::debug!(target: "asset-registry::weight", "StableAsset: {}, MinimumBalance: {}, rate:{:?}", pool_id, minimum_balance, rate);
					Some(rate)
				} else {
					None
				}
			}
			_ => None,
		}
	}
}

pub struct BuyWeightRateOfErc20<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> BuyWeightRate for BuyWeightRateOfErc20<T>
where
	BalanceOf<T>: Into<u128>,
{
	fn calculate_rate(location: Location) -> Option<Ratio> {
		let currency = key_to_currency(location);
		match currency {
			Some(CurrencyId::Erc20(address)) if !is_system_contract(&address) => {
				if let Some(asset_metadata) = Pallet::<T>::asset_metadatas(AssetIds::Erc20(address)) {
					let minimum_balance = asset_metadata.minimal_balance.into();
					let rate =
						FixedU128::saturating_from_rational(minimum_balance, T::Currency::minimum_balance().into());
					log::debug!(target: "asset-registry::weight", "Erc20: {}, MinimumBalance: {}, rate:{:?}", address, minimum_balance, rate);
					Some(rate)
				} else {
					None
				}
			}
			_ => None,
		}
	}
}

pub struct EvmErc20InfoMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> EvmErc20InfoMapping<T> {
	fn name_for_dex_share(symbol: DexShare) -> Option<Vec<u8>> {
		match symbol {
			DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
			DexShare::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.name),
			DexShare::LiquidCrowdloan(lease) => Some(
				format!(
					"LiquidCrowdloan-{}-{}",
					T::StakingCurrencyId::get().name().expect("constant never failed; qed"),
					lease
				)
				.into_bytes(),
			),
			DexShare::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.name)
			}
			DexShare::StableAssetPoolToken(stable_asset_pool_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_pool_id)).map(|v| v.name)
			}
		}
	}

	fn symbol_for_dex_share(symbol: DexShare) -> Option<Vec<u8>> {
		match symbol {
			DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
			DexShare::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.symbol),
			DexShare::LiquidCrowdloan(lease) => Some(
				format!(
					"LC{}-{}",
					T::StakingCurrencyId::get()
						.symbol()
						.expect("constant never failed; qed"),
					lease
				)
				.into_bytes(),
			),
			DexShare::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.symbol)
			}
			DexShare::StableAssetPoolToken(stable_asset_pool_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_pool_id)).map(|v| v.symbol)
			}
		}
	}

	fn decimal_for_dex_share(symbol: DexShare) -> Option<u8> {
		match symbol {
			DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
			DexShare::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.decimals),
			DexShare::LiquidCrowdloan(_) => T::StakingCurrencyId::get().decimals(),
			DexShare::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.decimals)
			}
			DexShare::StableAssetPoolToken(stable_asset_pool_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_pool_id)).map(|v| v.decimals)
			}
		}
	}

	fn decode_evm_address_for_dex_share(address: &[u8], left: bool) -> Option<DexShare> {
		let (dex_share_type, dex_share_field) = if left {
			(H160_POSITION_DEXSHARE_LEFT_TYPE, H160_POSITION_DEXSHARE_LEFT_FIELD)
		} else {
			(H160_POSITION_DEXSHARE_RIGHT_TYPE, H160_POSITION_DEXSHARE_RIGHT_FIELD)
		};
		match DexShareType::try_from(address[dex_share_type]).ok()? {
			DexShareType::Token => address[dex_share_field][3].try_into().map(DexShare::Token).ok(),
			DexShareType::Erc20 => {
				let id = u32::from_be_bytes(address[dex_share_field].try_into().ok()?);
				Erc20IdToAddress::<T>::get(id).map(DexShare::Erc20)
			}
			DexShareType::LiquidCrowdloan => {
				let id = Lease::from_be_bytes(address[dex_share_field].try_into().ok()?);
				Some(DexShare::LiquidCrowdloan(id))
			}
			DexShareType::ForeignAsset => {
				let id = ForeignAssetId::from_be_bytes(address[dex_share_field][2..].try_into().ok()?);
				Some(DexShare::ForeignAsset(id))
			}
			DexShareType::StableAssetPoolToken => {
				let id = StableAssetPoolId::from_be_bytes(address[dex_share_field][..].try_into().ok()?);
				Some(DexShare::StableAssetPoolToken(id))
			}
		}
	}
}

impl<T: Config> Erc20InfoMapping for EvmErc20InfoMapping<T> {
	// Returns the name associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let name = match currency_id {
			CurrencyId::Token(_) => AssetMetadatas::<T>::get(AssetIds::NativeAssetId(currency_id)).map(|v| v.name),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let name_0 = EvmErc20InfoMapping::<T>::name_for_dex_share(symbol_0)?;
				let name_1 = EvmErc20InfoMapping::<T>::name_for_dex_share(symbol_1)?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP "[..]);
				vec.extend_from_slice(&name_0);
				vec.extend_from_slice(&b" - "[..]);
				vec.extend_from_slice(&name_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.name),
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_id)).map(|v| v.name)
			}
			CurrencyId::LiquidCrowdloan(lease) => Some(
				format!(
					"LiquidCrowdloan-{}-{}",
					T::StakingCurrencyId::get().name().expect("constant never failed; qed"),
					lease
				)
				.into_bytes(),
			),
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.name)
			}
		}?;

		// More than 32 bytes will be truncated.
		if name.len() > 32 {
			Some(name[..32].to_vec())
		} else {
			Some(name)
		}
	}

	// Returns the symbol associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn symbol(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let symbol = match currency_id {
			CurrencyId::Token(_) => AssetMetadatas::<T>::get(AssetIds::NativeAssetId(currency_id)).map(|v| v.symbol),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let token_symbol_0 = EvmErc20InfoMapping::<T>::symbol_for_dex_share(symbol_0)?;
				let token_symbol_1 = EvmErc20InfoMapping::<T>::symbol_for_dex_share(symbol_1)?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP_"[..]);
				vec.extend_from_slice(&token_symbol_0);
				vec.extend_from_slice(&b"_"[..]);
				vec.extend_from_slice(&token_symbol_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.symbol),
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_id)).map(|v| v.symbol)
			}
			CurrencyId::LiquidCrowdloan(lease) => Some(
				format!(
					"LC{}-{}",
					T::StakingCurrencyId::get()
						.symbol()
						.expect("constant never failed; qed"),
					lease
				)
				.into_bytes(),
			),
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.symbol)
			}
		}?;

		// More than 32 bytes will be truncated.
		if symbol.len() > 32 {
			Some(symbol[..32].to_vec())
		} else {
			Some(symbol)
		}
	}

	// Returns the decimals associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn decimals(currency_id: CurrencyId) -> Option<u8> {
		match currency_id {
			CurrencyId::Token(_) => AssetMetadatas::<T>::get(AssetIds::NativeAssetId(currency_id)).map(|v| v.decimals),
			CurrencyId::DexShare(symbol_0, _) => {
				// initial dex share amount is calculated based on currency_id_0,
				// use the decimals of currency_id_0 as the decimals of lp token.
				EvmErc20InfoMapping::<T>::decimal_for_dex_share(symbol_0)
			}
			CurrencyId::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.decimals),
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::StableAssetId(stable_asset_id)).map(|v| v.decimals)
			}
			CurrencyId::LiquidCrowdloan(_) => T::StakingCurrencyId::get().decimals(),
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.decimals)
			}
		}
	}

	// Encode the CurrencyId to EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress> {
		match v {
			CurrencyId::DexShare(left, right) => {
				match left {
					DexShare::Erc20(address) => {
						// ensure erc20 is mapped
						AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|_| ())?;
					}
					DexShare::Token(_)
					| DexShare::LiquidCrowdloan(_)
					| DexShare::ForeignAsset(_)
					| DexShare::StableAssetPoolToken(_) => {}
				};
				match right {
					DexShare::Erc20(address) => {
						// ensure erc20 is mapped
						AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|_| ())?;
					}
					DexShare::Token(_)
					| DexShare::LiquidCrowdloan(_)
					| DexShare::ForeignAsset(_)
					| DexShare::StableAssetPoolToken(_) => {}
				};
			}
			CurrencyId::Token(_)
			| CurrencyId::Erc20(_)
			| CurrencyId::StableAssetPoolToken(_)
			| CurrencyId::LiquidCrowdloan(_)
			| CurrencyId::ForeignAsset(_) => {}
		};

		EvmAddress::try_from(v).ok()
	}

	// Decode the CurrencyId from EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(addr: EvmAddress) -> Option<CurrencyId> {
		if !is_system_contract(&addr) {
			return Some(CurrencyId::Erc20(addr));
		}

		let address = addr.as_bytes();
		let currency_id = match CurrencyIdType::try_from(address[H160_POSITION_CURRENCY_ID_TYPE]).ok()? {
			CurrencyIdType::Token => address[H160_POSITION_TOKEN].try_into().map(CurrencyId::Token).ok(),
			CurrencyIdType::DexShare => {
				let left = EvmErc20InfoMapping::<T>::decode_evm_address_for_dex_share(address, true)?;
				let right = EvmErc20InfoMapping::<T>::decode_evm_address_for_dex_share(address, false)?;
				Some(CurrencyId::DexShare(left, right))
			}
			CurrencyIdType::StableAsset => {
				let id = StableAssetPoolId::from_be_bytes(address[H160_POSITION_STABLE_ASSET].try_into().ok()?);
				Some(CurrencyId::StableAssetPoolToken(id))
			}
			CurrencyIdType::LiquidCrowdloan => {
				let id = Lease::from_be_bytes(address[H160_POSITION_LIQUID_CROADLOAN].try_into().ok()?);
				Some(CurrencyId::LiquidCrowdloan(id))
			}
			CurrencyIdType::ForeignAsset => {
				let id = ForeignAssetId::from_be_bytes(address[H160_POSITION_FOREIGN_ASSET].try_into().ok()?);
				Some(CurrencyId::ForeignAsset(id))
			}
		};

		// Make sure that every bit of the address is the same
		Self::encode_evm_address(currency_id?).and_then(|encoded| if encoded == addr { currency_id } else { None })
	}
}
