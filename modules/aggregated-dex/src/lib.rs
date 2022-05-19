// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

//! # Aggregated DEX Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::type_complexity)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use nutsfinance_stable_asset::{traits::StableAsset as StableAssetT, PoolTokenIndex, StableAssetPoolId};
use primitives::{Balance, CurrencyId};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::traits::Zero;
use sp_std::{marker::PhantomData, vec::Vec};
use support::{DEXManager, Swap, SwapLimit};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[derive(Encode, Decode, Eq, PartialEq, Clone, RuntimeDebug, PartialOrd, Ord, TypeInfo)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum SwapPath {
	Dex(Vec<CurrencyId>),
	Taiga(StableAssetPoolId, PoolTokenIndex, PoolTokenIndex),
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// DEX
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		/// Taiga
		type StableAsset: StableAssetT<
			AssetId = CurrencyId,
			AtLeast64BitUnsigned = Balance,
			Balance = Balance,
			AccountId = Self::AccountId,
			BlockNumber = Self::BlockNumber,
		>;

		/// The alternative swap path joint list for DEX swap
		type DexSwapJointList: Get<Vec<Vec<CurrencyId>>>;

		/// The limit for length of swap path
		#[pallet::constant]
		type SwapPathLimit: Get<u32>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Cannot swap.
		CannotSwap,
		/// The stable asset pool id of Taiga is invalid.
		InvalidPoolId,
		/// The asset index of stable asset pool is invalid.
		InvalidTokenIndex,
		/// The SwapPath is invalid.
		InvalidSwapPath,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Swap with aggregated DEX at exact supply amount.
		///
		/// - `paths`: aggregated swap path.
		/// - `supply_amount`: exact supply amount.
		/// - `min_target_amount`: acceptable minimum target amount.
		#[pallet::weight(<T as Config>::WeightInfo::swap_with_exact_supply(
			paths.iter().fold(0, |u, swap_path| match swap_path {
				SwapPath::Dex(v) => u + (v.len() as u32),
				SwapPath::Taiga(_, _, _) => u + 1
			})
		))]
		#[transactional]
		pub fn swap_with_exact_supply(
			origin: OriginFor<T>,
			paths: frame_support::BoundedVec<SwapPath, T::SwapPathLimit>,
			supply_amount: Balance,
			min_target_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let _ = Self::do_aggregated_swap(
				&who,
				paths.into(),
				SwapLimit::ExactSupply(supply_amount, min_target_amount),
			)?;
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Swap by the the swap aggregated by DEX and Taiga.
	/// Note: TaigaSwap dosen't support ExactTarget swap yet, so just the swap at `ExactSupply`
	/// works.
	#[transactional]
	fn do_aggregated_swap(
		who: &T::AccountId,
		paths: Vec<SwapPath>,
		swap_limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		ensure!(!paths.is_empty(), Error::<T>::InvalidSwapPath);

		match swap_limit {
			SwapLimit::ExactSupply(exact_supply_amount, min_target_amount) => {
				let mut previous_output_currency_id: Option<CurrencyId> = None;
				let mut output_amount: Balance = exact_supply_amount;

				for path in paths {
					match path {
						SwapPath::Dex(dex_path) => {
							let input_currency_id = dex_path.first().ok_or(Error::<T>::InvalidSwapPath)?;
							let output_currency_id = dex_path.last().ok_or(Error::<T>::InvalidSwapPath)?;

							// If there has been a swap before,
							// the currency id of this swap must be the output currency id of the previous swap.
							if let Some(currency_id) = previous_output_currency_id {
								ensure!(currency_id == *input_currency_id, Error::<T>::InvalidSwapPath);
							}

							// use the output of the previous swap as input.
							let (_, actual_target) = T::DEX::swap_with_specific_path(
								who,
								&dex_path,
								SwapLimit::ExactSupply(output_amount, Zero::zero()),
							)?;

							previous_output_currency_id = Some(*output_currency_id);
							output_amount = actual_target;
						}
						SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
							let pool_info = T::StableAsset::pool(pool_id).ok_or(Error::<T>::InvalidPoolId)?;
							let input_currency_id = pool_info
								.assets
								.get(supply_asset_index as usize)
								.ok_or(Error::<T>::InvalidTokenIndex)?;
							let output_currency_id = pool_info
								.assets
								.get(target_asset_index as usize)
								.ok_or(Error::<T>::InvalidTokenIndex)?;

							// If there has been a swap before,
							// the currency id of this swap must be the output currency id of the previous swap.
							if let Some(currency_id) = previous_output_currency_id {
								ensure!(currency_id == *input_currency_id, Error::<T>::InvalidSwapPath);
							}

							let asset_length = pool_info.assets.len() as u32;

							// use the output of the previous swap as input.
							let (_, actual_target) = T::StableAsset::swap(
								who,
								pool_id,
								supply_asset_index,
								target_asset_index,
								output_amount,
								Zero::zero(),
								asset_length,
							)?;

							previous_output_currency_id = Some(*output_currency_id);
							output_amount = actual_target;
						}
					}
				}

				// the result must meet the swap_limit.
				ensure!(output_amount >= min_target_amount, Error::<T>::CannotSwap);

				Ok((exact_supply_amount, output_amount))
			}
			SwapLimit::ExactTarget(max_supply_amount, exact_target_amount) => {
				let mut previous_input_currency_id: Option<CurrencyId> = None;
				let mut input_amount: Balance = exact_target_amount;

				for path in paths.iter().rev() {
					match path {
						SwapPath::Dex(dex_path) => {
							let output_currency_id = dex_path.last().ok_or(Error::<T>::InvalidSwapPath)?;
							let input_currency_id = dex_path.first().ok_or(Error::<T>::InvalidSwapPath)?;

							if let Some(currency_id) = previous_input_currency_id {
								ensure!(currency_id == *output_currency_id, Error::<T>::InvalidSwapPath);
							}

							// calculate the supply amount
							let (supply_amount, _) = T::DEX::get_swap_amount(
								dex_path,
								SwapLimit::ExactTarget(Balance::max_value(), input_amount),
							)
							.ok_or(Error::<T>::CannotSwap)?;

							previous_input_currency_id = Some(*input_currency_id);
							input_amount = supply_amount;
						}
						SwapPath::Taiga(pool_id, supply_asset_index, target_asset_index) => {
							let pool_info = T::StableAsset::pool(*pool_id).ok_or(Error::<T>::InvalidPoolId)?;
							let input_currency_id = pool_info
								.assets
								.get(*supply_asset_index as usize)
								.ok_or(Error::<T>::InvalidTokenIndex)?;
							let output_currency_id = pool_info
								.assets
								.get(*target_asset_index as usize)
								.ok_or(Error::<T>::InvalidTokenIndex)?;

							if let Some(currency_id) = previous_input_currency_id {
								ensure!(currency_id == *output_currency_id, Error::<T>::InvalidSwapPath);
							}

							let swap_result = T::StableAsset::get_swap_input_amount(
								*pool_id,
								*supply_asset_index,
								*target_asset_index,
								input_amount,
							)
							.ok_or(Error::<T>::CannotSwap)?;

							previous_input_currency_id = Some(*input_currency_id);
							input_amount = swap_result.dx;
						}
					}
				}

				// the result must meet the swap_limit.
				ensure!(
					!input_amount.is_zero() && input_amount <= max_supply_amount,
					Error::<T>::CannotSwap
				);

				// actually swap by `ExactSupply` limit
				Self::do_aggregated_swap(who, paths, SwapLimit::ExactSupply(input_amount, exact_target_amount))
			}
		}
	}
}

