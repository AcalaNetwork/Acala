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

use sp_runtime::DispatchResult;
use sp_std::vec::Vec;

pub trait XcmHelper<AccountId, Balance> {
	fn mint_xcm_fail(
		chain_id: u32,
		account_id: AccountId,
		pool_id: u32,
		mint_amount: Balance,
		asset_key: Vec<u8>,
	) -> DispatchResult;
	fn redeem_proportion_xcm(
		chain_id: u32,
		account_id: AccountId,
		pool_id: u32,
		amount: Balance,
		min_redeem_amounts: Vec<Balance>,
		asset_key: Vec<u8>,
	) -> DispatchResult;
	fn redeem_single_xcm(
		chain_id: u32,
		account_id: AccountId,
		pool_id: u32,
		amount: Balance,
		i: u32,
		min_redeem_amount: Balance,
		asset_length: u32,
		asset_key: Vec<u8>,
	) -> DispatchResult;
}
