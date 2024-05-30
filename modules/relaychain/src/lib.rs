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

//! # Module RelayChain
//!
//! This module is in charge of handling relaychain related utilities and business logic.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::large_enum_variant)]

use parity_scale_codec::{Decode, Encode, FullCodec};
use sp_runtime::{traits::StaticLookup, RuntimeDebug};

use frame_support::traits::Get;
use module_support::relaychain::*;
use primitives::{AccountId, Balance};
use sp_std::{boxed::Box, marker::PhantomData, prelude::*};

pub use cumulus_primitives_core::ParaId;
use xcm::v4::{prelude::*, Weight as XcmWeight};

/// The encoded index corresponds to Kusama's Runtime module configuration.
/// https://github.com/paritytech/polkadot/blob/444e96ae34bcec8362f0f947a07bd912b32ca48f/runtime/kusama/src/lib.rs#L1379
#[derive(Encode, Decode, RuntimeDebug)]
pub enum KusamaRelayChainCall {
	#[codec(index = 4)]
	Balances(BalancesCall),
	#[codec(index = 6)]
	Staking(StakingCall),
	#[codec(index = 24)]
	Utility(Box<UtilityCall<Self>>),
	#[codec(index = 30)]
	Proxy(Box<ProxyCall<Self>>),
	#[codec(index = 99)]
	XcmPallet(XcmCall),
}

impl RelayChainCall for KusamaRelayChainCall {
	fn balances(call: BalancesCall) -> Self {
		KusamaRelayChainCall::Balances(call)
	}

	fn staking(call: StakingCall) -> Self {
		KusamaRelayChainCall::Staking(call)
	}

	fn utility(call: UtilityCall<Self>) -> Self {
		KusamaRelayChainCall::Utility(Box::new(call))
	}

	fn proxy(call: ProxyCall<Self>) -> Self {
		KusamaRelayChainCall::Proxy(Box::new(call))
	}

	fn xcm_pallet(call: XcmCall) -> Self {
		KusamaRelayChainCall::XcmPallet(call)
	}
}

/// The encoded index corresponds to Polkadot's Runtime module configuration.
/// https://github.com/paritytech/polkadot/blob/84a3962e76151ac5ed3afa4ef1e0af829531ab42/runtime/polkadot/src/lib.rs#L1040
#[derive(Encode, Decode, RuntimeDebug)]
pub enum PolkadotRelayChainCall {
	#[codec(index = 5)]
	Balances(BalancesCall),
	#[codec(index = 7)]
	Staking(StakingCall),
	#[codec(index = 26)]
	Utility(Box<UtilityCall<Self>>),
	#[codec(index = 29)]
	Proxy(Box<ProxyCall<Self>>),
	#[codec(index = 99)]
	XcmPallet(XcmCall),
}

impl RelayChainCall for PolkadotRelayChainCall {
	fn balances(call: BalancesCall) -> Self {
		PolkadotRelayChainCall::Balances(call)
	}

	fn staking(call: StakingCall) -> Self {
		PolkadotRelayChainCall::Staking(call)
	}

	fn utility(call: UtilityCall<Self>) -> Self {
		PolkadotRelayChainCall::Utility(Box::new(call))
	}

	fn proxy(call: ProxyCall<Self>) -> Self {
		PolkadotRelayChainCall::Proxy(Box::new(call))
	}

	fn xcm_pallet(call: XcmCall) -> Self {
		PolkadotRelayChainCall::XcmPallet(call)
	}
}

pub struct RelayChainCallBuilder<ParachainId, RCC>(PhantomData<(ParachainId, RCC)>);

impl<ParachainId, RCC> CallBuilder for RelayChainCallBuilder<ParachainId, RCC>
where
	ParachainId: Get<ParaId>,
	RCC: RelayChainCall + FullCodec,
{
	type RelayChainAccountId = AccountId;
	type Balance = Balance;
	type RelayChainCall = RCC;

	fn utility_as_derivative_call(call: RCC, index: u16) -> RCC {
		RCC::utility(UtilityCall::AsDerivative(index, call))
	}

	fn staking_bond_extra(amount: Self::Balance) -> RCC {
		RCC::staking(StakingCall::BondExtra(amount))
	}

	fn staking_unbond(amount: Self::Balance) -> RCC {
		RCC::staking(StakingCall::Unbond(amount))
	}

	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> RCC {
		RCC::staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
	}

	fn staking_nominate(targets: Vec<Self::RelayChainAccountId>) -> RCC {
		RCC::staking(StakingCall::Nominate(
			targets.iter().map(|a| RelayChainLookup::unlookup(a.clone())).collect(),
		))
	}

	fn balances_transfer_keep_alive(to: Self::RelayChainAccountId, amount: Self::Balance) -> RCC {
		RCC::balances(BalancesCall::TransferKeepAlive(RelayChainLookup::unlookup(to), amount))
	}

	fn xcm_pallet_reserve_transfer_assets(
		dest: Location,
		beneficiary: Location,
		assets: Assets,
		fee_assets_item: u32,
	) -> RCC {
		RCC::xcm_pallet(XcmCall::LimitedReserveTransferAssets(
			dest.into_versioned(),
			beneficiary.into_versioned(),
			assets.into(),
			fee_assets_item,
			WeightLimit::Unlimited,
		))
	}

	fn proxy_call(real: Self::RelayChainAccountId, call: RCC) -> RCC {
		RCC::proxy(ProxyCall::Proxy(RelayChainLookup::unlookup(real), None, call))
	}

	fn finalize_call_into_xcm_message(call: RCC, extra_fee: Self::Balance, weight: XcmWeight) -> Xcm<()> {
		let asset = Asset {
			id: AssetId(Location::here()),
			fun: Fungibility::Fungible(extra_fee),
		};
		Xcm(vec![
			WithdrawAsset(asset.clone().into()),
			BuyExecution {
				fees: asset,
				weight_limit: Unlimited,
			},
			Transact {
				origin_kind: OriginKind::SovereignAccount,
				require_weight_at_most: weight,
				call: call.encode().into(),
			},
			RefundSurplus,
			DepositAsset {
				assets: AllCounted(1).into(), // there is only 1 asset on relaychain
				beneficiary: Location {
					parents: 0,
					interior: Parachain(ParachainId::get().into()).into(),
				},
			},
		])
	}

	fn finalize_multiple_calls_into_xcm_message(calls: Vec<(RCC, XcmWeight)>, extra_fee: Self::Balance) -> Xcm<()> {
		let asset = Asset {
			id: AssetId(Location::here()),
			fun: Fungibility::Fungible(extra_fee),
		};

		let transacts = calls
			.iter()
			.map(|(call, weight)| Transact {
				origin_kind: OriginKind::SovereignAccount,
				require_weight_at_most: *weight,
				call: call.encode().into(),
			})
			.collect();

		Xcm([
			vec![
				WithdrawAsset(asset.clone().into()),
				BuyExecution {
					fees: asset,
					weight_limit: Unlimited,
				},
			],
			transacts,
			vec![
				RefundSurplus,
				DepositAsset {
					assets: AllCounted(1).into(), // there is only 1 asset on relaychain
					beneficiary: Location {
						parents: 0,
						interior: Parachain(ParachainId::get().into()).into(),
					},
				},
			],
		]
		.concat())
	}
}
