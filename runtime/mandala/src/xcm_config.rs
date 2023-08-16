// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

use super::{
	constants::fee::*, AccountId, AllPalletsWithSystem, AssetIdMapping, AssetIdMaps, Balance, Balances, Convert,
	Currencies, CurrencyId, EvmAddressMapping, ExistentialDeposits, GetNativeCurrencyId, NativeTokenExistentialDeposit,
	ParachainInfo, ParachainSystem, PolkadotXcm, Runtime, RuntimeCall, RuntimeEvent, RuntimeOrigin, TreasuryAccount,
	UnknownTokens, XcmpQueue, ACA,
};
use codec::{Decode, Encode};
pub use cumulus_primitives_core::ParaId;
pub use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, Get, Nothing},
	weights::Weight,
};
use module_asset_registry::{BuyWeightRateOfErc20, BuyWeightRateOfForeignAsset, BuyWeightRateOfStableAsset};
use module_transaction_payment::BuyWeightRateOfTransactionFeePool;
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key, MultiCurrency};
use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use primitives::evm::is_system_contract;
use runtime_common::{
	local_currency_location, native_currency_location, xcm_impl::AccountKey20Aliases, AcalaDropAssets,
	EnsureRootOrHalfGeneralCouncil, EnsureRootOrThreeFourthsGeneralCouncil, FixedRateOfAsset,
};
use xcm::{prelude::*, v3::Weight as XcmWeight};
pub use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, NativeAsset,
	ParentAsSuperuser, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative, SiblingParachainConvertsVia,
	SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation, TakeRevenue, TakeWeightCredit,
};

parameter_types! {
	pub const DotLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Polkadot;
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub UniversalLocation: InteriorMultiLocation = X2(GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into()));
	pub CheckingAccount: AccountId = PolkadotXcm::check_account();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch RuntimeOrigin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
	// Convert `AccountKey20` to `AccountId`
	AccountKey20Aliases<RelayNetwork, AccountId, EvmAddressMapping<Runtime>>,
);

/// This is the type we use to convert an (incoming) XCM origin into a local `RuntimeOrigin`
/// instance, ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind`
/// which can biases the kind of local `RuntimeOrigin` it will become.
pub type XcmOriginToCallOrigin = (
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
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<RuntimeOrigin>,
);

pub type Barrier = (
	TakeWeightCredit,
	AllowTopLevelPaidExecutionFrom<Everything>,
	// Expected responses are OK.
	AllowKnownQueryResponses<PolkadotXcm>,
	// Subscriptions for version tracking are OK.
	AllowSubscriptionsFrom<Everything>,
);

pub struct ToTreasury;
impl TakeRevenue for ToTreasury {
	fn take_revenue(revenue: MultiAsset) {
		if let MultiAsset {
			id: Concrete(location),
			fun: Fungible(amount),
		} = revenue
		{
			if let Some(currency_id) = CurrencyIdConvert::convert(location) {
				// Ensure TreasuryAccount have ed requirement for native asset, but don't need
				// ed requirement for cross-chain asset because it's one of whitelist accounts.
				// Ignore the result.
				let _ = Currencies::deposit(currency_id, &TreasuryAccount::get(), amount);
			}
		}
	}
}

parameter_types! {
	// One XCM operation is 1_000_000 weight - almost certainly a conservative estimate.
	pub UnitWeightCost: XcmWeight = XcmWeight::from_parts(1_000_000, 0);
	pub const MaxInstructions: u32 = 100;
	pub DotPerSecond: (AssetId, u128, u128) = (
		MultiLocation::parent().into(),
		dot_per_second(),
		0
	);
	pub AcaPerSecond: (AssetId, u128, u128) = (
		local_currency_location(ACA).unwrap().into(),
		aca_per_second(),
		0
	);
	pub BaseRate: u128 = aca_per_second();
}

pub type Trader = (
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>,
	FixedRateOfFungible<DotPerSecond, ToTreasury>,
	FixedRateOfFungible<AcaPerSecond, ToTreasury>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfForeignAsset<Runtime>>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfErc20<Runtime>>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfStableAsset<Runtime>>,
);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type RuntimeCall = RuntimeCall;
	type XcmSender = XcmRouter;
	// How to withdraw and deposit an asset.
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToCallOrigin;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
	// Teleporting is disabled.
	type IsTeleporter = ();
	type UniversalLocation = UniversalLocation;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	// Only receiving DOT is handled, and all fees must be paid in DOT.
	type Trader = Trader;
	type ResponseHandler = (); // Don't handle responses for now.
	type AssetTrap = AcalaDropAssets<
		PolkadotXcm,
		ToTreasury,
		CurrencyIdConvert,
		GetNativeCurrencyId,
		NativeTokenExistentialDeposit,
		ExistentialDeposits,
	>;
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = ();
	type SubscriptionService = PolkadotXcm;
	type PalletInstancesInfo = AllPalletsWithSystem;
	type MaxAssetsIntoHolding = ConstU32<64>;
	type FeeManager = ();
	type MessageExporter = ();
	type UniversalAliases = Nothing;
	type CallDispatcher = RuntimeCall;
	type SafeCallFilter = Everything;
	type Aliasers = Nothing;
}

