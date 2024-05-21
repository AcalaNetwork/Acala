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

use crate::{ExchangeRate, Rate};
use sp_runtime::DispatchResult;
use sp_std::{fmt::Debug, vec::Vec};
use xcm::v4::prelude::*;

pub trait HomaSubAccountXcm<AccountId, Balance> {
	type RelayChainAccountId: Debug + Clone + Ord;
	/// Cross-chain transfer staking currency to sub account on relaychain.
	fn transfer_staking_to_sub_account(sender: &AccountId, sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to withdraw_unbonded staking currency and
	/// send it back.
	fn withdraw_unbonded_from_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to bond extra.
	fn bond_extra_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to unbond.
	fn unbond_on_sub_account(sub_account_index: u16, amount: Balance) -> DispatchResult;
	/// Send XCM message to the relaychain for sub account to nominate.
	fn nominate_on_sub_account(sub_account_index: u16, targets: Vec<Self::RelayChainAccountId>) -> DispatchResult;
	/// The fee of cross-chain transfer is deducted from the recipient.
	fn get_xcm_transfer_fee() -> Balance;
	/// The fee of parachain
	fn get_parachain_fee(location: Location) -> Balance;
}

pub trait HomaManager<AccountId, Balance> {
	/// Mint liquid currency by locking up staking currency
	fn mint(who: AccountId, amount: Balance) -> DispatchResult;
	/// Request for protocol to redeem liquid currency for staking currency
	fn request_redeem(who: AccountId, amount: Balance, fast_match: bool) -> DispatchResult;
	/// Calculates current exchange rate between staking and liquid currencies (staking : liquid)
	fn get_exchange_rate() -> ExchangeRate;
	/// Estimated return rate per era from liquid staking
	fn get_estimated_reward_rate() -> Rate;
	/// Gets commission rate of homa protocol
	fn get_commission_rate() -> Rate;
	/// Fee for fast matching redeem request
	fn get_fast_match_fee() -> Rate;
}
