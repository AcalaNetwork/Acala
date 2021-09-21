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

use codec::{Decode, Encode, FullCodec};
use sp_runtime::traits::StaticLookup;

// mod mock;
// mod tests;
use frame_support::RuntimeDebug;
use module_support::CallBuilder;
use primitives::Balance;
use sp_std::{boxed::Box, marker::PhantomData, vec::Vec};

use frame_system::Config;

#[derive(Encode, Decode, RuntimeDebug)]
pub enum BalancesCall<T: Config> {
	#[codec(index = 3)]
	TransferKeepAlive(BalancesTransferKeepAliveCall<T>),
}

/// Relaychain balances.transfer_keep_alive call arguments
#[derive(Clone, Encode, Decode, RuntimeDebug)]
pub struct BalancesTransferKeepAliveCall<T: Config> {
	/// dest account
	pub dest: <T::Lookup as StaticLookup>::Source,
	/// transfer amount
	#[codec(compact)]
	pub value: Balance,
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum UtilityCall<RelaychainCall> {
	#[codec(index = 1)]
	AsDerivative(UtilityAsDerivativeCall<RelaychainCall>),
	#[codec(index = 2)]
	BatchAll(UtilityBatchAllCall<RelaychainCall>),
}

/// Relaychain utility.as_derivative call arguments
#[derive(Encode, Decode, RuntimeDebug)]
pub struct UtilityAsDerivativeCall<RelaychainCall> {
	/// derivative index
	pub index: u16,
	/// call
	pub call: RelaychainCall,
}

/// Relaychain utility.batch_all call arguments
#[derive(Encode, Decode, RuntimeDebug)]
pub struct UtilityBatchAllCall<RelaychainCall> {
	/// calls
	pub calls: Vec<RelaychainCall>,
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum StakingCall {
	#[codec(index = 3)]
	WithdrawUnbonded(StakingWithdrawUnbondedCall),
}

/// Argument for withdraw_unbond call
#[derive(Clone, Encode, Decode, RuntimeDebug)]
pub struct StakingWithdrawUnbondedCall {
	/// Withdraw amount
	pub num_slashing_spans: u32,
}

mod kusama {
	use crate::*;

	/// The encoded index correspondes to Kusama's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelaychainCall<T: Config> {
		#[codec(index = 4)]
		Balances(BalancesCall<T>),
		#[codec(index = 6)]
		Staking(StakingCall),
		#[codec(index = 24)]
		Utility(Box<UtilityCall<Self>>),
	}
}

mod polkadot {
	use crate::*;

	/// The encoded index correspondes to Polkadot's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/84a3962e76151ac5ed3afa4ef1e0af829531ab42/runtime/polkadot/src/lib.rs#L1040
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelaychainCall<T: Config> {
		#[codec(index = 5)]
		Balances(BalancesCall<T>),
		#[codec(index = 7)]
		Staking(StakingCall),
		#[codec(index = 26)]
		Utility(Box<UtilityCall<Self>>),
	}
}

#[cfg(feature = "kusama")]
pub use kusama::*;

#[cfg(feature = "polkadot")]
pub use polkadot::*;

pub struct RelaychainCallBuilder<T: Config>(PhantomData<T>);

impl<T: Config> CallBuilder for RelaychainCallBuilder<T>
where
	T::AccountId: FullCodec,
	RelaychainCall<T>: FullCodec,
{
	type AccountId = T::AccountId;
	type Balance = Balance;
	type RelaychainCall = RelaychainCall<T>;

	fn utility_batch_call(calls: Vec<Self::RelaychainCall>) -> Self::RelaychainCall {
		RelaychainCall::Utility(Box::new(UtilityCall::BatchAll(UtilityBatchAllCall { calls })))
	}

	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelaychainCall {
		RelaychainCall::Staking(StakingCall::WithdrawUnbonded(StakingWithdrawUnbondedCall {
			num_slashing_spans,
		}))
	}

	fn balances_transfer_keep_alive(to: Self::AccountId, amount: Self::Balance) -> Self::RelaychainCall {
		RelaychainCall::Balances(BalancesCall::TransferKeepAlive(BalancesTransferKeepAliveCall {
			dest: T::Lookup::unlookup(to),
			value: amount,
		}))
	}
}
