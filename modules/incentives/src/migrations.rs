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

use super::*;

/// PoolId for various rewards pools
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PoolIdV0<AccountId> {
	/// Rewards pool(NativeCurrencyId) for users who open CDP
	LoansIncentive(CurrencyId),

	/// Rewards pool(NativeCurrencyId) for market makers who provide dex
	/// liquidity
	DexIncentive(CurrencyId),

	/// Rewards pool(NativeCurrencyId) for users who staking by Homa protocol
	HomaIncentive,

	/// Rewards pool(StableCurrencyId) for liquidators who provide dex liquidity
	/// to participate automatic liquidation
	DexSaving(CurrencyId),

	/// Rewards pool(LiquidCurrencyId) for users who guarantee for validators by
	/// Homa protocol
	HomaValidatorAllowance(AccountId),
}
