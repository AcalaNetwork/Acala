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

use sp_runtime::DispatchError;

pub trait EarningManager<AccountId, Balance, BondingLedger> {
	type Moment;
	type FeeRatio;
	fn bond(who: AccountId, amount: Balance) -> Result<Balance, DispatchError>;
	fn unbond(who: AccountId, amount: Balance) -> Result<Balance, DispatchError>;
	fn unbond_instant(who: AccountId, amount: Balance) -> Result<Balance, DispatchError>;
	fn rebond(who: AccountId, amount: Balance) -> Result<Balance, DispatchError>;
	fn withdraw_unbonded(who: AccountId) -> Result<Balance, DispatchError>;
	fn get_bonding_ledger(who: AccountId) -> BondingLedger;
	fn get_min_bond() -> Balance;
	fn get_unbonding_period() -> Self::Moment;
	fn get_max_unbonding_chunks() -> u32;
	fn get_instant_unstake_fee() -> Option<Self::FeeRatio>;
}
