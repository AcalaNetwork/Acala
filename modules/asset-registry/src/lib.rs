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

use frame_support::{
	dispatch::DispatchResult,
	ensure,
	pallet_prelude::*,
	require_transactional,
	traits::{Currency, EnsureOrigin},
	transactional, RuntimeDebug,
};
use frame_system::pallet_prelude::*;
use module_support::{CurrencyIdMapping, EVMBridge, ForeignAssetIdMapping, InvokeContext};
use primitives::{
	currency::{CurrencyIdType, DexShare, DexShareType, ForeignAssetId, Lease, TokenInfo, TokenSymbol},
	evm::{
		is_system_contract, Erc20Info, EvmAddress, H160_POSITION_CURRENCY_ID_TYPE, H160_POSITION_DEXSHARE_LEFT_FIELD,
		H160_POSITION_DEXSHARE_LEFT_TYPE, H160_POSITION_DEXSHARE_RIGHT_FIELD, H160_POSITION_DEXSHARE_RIGHT_TYPE,
		H160_POSITION_FOREIGN_ASSET, H160_POSITION_LIQUID_CROADLOAN, H160_POSITION_TOKEN,
	},
	CurrencyId,
};
use scale_info::{prelude::format, TypeInfo};
use sp_runtime::{traits::One, ArithmeticError};
use sp_std::{
	boxed::Box,
	convert::{TryFrom, TryInto},
	vec::Vec,
};
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

		/// Evm Bridge for getting info of contracts from the EVM.
		type EVMBridge: EVMBridge<Self::AccountId, BalanceOf<Self>>;

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
		/// MultiLocation existed
		MultiLocationExisted,
		/// ForeignAssetId not exists
		ForeignAssetIdNotExists,
		/// CurrencyId existed
		CurrencyIdExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Registered foreign asset. \[ForeignAssetId, AssetMetadata\]
		RegisteredForeignAsset(ForeignAssetId, MultiLocation, AssetMetadata<BalanceOf<T>>),
		/// Updated foreign asset. \[AssetMetadata\]
		UpdatedForeignAsset(MultiLocation, AssetMetadata<BalanceOf<T>>),
	}

	/// Next available Foreign AssetId ID.
	///
	/// NextForeignAssetId: ForeignAssetId
	#[pallet::storage]
	#[pallet::getter(fn next_foreign_asset_id)]
	pub type NextForeignAssetId<T: Config> = StorageValue<_, ForeignAssetId, ValueQuery>;

	/// The storages for MultiLocations.
	///
	/// MultiLocations: map ForeignAssetId => Option<MultiLocation>
	#[pallet::storage]
	#[pallet::getter(fn multi_locations)]
	pub type MultiLocations<T: Config> = StorageMap<_, Twox64Concat, ForeignAssetId, MultiLocation, OptionQuery>;

	/// The storages for CurrencyIds.
	///
	/// MultiLocations: map MultiLocation => Option<CurrencyId>
	#[pallet::storage]
	#[pallet::getter(fn currency_ids)]
	pub type CurrencyIds<T: Config> = StorageMap<_, Twox64Concat, MultiLocation, CurrencyId, OptionQuery>;

	/// The storages for AssetMetadatas.
	///
	/// AssetMetadatas: map ForeignAssetId => Option<AssetMetadata>
	#[pallet::storage]
	#[pallet::getter(fn asset_metadatas)]
	pub type AssetMetadatas<T: Config> =
		StorageMap<_, Twox64Concat, ForeignAssetId, AssetMetadata<BalanceOf<T>>, OptionQuery>;

	/// Mapping between u32 and Erc20 address.
	/// Erc20 address is 20 byte, take the first 4 non-zero bytes, if it is less
	/// than 4, add 0 to the left.
	///
	/// map u32 => Option<Erc20Info>
	#[pallet::storage]
	#[pallet::getter(fn currency_id_map)]
	pub type CurrencyIdMap<T: Config> = StorageMap<_, Twox64Concat, u32, Erc20Info, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::register_foreign_asset())]
		#[transactional]
		pub fn register_foreign_asset(
			origin: OriginFor<T>,
			location: Box<VersionedMultiLocation>,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let location: MultiLocation = (*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			let foreign_asset_id = Self::do_register_foreign_asset(&location, &metadata)?;

			Self::deposit_event(Event::<T>::RegisteredForeignAsset(
				foreign_asset_id,
				location,
				*metadata,
			));
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::update_foreign_asset())]
		#[transactional]
		pub fn update_foreign_asset(
			origin: OriginFor<T>,
			foreign_asset_id: ForeignAssetId,
			location: Box<VersionedMultiLocation>,
			metadata: Box<AssetMetadata<BalanceOf<T>>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;

			let location: MultiLocation = (*location).try_into().map_err(|()| Error::<T>::BadLocation)?;
			Self::do_update_foreign_asset(foreign_asset_id, &location, &metadata)?;

			Self::deposit_event(Event::<T>::UpdatedForeignAsset(location, *metadata));
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn get_next_foreign_asset_id() -> Result<ForeignAssetId, DispatchError> {
		NextForeignAssetId::<T>::mutate(|current| -> Result<ForeignAssetId, DispatchError> {
			let id = *current;
			*current = current.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	fn do_register_foreign_asset(
		location: &MultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> Result<ForeignAssetId, DispatchError> {
		let id = Self::get_next_foreign_asset_id()?;
		CurrencyIds::<T>::try_mutate(location, |maybe_currency_ids| -> DispatchResult {
			ensure!(maybe_currency_ids.is_none(), Error::<T>::MultiLocationExisted);
			*maybe_currency_ids = Some(CurrencyId::ForeignAsset(id));
			Ok(())
		})?;
		MultiLocations::<T>::insert(id, location);
		AssetMetadatas::<T>::insert(id, metadata);

		Ok(id)
	}

	fn do_update_foreign_asset(
		foreign_asset_id: ForeignAssetId,
		location: &MultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		MultiLocations::<T>::mutate(foreign_asset_id, |maybe_multi_locations| -> DispatchResult {
			let old_multi_locations = maybe_multi_locations
				.as_mut()
				.ok_or(Error::<T>::ForeignAssetIdNotExists)?;

			AssetMetadatas::<T>::mutate(foreign_asset_id, |maybe_asset_metadatas| -> DispatchResult {
				ensure!(maybe_asset_metadatas.is_some(), Error::<T>::ForeignAssetIdNotExists);

				// modify location
				if location != old_multi_locations {
					CurrencyIds::<T>::remove(old_multi_locations.clone());
					CurrencyIds::<T>::try_mutate(location, |maybe_currency_ids| -> DispatchResult {
						ensure!(maybe_currency_ids.is_none(), Error::<T>::MultiLocationExisted);
						*maybe_currency_ids = Some(CurrencyId::ForeignAsset(foreign_asset_id));
						Ok(())
					})?;
				}
				*maybe_asset_metadatas = Some(metadata.clone());
				*old_multi_locations = location.clone();
				Ok(())
			})
		})
	}

	fn get_erc20_mapping(address: EvmAddress) -> Option<Erc20Info> {
		CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address))).filter(|v| v.address == address)
	}
}

pub struct XcmForeignAssetIdMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> ForeignAssetIdMapping<ForeignAssetId, MultiLocation, AssetMetadata<BalanceOf<T>>>
	for XcmForeignAssetIdMapping<T>
{
	fn get_asset_metadata(foreign_asset_id: ForeignAssetId) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(foreign_asset_id)
	}

	fn get_multi_location(foreign_asset_id: ForeignAssetId) -> Option<MultiLocation> {
		Pallet::<T>::multi_locations(foreign_asset_id)
	}

	fn get_currency_id(multi_location: MultiLocation) -> Option<CurrencyId> {
		Pallet::<T>::currency_ids(multi_location)
	}
}

