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

//! # Honzon Distribution Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::MaxEncodedLen;

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{One, Saturating},
	DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;

use module_support::Ratio;
use nutsfinance_stable_asset::{traits::StableAsset as StableAssetT, StableAssetPoolId};
use orml_traits::MultiCurrency;
use primitives::{Amount, Balance, CurrencyId};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum DistributionDestination<AccountId> {
	StableAsset(DistributionToStableAsset<AccountId>),
}

/// Information needed when distribution to StableAsset.
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct DistributionToStableAsset<AccountId> {
	pub pool_id: StableAssetPoolId,
	pub stable_token_index: u32,
	pub stable_currency_id: CurrencyId,
	pub account_id: AccountId,
}

/// Distribution params
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
pub struct DistributionParams {
	pub capacity: Balance,
	pub max_step: Balance,
	pub target_min: Ratio,
	pub target_max: Ratio,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Adjust time period.
		type AdjustPeriod: Get<Self::BlockNumber>;

		/// Adjust time offset.
		type AdjustOffset: Get<Self::BlockNumber>;

		/// Taiga stable asset protocol.
		type StableAsset: StableAssetT<
			AssetId = CurrencyId,
			AtLeast64BitUnsigned = Balance,
			Balance = Balance,
			AccountId = Self::AccountId,
			BlockNumber = Self::BlockNumber,
		>;

