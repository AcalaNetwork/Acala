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

//! An orml_authority trait implementation.

use crate::{AccountId, Balance, BlockNumber, CurrencyId};
use frame_support::ensure;
use module_support::{Swap, SwapError, SwapLimit};
use nutsfinance_stable_asset::traits::StableAsset as StableAssetT;
use sp_runtime::DispatchError;

pub struct StableAssetSwapAdaptor<StableAsset>(sp_std::marker::PhantomData<StableAsset>);

impl<StableAsset> Swap<AccountId, Balance, CurrencyId> for StableAssetSwapAdaptor<StableAsset>
where
	StableAsset: StableAssetT<
		AssetId = CurrencyId,
		AtLeast64BitUnsigned = Balance,
		Balance = Balance,
		AccountId = AccountId,
		BlockNumber = BlockNumber,
	>,
	Balance: Clone,
{
	fn get_swap_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(Balance, Balance)> {
		let target_limit = match limit {
			SwapLimit::ExactSupply(_, minimum_target_amount) => minimum_target_amount,
			SwapLimit::ExactTarget(_, exact_target_amount) => exact_target_amount,
		};
		let result = StableAsset::get_best_route(supply_currency_id, target_currency_id, target_limit)?;
		let supply_index = result.assets.iter().position(|&r| r == supply_currency_id)?;
		let target_index = result.assets.iter().position(|&r| r == target_currency_id)?;
		match result.pool_asset {
			CurrencyId::StableAssetPoolToken(stable_asset_id) => StableAsset::get_swap_amount_exact(
				stable_asset_id,
				supply_index as u32,
				target_index as u32,
				target_limit,
			)
			.map(|swap_result| (swap_result.dx, swap_result.dy)),
			_ => None,
		}
	}

	fn swap(
		who: &AccountId,
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		let target_limit = match limit {
			SwapLimit::ExactSupply(_, minimum_target_amount) => minimum_target_amount,
			SwapLimit::ExactTarget(_, exact_target_amount) => exact_target_amount,
		};
		let result = StableAsset::get_best_route(supply_currency_id, target_currency_id, target_limit)
			.ok_or_else(|| Into::<DispatchError>::into(SwapError::CannotSwap))?;
		let supply_index = result
			.assets
			.iter()
			.position(|&r| r == supply_currency_id)
			.ok_or_else(|| Into::<DispatchError>::into(SwapError::CannotSwap))?;
		let target_index = result
			.assets
			.iter()
			.position(|&r| r == target_currency_id)
			.ok_or_else(|| Into::<DispatchError>::into(SwapError::CannotSwap))?;
		match result.pool_asset {
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				let pool_info = StableAsset::pool(stable_asset_id)
					.ok_or_else(|| Into::<DispatchError>::into(SwapError::CannotSwap))?;
				let asset_length = pool_info.assets.len() as u32;

				match limit {
					SwapLimit::ExactSupply(exact_supply, minimum_target_amount) => StableAsset::swap(
						who,
						stable_asset_id,
						supply_index as u32,
						target_index as u32,
						exact_supply,
						minimum_target_amount,
						asset_length,
					),
					SwapLimit::ExactTarget(max_supply_amount, exact_target_amount) => {
						let result = StableAsset::get_swap_amount_exact(
							stable_asset_id,
							supply_index as u32,
							target_index as u32,
							exact_target_amount,
						)
						.ok_or_else(|| Into::<DispatchError>::into(SwapError::CannotSwap))?;
						ensure!(max_supply_amount >= result.dx, SwapError::CannotSwap);
						StableAsset::swap(
							who,
							stable_asset_id,
							supply_index as u32,
							target_index as u32,
							result.dx,
							exact_target_amount,
							asset_length,
						)
					}
				}
			}
			_ => Err(Into::<DispatchError>::into(SwapError::CannotSwap)),
		}
	}
}