pub struct EvmCurrencyIdMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> CurrencyIdMapping for EvmCurrencyIdMapping<T> {
	// Use first 4 non-zero bytes as u32 to the mapping between u32 and evm address.
	// Take the first 4 non-zero bytes, if it is less than 4, add 0 to the left.
	#[require_transactional]
	fn set_erc20_mapping(address: EvmAddress) -> DispatchResult {
		CurrencyIdMap::<T>::mutate(
			Into::<u32>::into(DexShare::Erc20(address)),
			|maybe_erc20_info| -> DispatchResult {
				if let Some(erc20_info) = maybe_erc20_info.as_mut() {
					// Multiple settings are allowed, such as enabling multiple LP tokens
					ensure!(erc20_info.address == address, Error::<T>::CurrencyIdExisted);
				} else {
					let invoke_context = InvokeContext {
						contract: address,
						sender: Default::default(),
						origin: Default::default(),
					};

					let info = Erc20Info {
						address,
						name: T::EVMBridge::name(invoke_context)?,
						symbol: T::EVMBridge::symbol(invoke_context)?,
						decimals: T::EVMBridge::decimals(invoke_context)?,
					};

					*maybe_erc20_info = Some(info);
				}
				Ok(())
			},
		)
	}

	// Returns the EvmAddress associated with a given u32.
	fn get_evm_address(currency_id: u32) -> Option<EvmAddress> {
		CurrencyIdMap::<T>::get(currency_id).map(|v| v.address)
	}