/// Swap by Acala DEX which has specific joints.
pub struct DexSwap<T>(PhantomData<T>);
impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for DexSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		T::DEX::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit,
			T::DexSwapJointList::get(),
		)
		.map(|(_, supply_amount, target_amount)| (supply_amount, target_amount))
	}

	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let path = T::DEX::get_best_price_swap_path(
			supply_currency_id,
			target_currency_id,
			limit,
			T::DexSwapJointList::get(),
		)
		.ok_or(Error::<T>::CannotSwap)?
		.0;

		T::DEX::swap_with_specific_path(who, &path, limit)
	}
}

/// Swap by Taiga pool.
pub struct TaigaSwap<T>(PhantomData<T>);
impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for TaigaSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		match limit {
			SwapLimit::ExactSupply(supply_amount, min_target_amount) => {
				let (pool_id, input_index, output_index, _) =
					T::StableAsset::get_best_route(supply_currency_id, target_currency_id, supply_amount)?;

				if let Some(swap_result) =
					T::StableAsset::get_swap_output_amount(pool_id, input_index, output_index, supply_amount)
				{
					if swap_result.dy >= min_target_amount {
						return Some((swap_result.dx, swap_result.dy));
					}
				}
			}
			SwapLimit::ExactTarget(max_supply_amount, target_amount) => {
				let (pool_id, input_index, output_index, _) =
					T::StableAsset::get_best_route(supply_currency_id, target_currency_id, max_supply_amount)?;

				if let Some(swap_result) =
					T::StableAsset::get_swap_input_amount(pool_id, input_index, output_index, target_amount)
				{
					if !swap_result.dx.is_zero() && swap_result.dx <= max_supply_amount {
						return Some((swap_result.dx, swap_result.dy));
					}
				}
			}
		};

		None
	}

	#[transactional]
	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let (supply_amount, min_target_amount) = match limit {
			SwapLimit::ExactSupply(supply_amount, min_target_amount) => (supply_amount, min_target_amount),
			SwapLimit::ExactTarget(_, target_amount) => {
				let (supply_amount, _) = Self::get_swap_amount(supply_currency_id, target_currency_id, limit)
					.ok_or(Error::<T>::CannotSwap)?;
				(supply_amount, target_amount)
			}
		};

		let (pool_id, input_index, output_index, _) =
			T::StableAsset::get_best_route(supply_currency_id, target_currency_id, min_target_amount)
				.ok_or(Error::<T>::CannotSwap)?;
		let pool_info = T::StableAsset::pool(pool_id).ok_or(Error::<T>::InvalidPoolId)?;
		let asset_length = pool_info.assets.len() as u32;

		let (actual_supply, actual_target) = T::StableAsset::swap(
			who,
			pool_id,
			input_index,
			output_index,
			supply_amount,
			min_target_amount,
			asset_length,
		)?;
		ensure!(actual_target >= min_target_amount, Error::<T>::CannotSwap);

		Ok((actual_supply, actual_target))
	}
}

