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
	RuntimeDebug,
};
use frame_system::pallet_prelude::*;
use module_support::{CurrencyIdMapping, EVMBridge, ForeignAssetIdMapping, InvokeContext};
use primitives::{
	currency::TokenInfo,
	evm::{Erc20Info, EvmAddress},
	CurrencyId, DexShare, H160_POSITION_DEXSHARE_LEFT, H160_POSITION_DEXSHARE_RIGHT, H160_POSITION_TOKEN,
	H160_PREFIX_DEXSHARE, H160_PREFIX_TOKEN,
};
use scale_info::TypeInfo;
use sp_runtime::{traits::One, ArithmeticError};
use sp_std::{
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
pub type ForeignAssetId = u16;

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
		/// AssetMetadata already existed
		AssetMetadataExisted,
		/// AssetMetadata not exists
		AssetMetadataNotExists,
		/// CurrencyId existed
		CurrencyIdExisted,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Registered foreign asset. \[ForeignAssetId, AssetMetadata\]
		RegisteredForeignAsset(ForeignAssetId, AssetMetadata<BalanceOf<T>>),
		/// Updated foreign asset. \[AssetMetadata\]
		UpdatedForeignAsset(AssetMetadata<BalanceOf<T>>),
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

	/// The storages for AssetMetadatas.
	///
	/// AssetMetadatas: map MultiLocation => Option<AssetMetadata>
	#[pallet::storage]
	#[pallet::getter(fn asset_metadatas)]
	pub type AssetMetadatas<T: Config> =
		StorageMap<_, Twox64Concat, MultiLocation, AssetMetadata<BalanceOf<T>>, OptionQuery>;

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
		#[pallet::weight(1000)]
		pub fn register_foreign_asset(
			origin: OriginFor<T>,
			location: VersionedMultiLocation,
			metadata: AssetMetadata<BalanceOf<T>>,
		) -> DispatchResult {
			T::RegisterOrigin::ensure_origin(origin)?;
			let foreign_asset_id = Self::do_register_foreign_asset(&location, &metadata)?;

			Self::deposit_event(Event::<T>::RegisteredForeignAsset(foreign_asset_id, metadata));
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
	fn get_next_foreign_asset_id() -> Result<ForeignAssetId, DispatchError> {
		NextForeignAssetId::<T>::mutate(|current| -> Result<ForeignAssetId, DispatchError> {
			let id = *current;
			*current = current.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	fn do_register_foreign_asset(
		location: &VersionedMultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> Result<ForeignAssetId, DispatchError> {
		let location: MultiLocation = location.clone().try_into().map_err(|()| Error::<T>::BadLocation)?;

		AssetMetadatas::<T>::mutate(
			location.clone(),
			|maybe_asset_metadatas| -> Result<ForeignAssetId, DispatchError> {
				ensure!(maybe_asset_metadatas.is_none(), Error::<T>::AssetMetadataExisted);

				let id = Self::get_next_foreign_asset_id()?;
				MultiLocations::<T>::insert(id, location);

				*maybe_asset_metadatas = Some(metadata.clone());

				Ok(id)
			},
		)
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

pub struct XcmForeignAssetIdMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> ForeignAssetIdMapping<ForeignAssetId, AssetMetadata<BalanceOf<T>>> for XcmForeignAssetIdMapping<T> {
	fn get_asset_metadata(foreign_asset_id: ForeignAssetId) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(Pallet::<T>::multi_locations(foreign_asset_id)?)
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
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.name),
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;
				let name_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).name().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_1))
						.filter(|v| v.address == address)
						.map(|v| v.name),
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP "[..]);
				vec.extend_from_slice(&name_0);
				vec.extend_from_slice(&b" - ".to_vec());
				vec.extend_from_slice(&name_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.name),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(_) => None,
			CurrencyId::ForeignAsset(_) => None,
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
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.symbol),
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;
				let token_symbol_1 = match symbol_1 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).symbol().map(|v| v.as_bytes().to_vec()),
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_1))
						.filter(|v| v.address == address)
						.map(|v| v.symbol),
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP_"[..]);
				vec.extend_from_slice(&token_symbol_0);
				vec.extend_from_slice(&b"_".to_vec());
				vec.extend_from_slice(&token_symbol_1);
				Some(vec)
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.symbol),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(_) => None,
			CurrencyId::ForeignAsset(_) => None,
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
					DexShare::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(symbol_0))
						.filter(|v| v.address == address)
						.map(|v| v.decimals),
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}
			}
			CurrencyId::Erc20(address) => CurrencyIdMap::<T>::get(Into::<u32>::into(DexShare::Erc20(address)))
				.filter(|v| v.address == address)
				.map(|v| v.decimals),
			CurrencyId::StableAssetPoolToken(_) => None,
			CurrencyId::LiquidCroadloan(_) => None,
			CurrencyId::ForeignAsset(_) => None,
		}
	}

	// Encode the CurrencyId to EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn encode_evm_address(v: CurrencyId) -> Option<EvmAddress> {
		match v {
			CurrencyId::DexShare(left, right) => {
				let symbol_0 = match left {
					DexShare::Token(_) => Some(left.into()),
					DexShare::Erc20(address) => {
						let id: u32 = left.into();
						CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).map(|_| id)
					}
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;
				let symbol_1 = match right {
					DexShare::Token(_) => Some(right.into()),
					DexShare::Erc20(address) => {
						let id: u32 = right.into();
						CurrencyIdMap::<T>::get(id).filter(|v| v.address == address).map(|_| id)
					}
					DexShare::LiquidCroadloan(_) => {
						unimplemented!()
					}
					DexShare::ForeignAsset(_) => {
						unimplemented!()
					}
				}?;

				let mut prefix = EvmAddress::default();
				prefix[0..H160_PREFIX_DEXSHARE.len()].copy_from_slice(&H160_PREFIX_DEXSHARE);
				Some(prefix | EvmAddress::from_low_u64_be(u64::from(symbol_0) << 32 | u64::from(symbol_1)))
			}

			// Token or Erc20
			_ => EvmAddress::try_from(v).ok(),
		}
	}

	// Decode the CurrencyId from EvmAddress.
	// If is CurrencyId::DexShare and contain DexShare::Erc20,
	// will use the u32 to get the DexShare::Erc20 from the mapping.
	fn decode_evm_address(addr: EvmAddress) -> Option<CurrencyId> {
		let address = addr.as_bytes();

		// Token
		if address.starts_with(&H160_PREFIX_TOKEN) {
			return address[H160_POSITION_TOKEN].try_into().map(CurrencyId::Token).ok();
		}

		// DexShare
		if address.starts_with(&H160_PREFIX_DEXSHARE) {
			let left = {
				if address[H160_POSITION_DEXSHARE_LEFT].starts_with(&[0u8; 3]) {
					// Token
					address[H160_POSITION_DEXSHARE_LEFT][3]
						.try_into()
						.map(DexShare::Token)
						.ok()
				} else {
					// Erc20
					let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_LEFT].try_into().ok()?);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;
			let right = {
				if address[H160_POSITION_DEXSHARE_RIGHT].starts_with(&[0u8; 3]) {
					// Token
					address[H160_POSITION_DEXSHARE_RIGHT][3]
						.try_into()
						.map(DexShare::Token)
						.ok()
				} else {
					// Erc20
					let id = u32::from_be_bytes(address[H160_POSITION_DEXSHARE_RIGHT].try_into().ok()?);
					CurrencyIdMap::<T>::get(id).map(|v| DexShare::Erc20(v.address))
				}
			}?;

			return Some(CurrencyId::DexShare(left, right));
		}

		// Erc20
		let id = Into::<u32>::into(DexShare::Erc20(addr));
		CurrencyIdMap::<T>::get(id).map(|v| CurrencyId::Erc20(v.address))
	}
}
