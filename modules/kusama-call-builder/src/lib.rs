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

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use sp_runtime::MultiAddress;

// mod mock;
// mod tests;

pub use module::*;
use module_support::CallBuilder;
use primitives::{AccountIndex, Balance};

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	/// The encoded index correspondes to Kusama's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum KusamaCall<T: Config> {
		#[codec(index = 4)]
		Balances(BalancesCall<T>),
		#[codec(index = 6)]
		Staking(StakingCall),
		#[codec(index = 24)]
		Utility(UtilityCall<Self>),
	}

	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum BalancesCall<T: Config> {
		#[codec(index = 3)]
		TransferKeepAlive(MultiAddress<T::AccountId, AccountIndex>, #[codec(compact)] u128),
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

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	impl<T: Config> CallBuilder<T::AccountId, Balance> for Pallet<T>
	where
		KusamaCall<T>: Encode + Decode,
	{
		type RelaychainCall = KusamaCall<T>;

		fn utility_batch_call(call: Vec<Self::RelaychainCall>) -> Self::RelaychainCall {
			KusamaCall::Utility(UtilityCall::BatchAll(call))
		}

		fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelaychainCall {
			KusamaCall::Staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
		}

		fn balances_transfer_keep_alive(to: T::AccountId, amount: Balance) -> Self::RelaychainCall {
			KusamaCall::Balances(BalancesCall::TransferKeepAlive(MultiAddress::Id(to), amount))
		}
	}
}
