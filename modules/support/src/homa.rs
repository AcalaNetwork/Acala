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
use frame_support::{traits::Get, Parameter};
use sp_runtime::{
	traits::{MaybeDisplay, MaybeSerializeDeserialize, Member},
	RuntimeDebug,
};

#[impl_trait_for_tuples::impl_for_tuples(30)]
pub trait OnNewEra<EraIndex> {
	fn on_new_era(era: EraIndex);
}

pub trait NomineesProvider<AccountId> {
	fn nominees() -> Vec<AccountId>;
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct PolkadotUnlockChunk<Balance, EraIndex> {
	pub value: Balance,
	pub era: EraIndex,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct PolkadotStakingLedger<Balance, EraIndex> {
	/// Total amount, `active` plus all `unlocking`
	pub total: Balance,
	/// Amount at bonded
	pub active: Balance,
	pub unlocking: Vec<PolkadotUnlockChunk<Balance, EraIndex>>,
}

pub trait PolkadotBridgeType<BlockNumber, EraIndex> {
	type BondingDuration: Get<EraIndex>;
	type EraLength: Get<BlockNumber>;
	type PolkadotAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;
}

pub trait PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex>:
	PolkadotBridgeType<BlockNumber, EraIndex>
{
	fn bond_extra(account_index: u32, amount: Balance) -> DispatchResult;
	fn unbond(account_index: u32, amount: Balance) -> DispatchResult;
	fn rebond(account_index: u32, amount: Balance) -> DispatchResult;
	fn withdraw_unbonded(account_index: u32);
	fn nominate(account_index: u32, targets: Vec<Self::PolkadotAccountId>);
	fn transfer_to_bridge(account_index: u32, from: &AccountId, amount: Balance) -> DispatchResult;
	fn receive_from_bridge(account_index: u32, to: &AccountId, amount: Balance) -> DispatchResult;
	fn payout_stakers(account_index: u32, era: EraIndex);
}

pub trait PolkadotBridgeState<Balance, EraIndex> {
	fn staking_ledger(account_index: u32) -> PolkadotStakingLedger<Balance, EraIndex>;
	fn free_balance(account_index: u32) -> Balance;
	fn current_era() -> EraIndex;
}

pub trait PolkadotBridge<AccountId, BlockNumber, Balance, EraIndex>:
	PolkadotBridgeCall<AccountId, BlockNumber, Balance, EraIndex> + PolkadotBridgeState<Balance, EraIndex>
{
}

pub trait OnCommission<Balance, CurrencyId> {
	fn on_commission(currency_id: CurrencyId, amount: Balance);
}

impl<Balance, CurrencyId> OnCommission<Balance, CurrencyId> for () {
	fn on_commission(_currency_id: CurrencyId, _amount: Balance) {}
}

pub trait HomaProtocol<AccountId, Balance, EraIndex> {
	type Balance: Decode + Encode + Debug + Eq + PartialEq + Clone + HasCompact;

	fn mint(who: &AccountId, amount: Balance) -> sp_std::result::Result<Balance, DispatchError>;
	fn redeem_by_unbond(who: &AccountId, amount: Balance) -> DispatchResult;
	fn redeem_by_free_unbonded(who: &AccountId, amount: Balance) -> DispatchResult;
	fn redeem_by_claim_unbonding(who: &AccountId, amount: Balance, target_era: EraIndex) -> DispatchResult;
	fn withdraw_redemption(who: &AccountId) -> sp_std::result::Result<Balance, DispatchError>;
}