		/// Currency for deposit/withdraw assets.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The origin updating params and force adjust.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn distribution_destination_params)]
	pub type DistributionDestinationParams<T: Config> =
		StorageMap<_, Twox64Concat, DistributionDestination<T::AccountId>, DistributionParams, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn distributed_balance)]
	pub type DistributedBalance<T: Config> =
		StorageMap<_, Twox64Concat, DistributionDestination<T::AccountId>, Balance, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// The DistributionParams is not exist.
		DistributionParamsNotExist,
		/// The Destination is invalid.
		InvalidDestination,
		/// The balance is invalid
		InvalidUpdateBalance,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		UpdateDistributionParams {
			destination: DistributionDestination<T::AccountId>,
			params: DistributionParams,
		},
		AdjustDestination {
			destination: DistributionDestination<T::AccountId>,
			stable_currency: CurrencyId,
			amount: Amount,
		},
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if now % T::AdjustPeriod::get() == T::AdjustOffset::get() {
				let mut total_weight: Weight = 0;
				DistributionDestinationParams::<T>::iter_keys().for_each(|destination| {
					let weight = T::WeightInfo::force_adjust();
					let _ = Self::do_adjust_to_destination(destination);
					total_weight += weight;
				});
				total_weight
			} else {
				0
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::update_params())]
		#[transactional]
		pub fn update_params(
			origin: OriginFor<T>,
			destination: DistributionDestination<T::AccountId>,
			capacity: Option<Balance>,
			max_step: Option<Balance>,
			target_min: Option<Ratio>,
			target_max: Option<Ratio>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let mut params = Self::distribution_destination_params(&destination).unwrap_or_default();
			if let Some(capacity) = capacity {
				params.capacity = capacity;
			}
			if let Some(max_step) = max_step {
				params.max_step = max_step;
			}
			if let Some(target_min) = target_min {
				params.target_min = target_min;
			}
			if let Some(target_max) = target_max {
				params.target_max = target_max;
			}
			DistributionDestinationParams::<T>::insert(&destination, &params);

			Self::deposit_event(Event::<T>::UpdateDistributionParams { destination, params });

			Ok(())
		}

		#[pallet::weight(T::WeightInfo::force_adjust())]
		#[transactional]
		pub fn force_adjust(
			origin: OriginFor<T>,
			destination: DistributionDestination<T::AccountId>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			Self::do_adjust_to_destination(destination)?;
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_adjust_to_destination(destination: DistributionDestination<T::AccountId>) -> DispatchResult {
		let params =
			DistributionDestinationParams::<T>::get(&destination).ok_or(Error::<T>::DistributionParamsNotExist)?;
		match destination.clone() {
			DistributionDestination::StableAsset(stable_asset) => {
				let balance = Self::adjust_for_stable_asset(&destination, stable_asset, params)?;

				// update `DistributedBalance` of destination
				DistributedBalance::<T>::try_mutate(destination, |maybe_balance| -> DispatchResult {
					let old_val = maybe_balance.take().unwrap_or_default();
					let new_val = if balance.is_positive() {
						old_val
							.checked_add(balance as Balance)
							.ok_or(Error::<T>::InvalidUpdateBalance)
					} else {
						old_val
							.checked_sub(balance.unsigned_abs() as Balance)
							.ok_or(Error::<T>::InvalidUpdateBalance)
					}?;
					*maybe_balance = Some(new_val);

					Ok(())
				})?;
			}
		}
		Ok(())
	}

	/// if current value less than target_min, mint aUSD:
	///     (balances+x)/(total_supply+x)=target,
	///     x=(target*total_supply-balances)/(1-target)
	/// if current value large than target_max, burn aUSD:
	///     (balances+x)/(total_supply+x)=target,
	///     x=(balances-target*total_supply)/(1-target)
	///
	/// return `Amount` that will be add to or subtract from `DistributedBalance`.
	fn adjust_for_stable_asset(
		destination: &DistributionDestination<T::AccountId>,
		stable_asset: DistributionToStableAsset<T::AccountId>,
		params: DistributionParams,
	) -> Result<Amount, DispatchError> {
		let pool_id = stable_asset.pool_id;
		let pool_info = T::StableAsset::pool(pool_id).ok_or(Error::<T>::InvalidDestination)?;
		let account_id = stable_asset.account_id;
		let asset_length = pool_info.assets.len();
		let asset_index = stable_asset.stable_token_index as usize;
		let total_supply = pool_info.total_supply;
		let ausd_supply = pool_info
			.balances
			.get(asset_index)
			.ok_or(Error::<T>::InvalidDestination)?;
		ensure!(asset_index < asset_length, Error::<T>::InvalidDestination);

		let current_rate = Ratio::saturating_from_rational(*ausd_supply, total_supply);
		let target_rate = params
			.target_min
			.saturating_add(params.target_max)
			.saturating_mul(Ratio::saturating_from_rational(1, 2));
		let one: Ratio = One::one();
		let remain_rate = one.saturating_sub(target_rate);
		let remain_reci = one.div(remain_rate);
		if current_rate < params.target_min {
			let numerator = target_rate
				.saturating_mul_int(total_supply)
				.saturating_sub(*ausd_supply);
			let mint_amount = remain_reci.saturating_mul_int(numerator).min(params.max_step);

			let distributed = DistributedBalance::<T>::get(destination).unwrap_or_default();
			let mint_amount = if mint_amount.saturating_add(distributed) <= params.capacity {
				mint_amount
			} else {
				params.capacity.saturating_sub(distributed)
			};

			log::info!(target: "honzon-dist", "current:{:?}, target:{:?}, mint:{:?}", current_rate, target_rate, mint_amount);
			let mut assets = vec![0; asset_length];
			assets[asset_index] = mint_amount;
			// deposit stable asset to treasury account.
			T::Currency::deposit(stable_asset.stable_currency_id, &account_id, mint_amount)?;
			// use this treasury amount and mint to stable asset pool
			T::StableAsset::mint(&account_id, stable_asset.pool_id, assets, 0)?;

			Pallet::<T>::deposit_event(Event::<T>::AdjustDestination {
				destination: destination.clone(),
				stable_currency: stable_asset.stable_currency_id,
				amount: mint_amount as Amount,
			});
			return Ok(mint_amount as Amount);
		} else if current_rate > params.target_max {
			let numerator = ausd_supply.saturating_sub(target_rate.saturating_mul_int(total_supply));
			let burn_amount = remain_reci.saturating_mul_int(numerator).min(params.max_step);
			log::info!(target: "honzon-dist", "current:{:?}, target:{:?}, burn:{:?}", current_rate, target_rate, burn_amount);
			let (_, stable_amount) = T::StableAsset::redeem_single(
				&account_id,
				stable_asset.pool_id,
				burn_amount,
				asset_index as u32,
				0,
				asset_length as u32,
			)?;
			T::Currency::withdraw(stable_asset.stable_currency_id, &account_id, stable_amount)?;
			let burn_amount = (0 as Amount).saturating_sub(stable_amount as Amount);

			Pallet::<T>::deposit_event(Event::<T>::AdjustDestination {
				destination: destination.clone(),
				stable_currency: stable_asset.stable_currency_id,
				amount: burn_amount,
			});
			return Ok(burn_amount);
		}

		Ok(0 as Amount)
	}
}