/// No local origins on this chain are allowed to dispatch XCM sends/executions.
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, (), ()>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

pub type XcmExecutor = runtime_common::XcmExecutor<
	XcmConfig,
	AccountId,
	Balance,
	LocationToAccountId,
	module_evm_bridge::EVMBridge<Runtime>,
>;

#[cfg(feature = "runtime-benchmarks")]
parameter_types! {
	pub ReachableDest: Option<MultiLocation> = Some(Parent.into());
}

impl pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = XcmExecutor;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type UniversalLocation = UniversalLocation;
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
	type Currency = Balances;
	type CurrencyMatcher = ();
	type TrustedLockers = ();
	type SovereignAccountOf = ();
	type MaxLockers = ConstU32<8>;
	type WeightInfo = crate::weights::pallet_xcm::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type ReachableDest = ReachableDest;
	type AdminOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type MaxRemoteLockConsumers = ConstU32<0>;
	type RemoteLockConsumerIdentifier = ();
}

impl cumulus_pallet_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = ();
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Self>;
	type PriceForSiblingDelivery = ();
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor;
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
}

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsNativeConcrete<CurrencyId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	CurrencyId,
	CurrencyIdConvert,
	DepositToAlternative<TreasuryAccount, Currencies, CurrencyId, AccountId, Balance>,
>;

pub struct CurrencyIdConvert;
impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		use primitives::TokenSymbol::*;
		use CurrencyId::{Erc20, ForeignAsset, StableAssetPoolToken, Token};
		match id {
			Token(DOT) => Some(MultiLocation::parent()),
			Token(ACA) | Token(AUSD) | Token(LDOT) | Token(TAI) => {
				native_currency_location(ParachainInfo::get().into(), id.encode())
			}
			Erc20(address) if !is_system_contract(&address) => {
				native_currency_location(ParachainInfo::get().into(), id.encode())
			}
			StableAssetPoolToken(_pool_id) => native_currency_location(ParachainInfo::get().into(), id.encode()),
			ForeignAsset(foreign_asset_id) => AssetIdMaps::<Runtime>::get_multi_location(foreign_asset_id),
			_ => None,
		}
	}
}
impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<CurrencyId> {
		use primitives::TokenSymbol::*;
		use CurrencyId::{Erc20, StableAssetPoolToken, Token};

		if location == MultiLocation::parent() {
			return Some(Token(DOT));
		}

		if let Some(currency_id) = AssetIdMaps::<Runtime>::get_currency_id(location) {
			return Some(currency_id);
		}

		match location {
			MultiLocation {
				parents,
				interior: X2(Parachain(para_id), GeneralKey { data, length }),
			} if parents == 1 && ParaId::from(para_id) == ParachainInfo::get() => {
				// decode the general key
				let key = &data[..data.len().min(length as usize)];
				if let Ok(currency_id) = CurrencyId::decode(&mut &*key) {
					// check if `currency_id` is cross-chain asset
					match currency_id {
						Token(ACA) | Token(AUSD) | Token(LDOT) | Token(TAI) => Some(currency_id),
						Erc20(address) if !is_system_contract(&address) => Some(currency_id),
						StableAssetPoolToken(_pool_id) => Some(currency_id),
						_ => None,
					}
				} else {
					None
				}
			}
			// adapt for re-anchor canonical location: https://github.com/paritytech/polkadot/pull/4470
			MultiLocation {
				parents: 0,
				interior: X1(GeneralKey { data, length }),
			} => {
				let key = &data[..data.len().min(length as usize)];
				if let Ok(currency_id) = CurrencyId::decode(&mut &*key) {
					match currency_id {
						Token(ACA) | Token(AUSD) | Token(LDOT) | Token(TAI) => Some(currency_id),
						Erc20(address) if !is_system_contract(&address) => Some(currency_id),
						StableAssetPoolToken(_pool_id) => Some(currency_id),
						_ => None,
					}
				} else {
					None
				}
			}
			_ => None,
		}
	}
}
impl Convert<MultiAsset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: MultiAsset) -> Option<CurrencyId> {
		if let MultiAsset {
			id: Concrete(location), ..
		} = asset
		{
			Self::convert(location)
		} else {
			None
		}
	}
}

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(AccountId32 {
			network: None,
			id: account.into(),
		})
		.into()
	}
}

parameter_types! {
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_parts(100_000_000, 0);
	pub const MaxAssetsForTransfer: usize = 2;
}

parameter_type_with_key! {
	pub ParachainMinFee: |_location: MultiLocation| -> Option<u128> {
		None
	};
}

impl orml_xtokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = XcmExecutor;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type UniversalLocation = UniversalLocation;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
}
