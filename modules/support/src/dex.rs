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

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{DispatchError, RuntimeDebug};
use sp_std::{cmp::PartialEq, prelude::*};

#[derive(RuntimeDebug, Encode, Decode, Clone, Copy, PartialEq, TypeInfo)]
pub enum SwapLimit<Balance> {
	/// use exact amount supply amount to swap. (exact_supply_amount, minimum_target_amount)
	ExactSupply(Balance, Balance),
	/// swap to get exact amount target. (maximum_supply_amount, exact_target_amount)
	ExactTarget(Balance, Balance),
}
pub trait DEXManager<AccountId, CurrencyId, Balance> {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance);

	fn get_liquidity_token_address(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> Option<H160>;

	fn get_swap_amount(path: &[CurrencyId], limit: SwapLimit<Balance>) -> Option<(Balance, Balance)>;

	fn get_best_price_swap_path(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
		alternative_path_joint_list: Vec<Vec<CurrencyId>>,
	) -> Option<Vec<CurrencyId>>;

	fn swap_with_specific_path(
		who: &AccountId,
		path: &[CurrencyId],
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;

	fn add_liquidity(
		who: &AccountId,
		currency_id_a: CurrencyId,
		currency_id_b: CurrencyId,
		max_amount_a: Balance,
		max_amount_b: Balance,
		min_share_increment: Balance,
		stake_increment_share: bool,
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError>;

	fn remove_liquidity(
		who: &AccountId,
		currency_id_a: CurrencyId,
		currency_id_b: CurrencyId,
		remove_share: Balance,
		min_withdrawn_a: Balance,
		min_withdrawn_b: Balance,
		by_unstake: bool,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;
}

pub trait StableAssetDEX<AccountId, Balance, CurrencyId, StableAssetPoolId, PoolTokenIndex> {
	fn get_best_price_pool(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		limit: SwapLimit<Balance>,
	) -> Option<(StableAssetPoolId, PoolTokenIndex, PoolTokenIndex)>;

	fn swap(
		who: &AccountId,
		pool_id: StableAssetPoolId,
		supply_asset_index: PoolTokenIndex,
		target_asset_index: PoolTokenIndex,
		limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError>;
}

#[cfg(feature = "std")]
impl<AccountId, CurrencyId, Balance> DEXManager<AccountId, CurrencyId, Balance> for ()
where
	Balance: Default,
{
	fn get_liquidity_pool(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> (Balance, Balance) {
		Default::default()
	}

	fn get_liquidity_token_address(_currency_id_a: CurrencyId, _currency_id_b: CurrencyId) -> Option<H160> {
		Some(Default::default())
	}

	fn get_swap_amount(_path: &[CurrencyId], _limit: SwapLimit<Balance>) -> Option<(Balance, Balance)> {
		Some(Default::default())
	}

	fn get_best_price_swap_path(
		_supply_currency_id: CurrencyId,
		_target_currency_id: CurrencyId,
		_limit: SwapLimit<Balance>,
		_alternative_path_joint_list: Vec<Vec<CurrencyId>>,
	) -> Option<Vec<CurrencyId>> {
		Some(Default::default())
	}

	fn swap_with_specific_path(
		_who: &AccountId,
		_path: &[CurrencyId],
		_limit: SwapLimit<Balance>,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		Ok(Default::default())
	}

	fn add_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_max_amount_a: Balance,
		_max_amount_b: Balance,
		_min_share_increment: Balance,
		_stake_increment_share: bool,
	) -> sp_std::result::Result<(Balance, Balance, Balance), DispatchError> {
		Ok(Default::default())
	}

	fn remove_liquidity(
		_who: &AccountId,
		_currency_id_a: CurrencyId,
		_currency_id_b: CurrencyId,
		_remove_share: Balance,
		_min_withdrawn_a: Balance,
		_min_withdrawn_b: Balance,
		_by_unstake: bool,
	) -> sp_std::result::Result<(Balance, Balance), DispatchError> {
		Ok(Default::default())
	}
}
