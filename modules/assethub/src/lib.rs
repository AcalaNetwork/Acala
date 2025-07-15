// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

//! # Module AssetHub
//!
//! This module is in charge of handling assethub related utilities and business logic.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::large_enum_variant)]

use parity_scale_codec::{Decode, Encode, FullCodec};
use sp_runtime::{traits::StaticLookup, RuntimeDebug};

use frame_support::traits::Get;
use module_support::assethub::*;
use primitives::{AccountId, Balance};
use sp_std::{boxed::Box, marker::PhantomData, prelude::*};

pub use cumulus_primitives_core::ParaId;
use xcm::v4::{prelude::*, Weight as XcmWeight};

/// The encoded index corresponds to AssetHub's Runtime module configuration.
/// https://github.com/polkadot-fellows/runtimes/blob/2fb47566718a974c261b68fffaae500be5581820/system-parachains/asset-hubs/asset-hub-kusama/src/lib.rs#L1065
#[derive(Encode, Decode, RuntimeDebug)]
pub enum KusamaAssetHubCall {
	#[codec(index = 10)]
	Balances(BalancesCall),
	#[codec(index = 89)] // TODO: update
	Staking(StakingCall),
	#[codec(index = 40)]
	Utility(Box<UtilityCall<Self>>),
	#[codec(index = 42)]
	Proxy(Box<ProxyCall<Self>>),
	#[codec(index = 31)]
	XcmPallet(XcmCall),
}

impl AssetHubCall for KusamaAssetHubCall {
	fn balances(call: BalancesCall) -> Self {
		KusamaAssetHubCall::Balances(call)
	}

	fn staking(call: StakingCall) -> Self {
		KusamaAssetHubCall::Staking(call)
	}

	fn utility(call: UtilityCall<Self>) -> Self {
		KusamaAssetHubCall::Utility(Box::new(call))
	}

	fn proxy(call: ProxyCall<Self>) -> Self {
		KusamaAssetHubCall::Proxy(Box::new(call))
	}

	fn xcm_pallet(call: XcmCall) -> Self {
		KusamaAssetHubCall::XcmPallet(call)
	}
}

/// The encoded index corresponds to AssetHub's Runtime module configuration.
/// https://github.com/polkadot-fellows/runtimes/blob/2fb47566718a974c261b68fffaae500be5581820/system-parachains/asset-hubs/asset-hub-polkadot/src/lib.rs#L983
#[derive(Encode, Decode, RuntimeDebug)]
pub enum PolkadotAssetHubCall {
	#[codec(index = 10)]
	Balances(BalancesCall),
	#[codec(index = 89)] // TODO: check https://github.com/polkadot-fellows/runtimes/pull/812
	Staking(StakingCall),
	#[codec(index = 40)]
	Utility(Box<UtilityCall<Self>>),
	#[codec(index = 42)]
	Proxy(Box<ProxyCall<Self>>),
	#[codec(index = 31)]
	XcmPallet(XcmCall),
}

impl AssetHubCall for PolkadotAssetHubCall {
	fn balances(call: BalancesCall) -> Self {
		PolkadotAssetHubCall::Balances(call)
	}

	fn staking(call: StakingCall) -> Self {
		PolkadotAssetHubCall::Staking(call)
	}

	fn utility(call: UtilityCall<Self>) -> Self {
		PolkadotAssetHubCall::Utility(Box::new(call))
	}

	fn proxy(call: ProxyCall<Self>) -> Self {
		PolkadotAssetHubCall::Proxy(Box::new(call))
	}

	fn xcm_pallet(call: XcmCall) -> Self {
		PolkadotAssetHubCall::XcmPallet(call)
	}
}

pub struct AssetHubCallBuilder<ParachainId, AHC>(PhantomData<(ParachainId, AHC)>);

impl<ParachainId, AHC> CallBuilder for AssetHubCallBuilder<ParachainId, AHC>
where
	ParachainId: Get<ParaId>,
	AHC: AssetHubCall + FullCodec,
{
	type AssetHubAccountId = AccountId;
	type Balance = Balance;
	type AssetHubCall = AHC;

	fn utility_as_derivative_call(call: AHC, index: u16) -> AHC {
		AHC::utility(UtilityCall::AsDerivative(index, call))
	}

	fn staking_bond_extra(amount: Self::Balance) -> AHC {
		AHC::staking(StakingCall::BondExtra(amount))
	}

	fn staking_unbond(amount: Self::Balance) -> AHC {
		AHC::staking(StakingCall::Unbond(amount))
	}

	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> AHC {
		AHC::staking(StakingCall::WithdrawUnbonded(num_slashing_spans))
	}

	fn staking_nominate(targets: Vec<Self::AssetHubAccountId>) -> AHC {
		AHC::staking(StakingCall::Nominate(
			targets.iter().map(|a| AssetHubLookup::unlookup(a.clone())).collect(),
		))
	}

	fn balances_transfer_keep_alive(to: Self::AssetHubAccountId, amount: Self::Balance) -> AHC {
		AHC::balances(BalancesCall::TransferKeepAlive(AssetHubLookup::unlookup(to), amount))
	}

	fn xcm_pallet_reserve_transfer_assets(
		dest: Location,
		beneficiary: Location,
		assets: Assets,
		fee_assets_item: u32,
	) -> AHC {
		AHC::xcm_pallet(XcmCall::LimitedReserveTransferAssets(
			dest.into_versioned(),
			beneficiary.into_versioned(),
			assets.into(),
			fee_assets_item,
			WeightLimit::Unlimited,
		))
	}

	fn proxy_call(real: Self::AssetHubAccountId, call: AHC) -> AHC {
		AHC::proxy(ProxyCall::Proxy(AssetHubLookup::unlookup(real), None, call))
	}

	fn finalize_call_into_xcm_message(call: AHC, extra_fee: Self::Balance, weight: XcmWeight) -> Xcm<()> {
		let asset = Asset {
			id: AssetId(Location::parent()),
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
				assets: AllCounted(1).into(), // there is only 1 asset on assethub
				beneficiary: Location {
					parents: 0,
					interior: Parachain(ParachainId::get().into()).into(),
				},
			},
		])
	}

	fn finalize_multiple_calls_into_xcm_message(calls: Vec<(AHC, XcmWeight)>, extra_fee: Self::Balance) -> Xcm<()> {
		let asset = Asset {
			id: AssetId(Location::parent()),
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
					assets: AllCounted(1).into(), // there is only 1 asset on assethub
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
