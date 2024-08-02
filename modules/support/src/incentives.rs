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

use crate::Rate;
use parity_scale_codec::{Decode, Encode};
use primitives::CurrencyId;
use scale_info::TypeInfo;
use sp_runtime::{DispatchResult, RuntimeDebug};
use sp_std::prelude::*;

/// PoolId for various rewards pools
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum PoolId {
	/// Rewards and shares pool for users who open CDP(CollateralCurrencyId)
	Loans(CurrencyId),

	/// Rewards and shares pool for DEX makers who stake LP token(LPCurrencyId)
	Dex(CurrencyId),

	/// Rewards and shares pool for earning module
	Earning(CurrencyId),

	/// Rewards and shares pool for Homa nominees election
	NomineesElection,
}

pub trait IncentivesManager<AccountId, Balance, CurrencyId, PoolId> {
	/// Gets reward amount for the given reward currency added per period
	fn get_incentive_reward_amount(pool_id: PoolId, currency_id: CurrencyId) -> Balance;
	/// Stake LP token to add shares to pool
	fn deposit_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
	/// Unstake LP token to remove shares from pool
	fn withdraw_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
	/// Claim all available rewards for specific `PoolId`
	fn claim_rewards(who: AccountId, pool_id: PoolId) -> DispatchResult;
	/// Gets deduction reate for claiming reward early
	fn get_claim_reward_deduction_rate(pool_id: PoolId) -> Rate;
	/// Gets the pending rewards for a pool, for an account
	fn get_pending_rewards(pool_id: PoolId, who: AccountId, reward_currency: Vec<CurrencyId>) -> Vec<Balance>;
}

pub trait DEXIncentives<AccountId, CurrencyId, Balance> {
	fn do_deposit_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
	fn do_withdraw_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult;
}

#[cfg(feature = "std")]
impl<AccountId, CurrencyId, Balance> DEXIncentives<AccountId, CurrencyId, Balance> for () {
	fn do_deposit_dex_share(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn do_withdraw_dex_share(_: &AccountId, _: CurrencyId, _: Balance) -> DispatchResult {
		Ok(())
	}
}
