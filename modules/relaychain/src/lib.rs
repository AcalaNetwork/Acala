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

//! # Module RelayChain
//!
//! This module is in charge of handling relaychain related utilities and business logic.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use codec::{Decode, Encode, FullCodec};
use sp_runtime::traits::StaticLookup;

use frame_support::{traits::Get, weights::Weight, RuntimeDebug};
use module_support::CallBuilder;
use primitives::Balance;
use sp_std::{boxed::Box, marker::PhantomData, prelude::*};

pub use cumulus_primitives_core::ParaId;
use xcm::latest::prelude::*;

use frame_system::Config;

#[derive(Encode, Decode, RuntimeDebug)]
pub enum BalancesCall<T: Config> {
	#[codec(index = 3)]
	TransferKeepAlive(<T::Lookup as StaticLookup>::Source, #[codec(compact)] Balance), /* TODO: because param type
	                                                                                    * in relaychain is u64,
	                                                                                    * need to confirm
	                                                                                    * Balance(u128) is working. */
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum UtilityCall<RelayChainCall> {
	#[codec(index = 1)]
	AsDerivative(u16, RelayChainCall),
	#[codec(index = 2)]
	BatchAll(Vec<RelayChainCall>),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum StakingCall {
	#[codec(index = 1)]
	BondExtra(#[codec(compact)] Balance), /* TODO: because param type in relaychain is u64, need to confirm
	                                       * Balance(u128) is working. */
	#[codec(index = 2)]
	Unbond(#[codec(compact)] Balance), /* TODO: because param type in relaychain is u64, need to confirm
	                                    * Balance(u128) is working. */
	#[codec(index = 3)]
	WithdrawUnbonded(u32),
}

#[cfg(feature = "kusama")]
mod kusama {
	use crate::*;

	/// The encoded index correspondes to Kusama's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelayChainCall<T: Config> {
		#[codec(index = 4)]
		Balances(BalancesCall<T>),
		#[codec(index = 6)]
		Staking(StakingCall),
		#[codec(index = 24)]
		Utility(Box<UtilityCall<Self>>),
	}
}

#[cfg(feature = "polkadot")]
mod polkadot {
	use crate::*;

	/// The encoded index correspondes to Polkadot's Runtime module configuration.
	/// https://github.com/paritytech/polkadot/blob/84a3962e76151ac5ed3afa4ef1e0af829531ab42/runtime/polkadot/src/lib.rs#L1040
	#[derive(Encode, Decode, RuntimeDebug)]
	pub enum RelayChainCall<T: Config> {
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

pub struct RelayChainCallBuilder<T: Config, ParachainId: Get<ParaId>>(PhantomData<(T, ParachainId)>);

impl<T: Config, ParachainId: Get<ParaId>> CallBuilder for RelayChainCallBuilder<T, ParachainId>
where
	T::AccountId: FullCodec,
	RelayChainCall<T>: FullCodec,
{
	type AccountId = T::AccountId;
	type Balance = Balance;
	type RelayChainCall = RelayChainCall<T>;

	fn utility_batch_call(calls: Vec<Self::RelayChainCall>) -> Self::RelayChainCall {
		RelayChainCall::Utility(Box::new(UtilityCall::BatchAll(calls)))
	}

	fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall {
		RelayChainCall::Utility(Box::new(UtilityCall::AsDerivative(index, call)))
	}

	fn staking_bond_extra(amount: Self::Balance) -> Self::RelayChainCall {
		RelayChainCall::Staking(StakingCall::BondExtra(amount))
	}

	fn staking_unbond(amount: Self::Balance) -> Self::RelayChainCall {
		RelayChainCall::Staking(StakingCall::Unbond(amount))
	}

	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall {
		RelayChainCall::Staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
	}

	fn balances_transfer_keep_alive(to: Self::AccountId, amount: Self::Balance) -> Self::RelayChainCall {
		RelayChainCall::Balances(BalancesCall::TransferKeepAlive(T::Lookup::unlookup(to), amount))
	}

	fn finalize_call_into_xcm_message(call: Self::RelayChainCall, extra_fee: Self::Balance, weight: Weight) -> Xcm<()> {
		let asset = MultiAsset {
			id: Concrete(MultiLocation::here()),
			fun: Fungibility::Fungible(extra_fee),
		};
		Xcm(vec![
			WithdrawAsset(asset.clone().into()),
			BuyExecution {
				fees: asset,
				weight_limit: Unlimited,
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: weight,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: u32::max_value(),
				beneficiary: MultiLocation {
					parents: 0,
					interior: X1(Parachain(ParachainId::get().into())),
				},
			},
		])
	}
}