/// Choose DEX or Taiga to fully execute the swap by which price is better.
pub struct EitherDexOrTaigaSwap<T>(PhantomData<T>);

pub struct DexOrTaigaSwapParams {
	pub dex_result: Option<(Balance, Balance)>,
	pub taiga_result: Option<(Balance, Balance)>,
	pub swap_amount: Option<(Balance, Balance)>,
}

impl<T: Config> EitherDexOrTaigaSwap<T> {
	fn get_swap_params(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> DexOrTaigaSwapParams {
		let dex_result = DexSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let taiga_result = TaigaSwap::<T>::get_swap_amount(supply_currency_id, target_currency_id, limit);
		let swap_amount =
			if let (Some((dex_supply, dex_target)), Some((taiga_supply, taiga_target))) = (dex_result, taiga_result) {
				match limit {
					SwapLimit::ExactSupply(_, _) => {
						if taiga_target > dex_target {
							taiga_result
						} else {
							dex_result
						}
					}
					SwapLimit::ExactTarget(_, _) => {
						if taiga_supply < dex_supply {
							taiga_result
						} else {
							dex_result
						}
					}
				}
			} else {
				dex_result.or(taiga_result)
			};

		DexOrTaigaSwapParams {
			dex_result,
			taiga_result,
			swap_amount,
		}
	}
}

impl<T: Config> Swap<T::AccountId, Balance, CurrencyId> for EitherDexOrTaigaSwap<T> {
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		Self::get_swap_params(supply_currency_id, target_currency_id, limit).swap_amount
	}

	fn swap(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let DexOrTaigaSwapParams {
			dex_result,
			taiga_result,
			swap_amount,
		} = Self::get_swap_params(supply_currency_id, target_currency_id, limit);

		if swap_amount.is_some() {
			if dex_result == swap_amount {
				return DexSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			} else if taiga_result == swap_amount {
				return TaigaSwap::<T>::swap(who, supply_currency_id, target_currency_id, limit);
			}
		}

		Err(Error::<T>::CannotSwap.into())
	}
}

/// TODO:
pub struct AggregatedSwap<T>(PhantomData<T>);
