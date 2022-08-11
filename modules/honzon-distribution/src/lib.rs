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
use sp_runtime::traits::CheckedDiv;
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
use primitives::CurrencyId::StableAssetPoolToken;
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
	pub account_id: AccountId,
}

/// Distribution params
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
pub struct DistributionParams {
	// when capacity less than `DistributedBalance`, will redeem and ignore target value.
	pub capacity: Balance,
	// each time maximum amount of mint or burn adjust.
	pub max_step: Balance,
	// when current rate less than this target, mint stable asset to reaching this target.
	pub target_min: Ratio,
	// when current rate large than this target, burn stable asset to reaching this target.
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

		/// Minimum adjust amount, if mint or burn lower than this value, do not adjust.
		type MinimumAdjustAmount: Get<Balance>;

		/// Taiga stable asset protocol.
		type StableAsset: StableAssetT<
			AssetId = CurrencyId,
			AtLeast64BitUnsigned = Balance,
			Balance = Balance,
			AccountId = Self::AccountId,
			BlockNumber = Self::BlockNumber,
		>;

		/// Stable currency used to mint or burn from stable asset pool.
		type GetStableCurrencyId: Get<CurrencyId>;

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
		/// The DistributionParams does not exist.
		DistributionParamsNotExist,
		/// The Destination is invalid.
		InvalidDestination,
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

		#[pallet::weight(T::WeightInfo::remove_distribution())]
		#[transactional]
		pub fn remove_distribution(
			origin: OriginFor<T>,
			destination: DistributionDestination<T::AccountId>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			Self::do_remove_destination(destination)?;
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn update_distributed_balance(
		destination: &DistributionDestination<T::AccountId>,
		balance: Amount,
	) -> DispatchResult {
		// update `DistributedBalance` of destination
		DistributedBalance::<T>::try_mutate(destination, |maybe_balance| -> DispatchResult {
			let old_val = maybe_balance.take().unwrap_or_default();
			let new_val = if balance.is_positive() {
				old_val.saturating_add(balance as Balance)
			} else {
				old_val.saturating_sub(balance.unsigned_abs())
			};
			*maybe_balance = Some(new_val);

			Ok(())
		})?;
		Ok(())
	}

	pub fn do_adjust_to_destination(destination: DistributionDestination<T::AccountId>) -> DispatchResult {
		let params =
			DistributionDestinationParams::<T>::get(&destination).ok_or(Error::<T>::DistributionParamsNotExist)?;
		match destination.clone() {
			DistributionDestination::StableAsset(stable_asset) => {
				let amount = Self::adjust_for_stable_asset(&destination, stable_asset, params)?;
				Self::update_distributed_balance(&destination, amount)?;

				Pallet::<T>::deposit_event(Event::<T>::AdjustDestination { destination, amount });
			}
		}
		Ok(())
	}

	pub fn do_remove_destination(destination: DistributionDestination<T::AccountId>) -> DispatchResult {
		let mut params =
			DistributionDestinationParams::<T>::get(&destination).ok_or(Error::<T>::DistributionParamsNotExist)?;
		// manual set capacity to zero, trigger redeem all stable asset.
		params.capacity = 0;
		match destination.clone() {
			DistributionDestination::StableAsset(stable_asset) => {
				Self::adjust_for_stable_asset(&destination, stable_asset, params)?;
				DistributedBalance::<T>::remove(&destination);
				DistributionDestinationParams::<T>::remove(&destination);
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
		let stable_currency = T::GetStableCurrencyId::get();
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
		let distributed = DistributedBalance::<T>::get(destination).unwrap_or_default();
		let capacity = params.capacity;

		let current_rate = Ratio::saturating_from_rational(*ausd_supply, total_supply);
		let one: Ratio = One::one();
		if current_rate > params.target_max || capacity < distributed {
			// current rate large than target_max, or capacity is less than distributed, burn aUSD
			let burn_amount = if capacity < distributed {
				let remain = distributed.saturating_sub(capacity).min(*ausd_supply);
				let stable_balance = T::Currency::free_balance(StableAssetPoolToken(pool_id), &account_id);
				remain.min(stable_balance)
			} else {
				let target_rate = params.target_max;
				let remain_rate = one.saturating_sub(target_rate);
				let remain_reci = one.checked_div(&remain_rate).ok_or(Error::<T>::InvalidDestination)?;
				let numerator = ausd_supply.saturating_sub(params.target_max.saturating_mul_int(total_supply));
				// if burned amount is large than `distributed`, use `distributed` value as burn amount.
				remain_reci
					.saturating_mul_int(numerator)
					.min(params.max_step)
					.min(distributed)
			};
			if burn_amount < T::MinimumAdjustAmount::get() {
				return Ok(0_i128);
			}

			// redeem stable asset and withdraw aUSD from treasury account.
			let (_, stable_amount) = T::StableAsset::redeem_single(
				&account_id,
				pool_id,
				burn_amount, // this is refer to lp token
				asset_index as u32,
				0,
				asset_length as u32,
			)?;
			// the `stable_amount` may large than burn amount.
			T::Currency::withdraw(stable_currency, &account_id, stable_amount)?;

			log::info!(target: "honzon-dist", "current:{:?}, ausd:{:?}, redeem lp:{:?}, stable:{:?}, distributed:{:?}",
				current_rate, ausd_supply, burn_amount, stable_amount, distributed);
			let burn_amount = 0_i128.saturating_sub(stable_amount as Amount);
			return Ok(burn_amount);
		} else if current_rate < params.target_min {
			// less than target_min, mint aUSD
			let target_rate = params.target_min;
			let remain_rate = one.saturating_sub(target_rate);
			let remain_reci = one.checked_div(&remain_rate).ok_or(Error::<T>::InvalidDestination)?;
			let numerator = target_rate
				.saturating_mul_int(total_supply)
				.saturating_sub(*ausd_supply);
			let mint_amount = remain_reci.saturating_mul_int(numerator).min(params.max_step);

			let mint_amount = if mint_amount.saturating_add(distributed) <= params.capacity {
				mint_amount
			} else {
				params.capacity.saturating_sub(distributed)
			};
			if mint_amount < T::MinimumAdjustAmount::get() {
				return Ok(0_i128);
			}

			// deposit aUSD to treasury account, and then mint to stable asset pool.
			let mut assets = vec![0; asset_length];
			assets[asset_index] = mint_amount;
			T::Currency::deposit(T::GetStableCurrencyId::get(), &account_id, mint_amount)?;
			T::StableAsset::mint(&account_id, pool_id, assets, 0)?;

			log::info!(target: "honzon-dist", "current:{:?}, mint:{:?}, distributed:{:?}",
				current_rate, mint_amount, distributed);
			return Ok(mint_amount as Amount);
		}

		Ok(0_i128)
	}
}
