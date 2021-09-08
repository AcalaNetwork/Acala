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

//! # Module Relaychain
//!
//! This module is in charge of handling relaychain related utilities and business logic.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::{Codec, Decode, Encode};
use sp_runtime::MultiAddress;

// mod mock;
// mod tests;
use frame_support::RuntimeDebug;
use module_support::CallBuilder;
use primitives::{AccountIndex, Balance};
use sp_std::{marker::PhantomData, vec::Vec};

#[derive(Encode, Decode, RuntimeDebug)]
pub enum BalancesCall<AccountId> {
	#[codec(index = 3)]
	TransferKeepAlive(MultiAddress<AccountId, AccountIndex>, #[codec(compact)] u128),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum UtilityCall<RelaychainCall> {
	#[codec(index = 2)]
	BatchAll(Vec<RelaychainCall>),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum StakingCall {
	#[codec(index = 3)]
	WithdrawUnbonded(#[codec(compact)] u32),
}

mod kusama {
	use crate::*;

	/// The encoded index correspondes to Kusama's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelaychainCall<AccountId> {
		#[codec(index = 4)]
		Balances(BalancesCall<AccountId>),
		#[codec(index = 6)]
		Staking(StakingCall),
		#[codec(index = 24)]
		Utility(UtilityCall<Self>),
	}
}

mod polkadot {
	use crate::*;

	/// The encoded index correspondes to Polkadot's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/84a3962e76151ac5ed3afa4ef1e0af829531ab42/runtime/polkadot/src/lib.rs#L1040
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelaychainCall<AccountId> {
		#[codec(index = 5)]
		Balances(BalancesCall<AccountId>),
		#[codec(index = 7)]
		Staking(StakingCall),
		#[codec(index = 26)]
		Utility(UtilityCall<Self>),
	}
}

#[cfg(feature = "kusama")]
pub use kusama::*;

#[cfg(feature = "polkadot")]
pub use polkadot::*;

pub struct RelaychainCallBuilder<AccountId>(PhantomData<AccountId>);

impl<AccountId> CallBuilder for RelaychainCallBuilder<AccountId>
where
	AccountId: Codec,
	RelaychainCall<AccountId>: Codec,
{
	type AccountId = AccountId;
	type Balance = Balance;
	type RelaychainCall = RelaychainCall<Self::AccountId>;

	fn utility_batch_call(call: Vec<Self::RelaychainCall>) -> Self::RelaychainCall {
		RelaychainCall::Utility(UtilityCall::BatchAll(call))
	}

	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelaychainCall {
		RelaychainCall::Staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
	}

	fn balances_transfer_keep_alive(to: AccountId, amount: Balance) -> Self::RelaychainCall {
		RelaychainCall::Balances(BalancesCall::TransferKeepAlive(MultiAddress::Id(to), amount))
	}
}