	// Returns the name associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let name = match currency_id {
			CurrencyId::Token(_) => currency_id.name().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let name_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.name),
					DexShare::LiquidCroadloan(lease) => Some(format!("LiquidCroadloan-{}", lease).into_bytes()),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.name)
					}
				}?;
				let name_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.name),
					DexShare::LiquidCroadloan(lease) => Some(format!("LiquidCroadloan-{}", lease).into_bytes()),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.name)
					}
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP "[..]);
				vec.extend_from_slice(&name_0);
				vec.extend_from_slice(&b" - ".to_vec());
				vec.extend_from_slice(&name_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.name),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(lease) => Some(format!("LiquidCroadloan-{}", lease).into_bytes()),
			CurrencyId::ForeignAsset(foreign_asset_id) => AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.name),
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
			CurrencyId::Token(_) => currency_id.symbol().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let token_symbol_0 = match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.symbol),
					DexShare::LiquidCroadloan(lease) => Some(format!("LCDOT-{}", lease).into_bytes()),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.symbol)
					}
				}?;
				let token_symbol_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.symbol),
					DexShare::LiquidCroadloan(lease) => Some(format!("LCDOT-{}", lease).into_bytes()),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.symbol)
					}
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP_"[..]);
				vec.extend_from_slice(&token_symbol_0);
				vec.extend_from_slice(&b"_".to_vec());
				vec.extend_from_slice(&token_symbol_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.symbol),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(lease) => Some(format!("LCDOT-{}", lease).into_bytes()),
			CurrencyId::ForeignAsset(foreign_asset_id) => AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.symbol),
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
			CurrencyId::Token(_) => currency_id.decimals(),
			CurrencyId::DexShare(symbol_0, _) => {
				// initial dex share amount is calculated based on currency_id_0,
				// use the decimals of currency_id_0 as the decimals of lp token.
				match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.decimals),
					DexShare::LiquidCroadloan(_) => CurrencyId::Token(TokenSymbol::DOT).decimals(),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.decimals)
					}
				}
			}
			CurrencyId::Erc20(address) => Pallet::<T>::get_erc20_mapping(address).map(|v| v.decimals),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(_) => CurrencyId::Token(TokenSymbol::DOT).decimals(),
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetMetadatas::<T>::get(foreign_asset_id).map(|v| v.decimals)
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
					DexShare::Erc20(addr) => {
						// ensure erc20 is mapped
						Pallet::<T>::get_erc20_mapping(addr).map(|_| ())?;
					}
					DexShare::Token(_) | DexShare::LiquidCroadloan(_) | DexShare::ForeignAsset(_) => {}
				};
				match right {
					DexShare::Erc20(addr) => {
						// ensure erc20 is mapped
						Pallet::<T>::get_erc20_mapping(addr).map(|_| ())?;
					}
					DexShare::Token(_) | DexShare::LiquidCroadloan(_) | DexShare::ForeignAsset(_) => {}
				};
			}
			CurrencyId::Token(_)
			| CurrencyId::Erc20(_)
			| CurrencyId::StableAssetPoolToken(_)
			| CurrencyId::LiquidCroadloan(_)
			| CurrencyId::ForeignAsset(_) => {}
		};

		EvmAddress::try_from(v).ok()
	}

	// Decode the CurrencyId from EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(addr: EvmAddress) -> Option<CurrencyId> {
		if !is_system_contract(addr) {
			return Some(CurrencyId::Erc20(addr));
		}

		let address = addr.as_bytes();
		let currency_id = match CurrencyIdType::try_from(address[H160_POSITION_CURRENCY_ID_TYPE]).ok()? {
			CurrencyIdType::Token => address[H160_POSITION_TOKEN].try_into().map(CurrencyId::Token).ok(),
			CurrencyIdType::DexShare => {
				let left = match DexShareType::try_from(address[H160_POSITION_DEXSHARE_LEFT_TYPE]).ok()? {
					DexShareType::Token => address[H160_POSITION_DEXSHARE_LEFT_FIELD][3]
						.try_into()
						.map(DexShare::Token)
						.ok(),
					DexShareType::Erc20 => {
						let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_LEFT_FIELD].try_into().ok()?);
						CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
					}
					DexShareType::LiquidCroadloan => {
						let id = Lease::from_be_bytes(address[H160_POSITION_DEXSHARE_LEFT_FIELD].try_into().ok()?);
						Some(DexShare::LiquidCroadloan(id))
					}
					DexShareType::ForeignAsset => {
						let id = ForeignAssetId::from_be_bytes(
							address[H160_POSITION_DEXSHARE_LEFT_FIELD][2..].try_into().ok()?,
						);
						Some(DexShare::ForeignAsset(id))
					}
				}?;
				let right = match DexShareType::try_from(address[H160_POSITION_DEXSHARE_RIGHT_TYPE]).ok()? {
					DexShareType::Token => address[H160_POSITION_DEXSHARE_RIGHT_FIELD][3]
						.try_into()
						.map(DexShare::Token)
						.ok(),
					DexShareType::Erc20 => {
						let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_RIGHT_FIELD].try_into().ok()?);
						CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
					}
					DexShareType::LiquidCroadloan => {
						let id = Lease::from_be_bytes(address[H160_POSITION_DEXSHARE_RIGHT_FIELD].try_into().ok()?);
						Some(DexShare::LiquidCroadloan(id))
					}
					DexShareType::ForeignAsset => {
						let id = ForeignAssetId::from_be_bytes(
							address[H160_POSITION_DEXSHARE_RIGHT_FIELD][2..].try_into().ok()?,
						);
						Some(DexShare::ForeignAsset(id))
					}
				}?;

				Some(CurrencyId::DexShare(left, right))
			}
			CurrencyIdType::StableAsset => None,
			CurrencyIdType::LiquidCroadloan => {
				let id = Lease::from_be_bytes(address[H160_POSITION_LIQUID_CROADLOAN].try_into().ok()?);
				Some(CurrencyId::LiquidCroadloan(id))
			}
			CurrencyIdType::ForeignAsset => {
				let id = ForeignAssetId::from_be_bytes(address[H160_POSITION_FOREIGN_ASSET].try_into().ok()?);
				Some(CurrencyId::ForeignAsset(id))
			}
		};

		// Make sure that every bit of the address is the same
		Self::encode_evm_address(currency_id?).map_or(None, |encoded| if encoded == addr { currency_id } else { None })
	}
}
