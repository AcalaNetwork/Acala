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
use sp_runtime::traits::Convert;
use sp_runtime::{
	traits::{One, Saturating, Zero},
	ArithmeticError, DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;

use module_support::{Ratio, RebasedStableAssetError};
use nutsfinance_stable_asset::{traits::StableAsset as StableAssetT, StableAssetPoolId};
use orml_traits::MultiCurrency;
use primitives::{AccountId, Amount, Balance, CurrencyId};

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
	pool_id: StableAssetPoolId,
	stable_token_index: u32,
	stable_currency_id: CurrencyId,
	account_id: AccountId,
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

		/// Taiga
		type StableAsset: StableAssetT<
			AssetId = CurrencyId,
			AtLeast64BitUnsigned = Balance,
			Balance = Balance,
			AccountId = Self::AccountId,
			BlockNumber = Self::BlockNumber,
		>;

		/// Currency for transfer assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// The origin.
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
		/// Exceed capacity
		ExceedCapacity,
		/// The Destination is invalid.
		InvalidDestination,
		/// The DistributionParams is invalid
		InvalidDistributionParams,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		UpdateDistributionParams {
			destination: DistributionDestination<T::AccountId>,
			params: DistributionParams,
		},
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if now % T::AdjustPeriod::get() == Zero::zero() {
				DistributionDestinationParams::<T>::iter_keys().for_each(|destination| {
					let _ = Self::do_adjust_to_destination(destination);
				});
			}
			0
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1000)]
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
			DistributionDestinationParams::<T>::insert(destination.clone(), params.clone());

			Self::deposit_event(Event::<T>::UpdateDistributionParams {
				destination: destination.clone(),
				params: params.clone(),
			});

			Ok(())
		}

		#[pallet::weight(1000)]
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
		let params = DistributionDestinationParams::<T>::get(destination.clone())
			.ok_or(Error::<T>::DistributionParamsNotExist)?;
		match destination.clone() {
			DistributionDestination::StableAsset(stable_asset) => {
				let balance = Self::adjust_for_stable_asset(destination.clone(), stable_asset, params)?;

				// update `DistributedBalance` of destination
				DistributedBalance::<T>::try_mutate(destination, |maybe_balance| -> DispatchResult {
					let old_val = maybe_balance.take().unwrap_or_default();
					let new_val = if balance.is_positive() {
						old_val.checked_add(balance as Balance).ok_or(ArithmeticError::Overflow)
					} else {
						old_val
							.checked_sub(balance.abs() as Balance)
							.ok_or(ArithmeticError::Underflow)
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
	fn adjust_for_stable_asset(
		destination: DistributionDestination<T::AccountId>,
		stable_asset: DistributionToStableAsset<T::AccountId>,
		params: DistributionParams,
	) -> Result<Amount, DispatchError> {
		let pool_id = stable_asset.pool_id;
		let pool_info = T::StableAsset::pool(pool_id).ok_or(Error::<T>::InvalidDestination)?;
		let account_id = stable_asset.account_id;

		let total_supply = pool_info.total_supply;
		let ausd_supply = pool_info.balances[stable_asset.stable_token_index as usize];
		let asset_length = pool_info.assets.len();

		let current_rate = Ratio::saturating_from_rational(ausd_supply, total_supply);
		let target_rate = params
			.target_min
			.saturating_add(params.target_max)
			.saturating_mul(Ratio::saturating_from_rational(1, 2));
		let one: Ratio = One::one();
		let remain_rate = one.saturating_sub(target_rate);
		let remain_reci = one.div(remain_rate);
		if current_rate < params.target_min {
			let numerator = target_rate.saturating_mul_int(total_supply).saturating_sub(ausd_supply);
			let mint_amount = remain_reci.saturating_mul_int(numerator).min(params.max_step);
			if let Some(exist_minted) = DistributedBalance::<T>::get(&destination) {
				let newly_amount = mint_amount.saturating_add(exist_minted);
				ensure!(newly_amount < params.capacity, Error::<T>::ExceedCapacity);
			} else {
				ensure!(mint_amount < params.capacity, Error::<T>::ExceedCapacity);
			}
			log::info!(target: "honzon-dist", "current:{}, target:{}, mint:{}", current_rate, target_rate, mint_amount);
			let mut assets = vec![0; asset_length];
			assets[stable_asset.stable_token_index as usize] = mint_amount;
			// deposit stable asset
			T::Currency::deposit(stable_asset.stable_currency_id, &account_id, mint_amount)?;
			// mint to stable asset pool
			T::StableAsset::mint(&account_id, stable_asset.pool_id, assets, 0)?;
			return Ok(mint_amount as Amount);
		} else if current_rate > params.target_max {
			let numerator = ausd_supply.saturating_sub(target_rate.saturating_mul_int(total_supply));
			let burn_amount = remain_reci.saturating_mul_int(numerator).min(params.max_step);
			log::info!(target: "honzon-dist", "current:{}, target:{}, burn:{}", current_rate, target_rate, burn_amount);
			let stable_asset_amount_1 = T::Currency::free_balance(stable_asset.stable_currency_id, &account_id);
			T::StableAsset::redeem_single(
				&account_id,
				stable_asset.pool_id,
				burn_amount,
				stable_asset.stable_token_index,
				0,
				asset_length as u32,
			)?;
			let stable_asset_amount_2 = T::Currency::free_balance(stable_asset.stable_currency_id, &account_id);
			let stable_ed = T::Currency::minimum_balance(stable_asset.stable_currency_id);
			let stable_amount = stable_asset_amount_2
				.saturating_sub(stable_asset_amount_1)
				.saturating_sub(stable_ed);
			T::Currency::withdraw(stable_asset.stable_currency_id, &account_id, stable_amount)?;
			return Ok(0 - stable_amount as Amount);
		}

		Ok(0 as Amount)
	}
}

pub struct RebasedStableAssetErrorConvertor<T>(PhantomData<T>);
impl<T: Config> Convert<RebasedStableAssetError, DispatchError> for RebasedStableAssetErrorConvertor<T> {
	fn convert(e: RebasedStableAssetError) -> DispatchError {
		match e {
			RebasedStableAssetError::InvalidPoolId => Error::<T>::InvalidDestination.into(),
			RebasedStableAssetError::InvalidTokenIndex => Error::<T>::InvalidDestination.into(),
		}
	}
}
