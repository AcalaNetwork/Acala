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
	transactional,
	weights::constants::WEIGHT_PER_SECOND,
	RuntimeDebug,
};
use frame_system::pallet_prelude::*;
use module_support::{AssetIdMapping, EVMBridge, Erc20InfoMapping, InvokeContext};
use primitives::{
	currency::{CurrencyIdType, DexShare, DexShareType, Erc20Id, ForeignAssetId, Lease, StableAssetPoolId, TokenInfo},
	evm::{
		is_system_contract, EvmAddress, H160_POSITION_CURRENCY_ID_TYPE, H160_POSITION_DEXSHARE_LEFT_FIELD,
		H160_POSITION_DEXSHARE_LEFT_TYPE, H160_POSITION_DEXSHARE_RIGHT_FIELD, H160_POSITION_DEXSHARE_RIGHT_TYPE,
		H160_POSITION_FOREIGN_ASSET, H160_POSITION_LIQUID_CROADLOAN, H160_POSITION_STABLE_ASSET, H160_POSITION_TOKEN,
	},
	CurrencyId,
};
use scale_info::{prelude::format, TypeInfo};
use sp_runtime::{traits::One, ArithmeticError, FixedPointNumber, FixedU128};
use sp_std::{boxed::Box, vec::Vec};

