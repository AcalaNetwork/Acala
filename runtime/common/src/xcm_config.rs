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

use crate::{xcm_impl::AccountKey20Aliases, AccountId, Balance, Convert, CurrencyId};
use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, Get},
};
use orml_traits::MultiCurrency;
use pallet_xcm::XcmPassthrough;
use parachains_common::xcm_config::{ConcreteAssetFromSystem, ParentRelayOrSiblingParachains};
use polkadot_parachain_primitives::primitives::Sibling;
use xcm::latest::prelude::*;
use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	DescribeAllTerminal, DescribeFamily, HashedDescription, ParentIsPreset, RelayChainAsNative,
	SiblingParachainAsNative, SiblingParachainConvertsVia, SignedAccountId32AsNative, SovereignSignedViaLocation,
	TakeRevenue, TakeWeightCredit, TrailingSetTopicAsId, WithComputedOrigin,
};

/// Type for specifying how a `Location` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch RuntimeOrigin.
pub type LocationToAccountId<RelayNetwork, EvmAddressMapping> = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
	// Convert `AccountKey20` to `AccountId`
	AccountKey20Aliases<RelayNetwork, AccountId, EvmAddressMapping>,
	// Generate remote accounts according to polkadot standards
	HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
);

/// This is the type we use to convert an (incoming) XCM origin into a local `RuntimeOrigin`
/// instance, ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind`
/// which can biases the kind of local `RuntimeOrigin` it will become.
pub type XcmOriginToCallOrigin<LocationToAccountId, RuntimeOrigin, RelayChainOrigin, RelayNetwork> = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `RuntimeOrigin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

pub type Barrier<PolkadotXcm, UniversalLocation> = TrailingSetTopicAsId<(
	TakeWeightCredit,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Allow XCMs with some computed origins to pass through.
	WithComputedOrigin<
		(
			// If the message is one that immediately attempts to pay for execution, then
			// allow it.
			AllowTopLevelPaidExecutionFrom<Everything>,
			// Subscriptions for version tracking are OK.
			AllowSubscriptionsFrom<ParentRelayOrSiblingParachains>,
		),
		UniversalLocation,
		ConstU32<8>,
	>,
)>;

pub struct ToTreasury<CurrencyIdConvert, AcalaTreasuryAccount, Currencies>(
	sp_std::marker::PhantomData<(CurrencyIdConvert, AcalaTreasuryAccount, Currencies)>,
);
impl<CurrencyIdConvert, AcalaTreasuryAccount, Currencies> TakeRevenue
	for ToTreasury<CurrencyIdConvert, AcalaTreasuryAccount, Currencies>
where
	AcalaTreasuryAccount: Get<AccountId>,
	CurrencyIdConvert: Convert<Location, Option<CurrencyId>>,
	Currencies: MultiCurrency<AccountId, CurrencyId = CurrencyId, Balance = Balance>,
{
	fn take_revenue(revenue: Asset) {
		if let Asset {
			id: AssetId(location),
			fun: Fungible(amount),
		} = revenue
		{
			if let Some(currency_id) = CurrencyIdConvert::convert(location) {
				// Ensure AcalaTreasuryAccount have ed requirement for native asset, but don't need
				// ed requirement for cross-chain asset because it's one of whitelist accounts.
				// Ignore the result.
				let _ = Currencies::deposit(currency_id, &AcalaTreasuryAccount::get(), amount);
			}
		}
	}
}

pub struct AccountIdToLocation;
impl Convert<AccountId, Location> for AccountIdToLocation {
	fn convert(account: AccountId) -> Location {
		AccountId32 {
			network: None,
			id: account.into(),
		}
		.into()
	}
}

parameter_types! {
	pub const RelayLocation: Location = Location::parent();
}

// define assets that can be trusted to teleport by remotes
pub type TrustedTeleporters = (
	// relay token from relaychain or system parachain
	ConcreteAssetFromSystem<RelayLocation>,
);
