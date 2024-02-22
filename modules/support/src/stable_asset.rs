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

use nutsfinance_stable_asset::{
	traits::StableAsset as StableAssetT, PoolTokenIndex, RedeemProportionResult, StableAssetPoolId,
	StableAssetPoolInfo, SwapResult,
};
use orml_tokens::ConvertBalance;
use sp_runtime::{
	traits::{Bounded, Convert},
	DispatchError, DispatchResult,
};
use sp_std::vec::Vec;

pub enum RebasedStableAssetError {
	InvalidPoolId,
	InvalidTokenIndex,
}

pub struct RebasedStableAsset<StableAsset, RebaseTokenAmountConvertor, ErrorConvertor>(
	sp_std::marker::PhantomData<(StableAsset, RebaseTokenAmountConvertor, ErrorConvertor)>,
);

impl<AccountId, Balance, BlockNumber, CurrencyId, StableAsset, RebaseTokenAmountConvertor, ErrorConvertor> StableAssetT
	for RebasedStableAsset<StableAsset, RebaseTokenAmountConvertor, ErrorConvertor>
where
	StableAsset: StableAssetT<
		AssetId = CurrencyId,
		AtLeast64BitUnsigned = Balance,
		Balance = Balance,
		AccountId = AccountId,
		BlockNumber = BlockNumber,
	>,
	RebaseTokenAmountConvertor: ConvertBalance<Balance, Balance, AssetId = CurrencyId>,
	ErrorConvertor: Convert<RebasedStableAssetError, DispatchError>,
	CurrencyId: Copy,
	Balance: Copy + Bounded,
{
	type AssetId = CurrencyId;
	type AtLeast64BitUnsigned = Balance;
	type Balance = Balance;
	type AccountId = AccountId;
	type BlockNumber = BlockNumber;

	fn pool_count() -> StableAssetPoolId {
		StableAsset::pool_count()
	}

	fn pool(
		id: StableAssetPoolId,
	) -> Option<
		StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	> {
		StableAsset::pool(id)
	}

	fn create_pool(
		pool_asset: Self::AssetId,
		assets: Vec<Self::AssetId>,
		precisions: Vec<Self::AtLeast64BitUnsigned>,
		mint_fee: Self::AtLeast64BitUnsigned,
		swap_fee: Self::AtLeast64BitUnsigned,
		redeem_fee: Self::AtLeast64BitUnsigned,
		initial_a: Self::AtLeast64BitUnsigned,
		fee_recipient: Self::AccountId,
		yield_recipient: Self::AccountId,
		precision: Self::AtLeast64BitUnsigned,
	) -> DispatchResult {
		StableAsset::create_pool(
			pool_asset,
			assets,
			precisions,
			mint_fee,
			swap_fee,
			redeem_fee,
			initial_a,
			fee_recipient,
			yield_recipient,
			precision,
		)
	}

	fn mint(
		who: &Self::AccountId,
		pool_id: StableAssetPoolId,
		amounts: Vec<Self::Balance>,
		min_mint_amount: Self::Balance,
	) -> DispatchResult {
		let pool_info = StableAsset::pool(pool_id)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidPoolId))?;
		let rebased_amounts = amounts
			.iter()
			.enumerate()
			.map(|(index, amount)| {
				if let Some(currency_id) = pool_info.assets.get(index) {
					RebaseTokenAmountConvertor::convert_balance(*amount, *currency_id)
				} else {
					Ok(*amount)
				}
			})
			.collect::<Result<Vec<_>, _>>()?;

		StableAsset::mint(who, pool_id, rebased_amounts, min_mint_amount)
	}

	fn swap(
		who: &Self::AccountId,
		pool_id: StableAssetPoolId,
		i: PoolTokenIndex,
		j: PoolTokenIndex,
		dx: Self::Balance,
		min_dy: Self::Balance,
		asset_length: u32,
	) -> sp_std::result::Result<(Self::Balance, Self::Balance), DispatchError> {
		let pool_info = StableAsset::pool(pool_id)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidPoolId))?;
		let input_currency_id = pool_info
			.assets
			.get(i as usize)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidTokenIndex))?;
		let output_currency_id = pool_info
			.assets
			.get(j as usize)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidTokenIndex))?;

		StableAsset::swap(
			who,
			pool_id,
			i,
			j,
			RebaseTokenAmountConvertor::convert_balance(dx, *input_currency_id)?,
			RebaseTokenAmountConvertor::convert_balance(min_dy, *output_currency_id)?,
			asset_length,
		)
		.and_then(|(dx, dy)| {
			Ok((
				RebaseTokenAmountConvertor::convert_balance_back(dx, *input_currency_id)?,
				RebaseTokenAmountConvertor::convert_balance_back(dy, *output_currency_id)?,
			))
		})
	}

	fn redeem_proportion(
		who: &Self::AccountId,
		pool_id: StableAssetPoolId,
		amount: Self::Balance,
		min_redeem_amounts: Vec<Self::Balance>,
	) -> DispatchResult {
		let pool_info = StableAsset::pool(pool_id)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidPoolId))?;
		let rebased_min_redeem_amounts = min_redeem_amounts
			.iter()
			.enumerate()
			.map(|(index, redeem_amount)| {
				if let Some(currency_id) = pool_info.assets.get(index) {
					RebaseTokenAmountConvertor::convert_balance(*redeem_amount, *currency_id)
				} else {
					Ok(*redeem_amount)
				}
			})
			.collect::<Result<Vec<_>, _>>()?;

		StableAsset::redeem_proportion(who, pool_id, amount, rebased_min_redeem_amounts)
	}

	fn redeem_single(
		who: &Self::AccountId,
		pool_id: StableAssetPoolId,
		amount: Self::Balance,
		i: PoolTokenIndex,
		min_redeem_amount: Self::Balance,
		asset_length: u32,
	) -> sp_std::result::Result<(Self::Balance, Self::Balance), DispatchError> {
		let pool_info = StableAsset::pool(pool_id)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidPoolId))?;
		let currency_id = pool_info
			.assets
			.get(i as usize)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidTokenIndex))?;
		let rebased_min_redeem_amount = RebaseTokenAmountConvertor::convert_balance(min_redeem_amount, *currency_id)?;

		StableAsset::redeem_single(who, pool_id, amount, i, rebased_min_redeem_amount, asset_length)
	}

	fn redeem_multi(
		who: &Self::AccountId,
		pool_id: StableAssetPoolId,
		amounts: Vec<Self::Balance>,
		max_redeem_amount: Self::Balance,
	) -> DispatchResult {
		let pool_info = StableAsset::pool(pool_id)
			.ok_or_else(|| ErrorConvertor::convert(RebasedStableAssetError::InvalidPoolId))?;
		let rebased_amounts: Vec<Self::Balance> = amounts
			.iter()
			.enumerate()
			.map(|(index, amount)| {
				if let Some(currency_id) = pool_info.assets.get(index) {
					RebaseTokenAmountConvertor::convert_balance(*amount, *currency_id)
				} else {
					Ok(*amount)
				}
			})
			.collect::<Result<Vec<_>, _>>()?;

		StableAsset::redeem_multi(who, pool_id, rebased_amounts, max_redeem_amount)
	}

	fn collect_fee(
		pool_id: StableAssetPoolId,
		pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		StableAsset::collect_fee(pool_id, pool_info)
	}

	fn update_balance(
		pool_id: StableAssetPoolId,
		pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		StableAsset::update_balance(pool_id, pool_info)
	}

	fn collect_yield(
		pool_id: StableAssetPoolId,
		pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		StableAsset::collect_yield(pool_id, pool_info)
	}

	fn modify_a(
		pool_id: StableAssetPoolId,
		a: Self::AtLeast64BitUnsigned,
		future_a_block: Self::BlockNumber,
	) -> DispatchResult {
		StableAsset::modify_a(pool_id, a, future_a_block)
	}

	fn get_collect_yield_amount(
		pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<
		StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	> {
		StableAsset::get_collect_yield_amount(pool_info)
	}

	fn get_balance_update_amount(
		pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<
		StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	> {
		StableAsset::get_balance_update_amount(pool_info)
	}

	fn get_redeem_proportion_amount(
		pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
		amount_bal: Self::Balance,
	) -> Option<RedeemProportionResult<Self::Balance>> {
		StableAsset::get_redeem_proportion_amount(pool_info, amount_bal).and_then(|mut r| {
			r.amounts = r
				.amounts
				.iter()
				.enumerate()
				.map(|(index, amount)| {
					if let Some(currency_id) = pool_info.assets.get(index) {
						RebaseTokenAmountConvertor::convert_balance_back(*amount, *currency_id)
					} else {
						Ok(*amount)
					}
				})
				.collect::<Result<Vec<_>, _>>()
				.ok()?;

			Some(r)
		})
	}

	fn get_best_route(
		input_asset: Self::AssetId,
		output_asset: Self::AssetId,
		input_amount: Self::Balance,
	) -> Option<(StableAssetPoolId, PoolTokenIndex, PoolTokenIndex, Self::Balance)> {
		StableAsset::get_best_route(
			input_asset,
			output_asset,
			RebaseTokenAmountConvertor::convert_balance(input_amount, input_asset).ok()?,
		)
		.and_then(|mut tuple| {
			tuple.3 = RebaseTokenAmountConvertor::convert_balance_back(tuple.3, output_asset).ok()?;
			Some(tuple)
		})
	}

	fn get_swap_output_amount(
		pool_id: StableAssetPoolId,
		input_index: PoolTokenIndex,
		output_index: PoolTokenIndex,
		dx_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		let pool_info = StableAsset::pool(pool_id)?;
		let input_currency_id = pool_info.assets.get(input_index as usize)?;
		let output_currency_id = pool_info.assets.get(output_index as usize)?;

		StableAsset::get_swap_output_amount(
			pool_id,
			input_index,
			output_index,
			RebaseTokenAmountConvertor::convert_balance(dx_bal, *input_currency_id).ok()?,
		)
		.and_then(|mut swap_result| {
			swap_result.dx =
				RebaseTokenAmountConvertor::convert_balance_back(swap_result.dx, *input_currency_id).ok()?;
			swap_result.dy =
				RebaseTokenAmountConvertor::convert_balance_back(swap_result.dy, *output_currency_id).ok()?;
			Some(swap_result)
		})
	}

	fn get_swap_input_amount(
		pool_id: StableAssetPoolId,
		input_index: PoolTokenIndex,
		output_index: PoolTokenIndex,
		dy_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		let pool_info = StableAsset::pool(pool_id)?;
		let input_currency_id = pool_info.assets.get(input_index as usize)?;
		let output_currency_id = pool_info.assets.get(output_index as usize)?;

		StableAsset::get_swap_input_amount(
			pool_id,
			input_index,
			output_index,
			RebaseTokenAmountConvertor::convert_balance(dy_bal, *output_currency_id).ok()?,
		)
		.and_then(|mut swap_result| {
			swap_result.dx =
				RebaseTokenAmountConvertor::convert_balance_back(swap_result.dx, *input_currency_id).ok()?;
			swap_result.dy =
				RebaseTokenAmountConvertor::convert_balance_back(swap_result.dy, *output_currency_id).ok()?;
			Some(swap_result)
		})
	}
}