// NOTE:v1::MultiLocation is used in storages, we would need to do migration if upgrade the
// MultiLocation in the future.
use xcm::opaque::latest::{prelude::XcmError, AssetId, Fungibility::Fungible, MultiAsset};
use xcm::{v1::MultiLocation, VersionedMultiLocation};
use xcm_builder::TakeRevenue;
use xcm_executor::{traits::WeightTrader, Assets};

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

		/// The Currency ID for the staking currency
		#[pallet::constant]
		type StakingCurrencyId: Get<CurrencyId>;

		/// Evm Bridge for getting info of contracts from the EVM.
		type EVMBridge: EVMBridge<Self::AccountId, BalanceOf<Self>>;

		/// Required origin for registering asset.
		type RegisterOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode, TypeInfo)]
	pub enum AssetIds {
		Erc20(EvmAddress),
		StableAssetId(StableAssetPoolId),
		ForeignAssetId(ForeignAssetId),
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
			asset_address: MultiLocation,
			metadata: AssetMetadata<BalanceOf<T>>,
		},
		/// The foreign asset updated.
		ForeignAssetUpdated {
			asset_id: ForeignAssetId,
			asset_address: MultiLocation,
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

	/// The storages for MultiLocations.
	///
	/// ForeignAssetLocations: map ForeignAssetId => Option<MultiLocation>
	#[pallet::storage]
	#[pallet::getter(fn foreign_asset_locations)]
	pub type ForeignAssetLocations<T: Config> = StorageMap<_, Twox64Concat, ForeignAssetId, MultiLocation, OptionQuery>;

	/// The storages for CurrencyIds.
	///
	/// LocationToCurrencyIds: map MultiLocation => Option<CurrencyId>
	#[pallet::storage]
	#[pallet::getter(fn location_to_currency_ids)]
	pub type LocationToCurrencyIds<T: Config> = StorageMap<_, Twox64Concat, MultiLocation, CurrencyId, OptionQuery>;

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

			Self::deposit_event(Event::<T>::ForeignAssetRegistered {
				asset_id: foreign_asset_id,
				asset_address: location,
				metadata: *metadata,
			});
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

			Self::deposit_event(Event::<T>::ForeignAssetUpdated {
				asset_id: foreign_asset_id,
				asset_address: location,
				metadata: *metadata,
			});
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::register_stable_asset())]
		#[transactional]
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

		#[pallet::weight(T::WeightInfo::update_stable_asset())]
		#[transactional]
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

		#[pallet::weight(T::WeightInfo::register_erc20_asset())]
		#[transactional]
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

		#[pallet::weight(T::WeightInfo::update_erc20_asset())]
		#[transactional]
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
		location: &MultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> Result<ForeignAssetId, DispatchError> {
		let foreign_asset_id = Self::get_next_foreign_asset_id()?;
		LocationToCurrencyIds::<T>::try_mutate(location, |maybe_currency_ids| -> DispatchResult {
			ensure!(maybe_currency_ids.is_none(), Error::<T>::MultiLocationExisted);
			*maybe_currency_ids = Some(CurrencyId::ForeignAsset(foreign_asset_id));

			ForeignAssetLocations::<T>::try_mutate(foreign_asset_id, |maybe_location| -> DispatchResult {
				ensure!(maybe_location.is_none(), Error::<T>::MultiLocationExisted);
				*maybe_location = Some(location.clone());

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
		location: &MultiLocation,
		metadata: &AssetMetadata<BalanceOf<T>>,
	) -> DispatchResult {
		ForeignAssetLocations::<T>::try_mutate(foreign_asset_id, |maybe_multi_locations| -> DispatchResult {
			let old_multi_locations = maybe_multi_locations.as_mut().ok_or(Error::<T>::AssetIdNotExists)?;

			AssetMetadatas::<T>::try_mutate(
				AssetIds::ForeignAssetId(foreign_asset_id),
				|maybe_asset_metadatas| -> DispatchResult {
					ensure!(maybe_asset_metadatas.is_some(), Error::<T>::AssetIdNotExists);

					// modify location
					if location != old_multi_locations {
						LocationToCurrencyIds::<T>::remove(old_multi_locations.clone());
						LocationToCurrencyIds::<T>::try_mutate(location, |maybe_currency_ids| -> DispatchResult {
							ensure!(maybe_currency_ids.is_none(), Error::<T>::MultiLocationExisted);
							*maybe_currency_ids = Some(CurrencyId::ForeignAsset(foreign_asset_id));
							Ok(())
						})?;
					}
					*maybe_asset_metadatas = Some(metadata.clone());
					*old_multi_locations = location.clone();
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
}

pub struct AssetIdMaps<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> AssetIdMapping<StableAssetPoolId, ForeignAssetId, MultiLocation, AssetMetadata<BalanceOf<T>>>
	for AssetIdMaps<T>
{
	fn get_erc20_asset_metadata(contract: EvmAddress) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(AssetIds::Erc20(contract))
	}

	fn get_stable_asset_metadata(stable_asset_id: StableAssetPoolId) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(AssetIds::StableAssetId(stable_asset_id))
	}

	fn get_foreign_asset_metadata(foreign_asset_id: ForeignAssetId) -> Option<AssetMetadata<BalanceOf<T>>> {
		Pallet::<T>::asset_metadatas(AssetIds::ForeignAssetId(foreign_asset_id))
	}

	fn get_multi_location(foreign_asset_id: ForeignAssetId) -> Option<MultiLocation> {
		Pallet::<T>::foreign_asset_locations(foreign_asset_id)
	}

	fn get_currency_id(multi_location: MultiLocation) -> Option<CurrencyId> {
		Pallet::<T>::location_to_currency_ids(multi_location)
	}
}

/// Simple fee calculator that requires payment in a single fungible at a fixed rate.
///
/// The constant `FixedRate` type parameter should be the concrete fungible ID and the amount of it
/// required for one second of weight.
pub struct FixedRateOfForeignAsset<T, FixedRate: Get<u128>, R: TakeRevenue> {
	weight: Weight,
	amount: u128,
	ed_ratio: FixedU128,
	multi_location: Option<MultiLocation>,
	_marker: PhantomData<(T, FixedRate, R)>,
}

impl<T: Config, FixedRate: Get<u128>, R: TakeRevenue> WeightTrader for FixedRateOfForeignAsset<T, FixedRate, R>
where
	BalanceOf<T>: Into<u128>,
{
	fn new() -> Self {
		Self {
			weight: 0,
			amount: 0,
			ed_ratio: Default::default(),
			multi_location: None,
			_marker: PhantomData,
		}
	}

	fn buy_weight(&mut self, weight: Weight, payment: Assets) -> Result<Assets, XcmError> {
		log::trace!(target: "asset-registry::weight", "buy_weight weight: {:?}, payment: {:?}", weight, payment);

		// only support first fungible assets now.
		let asset_id = payment
			.fungible
			.iter()
			.next()
			.map_or(Err(XcmError::TooExpensive), |v| Ok(v.0))?;

		if let AssetId::Concrete(ref multi_location) = asset_id {
			log::debug!(target: "asset-registry::weight", "buy_weight multi_location: {:?}", multi_location);

			if let Some(CurrencyId::ForeignAsset(foreign_asset_id)) =
				Pallet::<T>::location_to_currency_ids(multi_location.clone())
			{
				if let Some(asset_metadatas) = Pallet::<T>::asset_metadatas(AssetIds::ForeignAssetId(foreign_asset_id))
				{
					// The integration tests can ensure the ed is non-zero.
					let ed_ratio = FixedU128::saturating_from_rational(
						asset_metadatas.minimal_balance.into(),
						T::Currency::minimum_balance().into(),
					);
					// The WEIGHT_PER_SECOND is non-zero.
					let weight_ratio = FixedU128::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
					let amount = ed_ratio.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

					let required = MultiAsset {
						id: asset_id.clone(),
						fun: Fungible(amount),
					};

					log::trace!(
						target: "asset-registry::weight", "buy_weight payment: {:?}, required: {:?}, fixed_rate: {:?}, ed_ratio: {:?}, weight_ratio: {:?}",
						payment, required, FixedRate::get(), ed_ratio, weight_ratio
					);
					let unused = payment
						.clone()
						.checked_sub(required)
						.map_err(|_| XcmError::TooExpensive)?;
					self.weight = self.weight.saturating_add(weight);
					self.amount = self.amount.saturating_add(amount);
					self.ed_ratio = ed_ratio;
					self.multi_location = Some(multi_location.clone());
					return Ok(unused);
				}
			}
		}

		log::trace!(target: "asset-registry::weight", "no concrete fungible asset");
		Err(XcmError::TooExpensive)
	}

	fn refund_weight(&mut self, weight: Weight) -> Option<MultiAsset> {
		log::trace!(
			target: "asset-registry::weight", "refund_weight weight: {:?}, weight: {:?}, amount: {:?}, ed_ratio: {:?}, multi_location: {:?}",
			weight, self.weight, self.amount, self.ed_ratio, self.multi_location
		);
		let weight = weight.min(self.weight);
		let weight_ratio = FixedU128::saturating_from_rational(weight as u128, WEIGHT_PER_SECOND as u128);
		let amount = self
			.ed_ratio
			.saturating_mul_int(weight_ratio.saturating_mul_int(FixedRate::get()));

		self.weight = self.weight.saturating_sub(weight);
		self.amount = self.amount.saturating_sub(amount);

		log::trace!(target: "asset-registry::weight", "refund_weight amount: {:?}", amount);
		if amount > 0 && self.multi_location.is_some() {
			Some(
				(
					self.multi_location.as_ref().expect("checked is non-empty; qed").clone(),
					amount,
				)
					.into(),
			)
		} else {
			None
		}
	}
}

impl<T, FixedRate: Get<u128>, R: TakeRevenue> Drop for FixedRateOfForeignAsset<T, FixedRate, R> {
	fn drop(&mut self) {
		log::trace!(target: "asset-registry::weight", "take revenue, weight: {:?}, amount: {:?}, multi_location: {:?}", self.weight, self.amount, self.multi_location);
		if self.amount > 0 && self.multi_location.is_some() {
			R::take_revenue(
				(
					self.multi_location.as_ref().expect("checked is non-empty; qed").clone(),
					self.amount,
				)
					.into(),
			);
		}
	}
}

pub struct EvmErc20InfoMapping<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Erc20InfoMapping for EvmErc20InfoMapping<T> {
	// Returns the name associated with a given CurrencyId.
	// If CurrencyId is CurrencyId::DexShare and contain DexShare::Erc20,
	// the EvmAddress must have been mapped.
	fn name(currency_id: CurrencyId) -> Option<Vec<u8>> {
		let name = match currency_id {
			CurrencyId::Token(_) => currency_id.name().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let name_0 = match symbol_0 {
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
				}?;
				let name_1 = match symbol_1 {
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
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP "[..]);
				vec.extend_from_slice(&name_0);
				vec.extend_from_slice(&b" - ".to_vec());
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
			CurrencyId::Token(_) => currency_id.symbol().map(|v| v.as_bytes().to_vec()),
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				let token_symbol_0 = match symbol_0 {
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
				}?;
				let token_symbol_1 = match symbol_1 {
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
				}?;

				let mut vec = Vec::new();
				vec.extend_from_slice(&b"LP_"[..]);
				vec.extend_from_slice(&token_symbol_0);
				vec.extend_from_slice(&b"_".to_vec());
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
			CurrencyId::Token(_) => currency_id.decimals(),
			CurrencyId::DexShare(symbol_0, _) => {
				// initial dex share amount is calculated based on currency_id_0,
				// use the decimals of currency_id_0 as the decimals of lp token.
				match symbol_0 {
					DexShare::Token(symbol) => CurrencyId::Token(symbol).decimals(),
					DexShare::Erc20(address) => AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|v| v.decimals),
					DexShare::LiquidCrowdloan(_) => T::StakingCurrencyId::get().decimals(),
					DexShare::ForeignAsset(foreign_asset_id) => {
						AssetMetadatas::<T>::get(AssetIds::ForeignAssetId(foreign_asset_id)).map(|v| v.decimals)
					}
				}
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
					DexShare::Token(_) | DexShare::LiquidCrowdloan(_) | DexShare::ForeignAsset(_) => {}
				};
				match right {
					DexShare::Erc20(address) => {
						// ensure erc20 is mapped
						AssetMetadatas::<T>::get(AssetIds::Erc20(address)).map(|_| ())?;
					}
					DexShare::Token(_) | DexShare::LiquidCrowdloan(_) | DexShare::ForeignAsset(_) => {}
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
						Erc20IdToAddress::<T>::get(id).map(DexShare::Erc20)
					}
					DexShareType::LiquidCrowdloan => {
						let id = Lease::from_be_bytes(address[H160_POSITION_DEXSHARE_LEFT_FIELD].try_into().ok()?);
						Some(DexShare::LiquidCrowdloan(id))
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
						Erc20IdToAddress::<T>::get(id).map(DexShare::Erc20)
					}
					DexShareType::LiquidCrowdloan => {
						let id = Lease::from_be_bytes(address[H160_POSITION_DEXSHARE_RIGHT_FIELD].try_into().ok()?);
						Some(DexShare::LiquidCrowdloan(id))
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
