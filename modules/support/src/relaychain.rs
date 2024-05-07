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

// * Since XCM V3, relaychain configs 'SafeCallFilter' to filter the call in Transact:
// * https://github.com/paritytech/polkadot/blob/master/runtime/polkadot/src/xcm_config.rs

use parity_scale_codec::{Decode, Encode, FullCodec};
use primitives::{AccountId, Balance};
use sp_runtime::{
	traits::{AccountIdLookup, StaticLookup},
	RuntimeDebug,
};
use sp_std::prelude::*;
use xcm::{prelude::*, v3::Weight as XcmWeight};

#[derive(Encode, Decode, RuntimeDebug)]
pub enum BalancesCall {
	#[codec(index = 3)]
	TransferKeepAlive(<RelayChainLookup as StaticLookup>::Source, #[codec(compact)] Balance),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum UtilityCall<RCC> {
	#[codec(index = 1)]
	AsDerivative(u16, RCC),
}

#[derive(Encode, Decode, RuntimeDebug)]
pub enum StakingCall {
	#[codec(index = 1)]
	BondExtra(#[codec(compact)] Balance),
	#[codec(index = 2)]
	Unbond(#[codec(compact)] Balance),
	#[codec(index = 3)]
	WithdrawUnbonded(u32),
	#[codec(index = 5)]
	Nominate(Vec<<RelayChainLookup as StaticLookup>::Source>),
}

/// `pallet-xcm` calls.
#[derive(Encode, Decode, RuntimeDebug)]
pub enum XcmCall {
	/// `limited_reserve_transfer_assets(dest, beneficiary, assets, fee_asset_item, weight_limit)`
	/// call.
	#[codec(index = 8)]
	LimitedReserveTransferAssets(VersionedLocation, VersionedLocation, VersionedAssets, u32, WeightLimit),
}

// Same to `Polkadot` and `Kusama` runtime `Lookup` config.
pub type RelayChainLookup = AccountIdLookup<AccountId, ()>;

/// `pallet-proxy` calls.
#[derive(Encode, Decode, RuntimeDebug)]
pub enum ProxyCall<RCC> {
	/// `proxy(real, force_proxy_type, call)` call. Force proxy type is not supported and
	/// is always set to `None`.
	#[codec(index = 0)]
	Proxy(<RelayChainLookup as StaticLookup>::Source, Option<()>, RCC),
}

pub trait RelayChainCall: Sized {
	fn balances(call: BalancesCall) -> Self;
	fn staking(call: StakingCall) -> Self;
	fn utility(call: UtilityCall<Self>) -> Self;
	fn proxy(call: ProxyCall<Self>) -> Self;
	fn xcm_pallet(call: XcmCall) -> Self;
}

pub trait CallBuilder {
	type RelayChainAccountId: FullCodec;
	type Balance: FullCodec;
	type RelayChainCall: FullCodec + RelayChainCall;

	/// Execute a call, replacing the `Origin` with a sub-account.
	///  params:
	/// - call: The call to be executed.
	/// - index: The index of sub-account to be used as the new origin.
	fn utility_as_derivative_call(call: Self::RelayChainCall, index: u16) -> Self::RelayChainCall;

	/// Bond extra on relay-chain.
	///  params:
	/// - amount: The amount of staking currency to bond.
	fn staking_bond_extra(amount: Self::Balance) -> Self::RelayChainCall;

	/// Unbond on relay-chain.
	///  params:
	/// - amount: The amount of staking currency to unbond.
	fn staking_unbond(amount: Self::Balance) -> Self::RelayChainCall;

	/// Withdraw unbonded staking on the relay-chain.
	///  params:
	/// - num_slashing_spans: The number of slashing spans to withdraw from.
	fn staking_withdraw_unbonded(num_slashing_spans: u32) -> Self::RelayChainCall;

	/// Nominate the relay-chain.
	///  params:
	/// - targets: The target validator list.
	fn staking_nominate(targets: Vec<Self::RelayChainAccountId>) -> Self::RelayChainCall;

	/// Transfer Staking currency to another account, disallowing "death".
	///  params:
	/// - to: The destination for the transfer
	/// - amount: The amount of staking currency to be transferred.
	fn balances_transfer_keep_alive(to: Self::RelayChainAccountId, amount: Self::Balance) -> Self::RelayChainCall;

	/// Reserve transfer assets.
	/// params:
	/// - dest: The destination chain.
	/// - beneficiary: The beneficiary.
	/// - assets: The assets to be transferred.
	/// - fee_assets_item: The index of assets for fees.
	fn xcm_pallet_reserve_transfer_assets(
		dest: Location,
		beneficiary: Location,
		assets: Assets,
		fee_assets_item: u32,
	) -> Self::RelayChainCall;

	/// Proxy a call with a `real` account without a forced proxy type.
	/// params:
	/// - real: The real account.
	/// - call: The call to be executed.
	fn proxy_call(real: Self::RelayChainAccountId, call: Self::RelayChainCall) -> Self::RelayChainCall;

	/// Wrap the final call into the Xcm format.
	///  params:
	/// - call: The call to be executed
	/// - extra_fee: Extra fee (in staking currency) used for buy the `weight`.
	/// - weight: the weight limit used for XCM.
	fn finalize_call_into_xcm_message(
		call: Self::RelayChainCall,
		extra_fee: Self::Balance,
		weight: XcmWeight,
	) -> Xcm<()>;

	/// Wrap the final multiple calls into the Xcm format.
	///  params:
	/// - calls: the multiple calls and its weight limit to be executed
	/// - extra_fee: Extra fee (in staking currency) used for buy the `weight`.
	fn finalize_multiple_calls_into_xcm_message(
		calls: Vec<(Self::RelayChainCall, XcmWeight)>,
		extra_fee: Self::Balance,
	) -> Xcm<()>;
}
