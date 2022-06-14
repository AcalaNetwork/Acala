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

use super::{
	constants::{fee::*, parachains},
	AccountId, AssetIdMapping, AssetIdMaps, Balance, Call, Convert, Currencies, CurrencyId, Event, ExistentialDeposits,
	FixedRateOfAsset, GetNativeCurrencyId, KaruraTreasuryAccount, NativeTokenExistentialDeposit, Origin, ParachainInfo,
	ParachainSystem, PolkadotXcm, Runtime, UnknownTokens, XcmInterface, XcmpQueue, KAR, KUSD, LKSM,
};
use codec::{Decode, Encode};
pub use cumulus_primitives_core::ParaId;
pub use frame_support::{
	parameter_types,
	traits::{Everything, Get, Nothing},
	weights::Weight,
};
pub use module_asset_registry::{BuyWeightRateOfErc20, BuyWeightRateOfForeignAsset};
use module_support::HomaSubAccountXcm;
use module_transaction_payment::BuyWeightRateOfTransactionFeePool;
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key, MultiCurrency};
use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset};
use pallet_xcm::XcmPassthrough;
use polkadot_parachain::primitives::Sibling;
use primitives::evm::is_system_contract;
use runtime_common::{native_currency_location, AcalaDropAssets, EnsureRootOrHalfGeneralCouncil};
use xcm::latest::prelude::*;
pub use xcm_builder::{
	AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom, AllowTopLevelPaidExecutionFrom,
	AllowUnpaidExecutionFrom, EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, IsConcrete, LocationInverter,
	NativeAsset, ParentAsSuperuser, ParentIsPreset, RelayChainAsNative, SiblingParachainAsNative,
	SiblingParachainConvertsVia, SignedAccountId32AsNative, SignedToAccountId32, SovereignSignedViaLocation,
	TakeRevenue, TakeWeightCredit,
};
use xcm_executor::XcmExecutor;

parameter_types! {
	pub KsmLocation: MultiLocation = MultiLocation::parent();
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub RelayChainOrigin: Origin = cumulus_pallet_xcm::Origin::Relay.into();
	pub Ancestry: MultiLocation = Parachain(ParachainInfo::parachain_id().into()).into();
}

/// Type for specifying how a `MultiLocation` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
	// The parent (Relay-chain) origin converts to the default `AccountId`.
	ParentIsPreset<AccountId>,
	// Sibling parachain origins convert to AccountId via the `ParaId::into`.
	SiblingParachainConvertsVia<Sibling, AccountId>,
	// Straight up local `AccountId32` origins just alias directly to `AccountId`.
	AccountId32Aliases<RelayNetwork, AccountId>,
);

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToCallOrigin = (
	// Sovereign account converter; this attempts to derive an `AccountId` from the origin location
	// using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
	// foreign chains who want to have a local sovereign account on this chain which they control.
	SovereignSignedViaLocation<LocationToAccountId, Origin>,
	// Native converter for Relay-chain (Parent) location; will converts to a `Relay` origin when
	// recognized.
	RelayChainAsNative<RelayChainOrigin, Origin>,
	// Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
	// recognized.
	SiblingParachainAsNative<cumulus_pallet_xcm::Origin, Origin>,
	// Native signed account converter; this just converts an `AccountId32` origin into a normal
	// `Origin::Signed` origin of the same 32-byte value.
	SignedAccountId32AsNative<RelayNetwork, Origin>,
	// Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
	XcmPassthrough<Origin>,
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
				// Ensure KaruraTreasuryAccount have ed requirement for native asset, but don't need
				// ed requirement for cross-chain asset because it's one of whitelist accounts.
				// Ignore the result.
				let _ = Currencies::deposit(currency_id, &KaruraTreasuryAccount::get(), amount);
			}
		}
	}
}

parameter_types! {
	// One XCM operation is 200_000_000 weight, cross-chain transfer ~= 2x of transfer.
	pub const UnitWeightCost: Weight = 200_000_000;
	pub const MaxInstructions: u32 = 100;
	pub KsmPerSecond: (AssetId, u128) = (MultiLocation::parent().into(), ksm_per_second());
	pub KusdPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			0,
			X1(GeneralKey(KUSD.encode())),
		).into(),
		// kUSD:KSM = 400:1
		ksm_per_second() * 400
	);
	pub KarPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			0,
			X1(GeneralKey(KAR.encode())),
		).into(),
		kar_per_second()
	);
	pub LksmPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			0,
			X1(GeneralKey(LKSM.encode())),
		).into(),
		// LKSM:KSM = 10:1
		ksm_per_second() * 10
	);
	pub PHAPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X1(Parachain(parachains::phala::ID)),
		).into(),
		// PHA:KSM = 400:1
		ksm_per_second() * 400
	);
	pub BncPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(Parachain(parachains::bifrost::ID), GeneralKey(parachains::bifrost::BNC_KEY.to_vec())),
		).into(),
		// BNC:KSM = 80:1
		ksm_per_second() * 80
	);
	pub VsksmPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(Parachain(parachains::bifrost::ID), GeneralKey(parachains::bifrost::VSKSM_KEY.to_vec())),
		).into(),
		// VSKSM:KSM = 1:1
		ksm_per_second()
	);
	pub KbtcPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(Parachain(parachains::kintsugi::ID), GeneralKey(parachains::kintsugi::KBTC_KEY.to_vec())),
		).into(),
		// KBTC:KSM = 1:150 & Satoshi:Planck = 1:10_000
		ksm_per_second() / 1_500_000
	);
	pub KintPerSecond: (AssetId, u128) = (
		MultiLocation::new(
			1,
			X2(Parachain(parachains::kintsugi::ID), GeneralKey(parachains::kintsugi::KINT_KEY.to_vec())),
		).into(),
		// KINT:KSM = 4:3
		(ksm_per_second() * 4) / 3
	);

	pub BaseRate: u128 = kar_per_second();
}

pub type Trader = (
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>,
	FixedRateOfFungible<KsmPerSecond, ToTreasury>,
	FixedRateOfFungible<KusdPerSecond, ToTreasury>,
	FixedRateOfFungible<KarPerSecond, ToTreasury>,
	FixedRateOfFungible<LksmPerSecond, ToTreasury>,
	FixedRateOfFungible<BncPerSecond, ToTreasury>,
	FixedRateOfFungible<VsksmPerSecond, ToTreasury>,
	FixedRateOfFungible<PHAPerSecond, ToTreasury>,
	FixedRateOfFungible<KbtcPerSecond, ToTreasury>,
	FixedRateOfFungible<KintPerSecond, ToTreasury>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfForeignAsset<Runtime>>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfErc20<Runtime>>,
);

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
	type Call = Call;
	type XcmSender = XcmRouter;
	// How to withdraw and deposit an asset.
	type AssetTransactor = LocalAssetTransactor;
	type OriginConverter = XcmOriginToCallOrigin;
	type IsReserve = MultiNativeAsset<AbsoluteReserveProvider>;
	// Teleporting is disabled.
	type IsTeleporter = ();
	type LocationInverter = LocationInverter<Ancestry>;
	type Barrier = Barrier;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type Trader = Trader;
	type ResponseHandler = PolkadotXcm;
	type AssetTrap = AcalaDropAssets<
		PolkadotXcm,
		ToTreasury,
		CurrencyIdConvert,
		GetNativeCurrencyId,
		NativeTokenExistentialDeposit,
		ExistentialDeposits,
	>;
	type AssetClaims = PolkadotXcm;
	type SubscriptionService = PolkadotXcm;
}

pub type LocalOriginToLocation = SignedToAccountId32<Origin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm>,
	// ..and XCMP to communicate with the sibling chains.
	XcmpQueue,
);

impl pallet_xcm::Config for Runtime {
	type Event = Event;
	type SendXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmRouter = XcmRouter;
	type ExecuteXcmOrigin = EnsureXcmOrigin<Origin, LocalOriginToLocation>;
	type XcmExecuteFilter = Nothing;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type XcmTeleportFilter = Nothing;
	type XcmReserveTransferFilter = Everything;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type LocationInverter = LocationInverter<Ancestry>;
	type Origin = Origin;
	type Call = Call;
	const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
	type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
}

impl cumulus_pallet_xcm::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
}

impl cumulus_pallet_xcmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ChannelInfo = ParachainSystem;
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Self>;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type Event = Event;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
}

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

pub struct AccountIdToMultiLocation;
impl Convert<AccountId, MultiLocation> for AccountIdToMultiLocation {
	fn convert(account: AccountId) -> MultiLocation {
		X1(AccountId32 {
			network: NetworkId::Any,
			id: account.into(),
		})
		.into()
	}
}

parameter_types! {
	pub const BaseXcmWeight: Weight = 100_000_000;
	pub const MaxAssetsForTransfer: usize = 2;
}

parameter_type_with_key! {
	pub ParachainMinFee: |location: MultiLocation| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(parachains::statemine::ID))) => Some(XcmInterface::get_parachain_fee(location.clone())),
			_ => None,
		}
	};
}

impl orml_xtokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = AccountIdToMultiLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = XcmExecutor<XcmConfig>;
	type Weigher = FixedWeightBounds<UnitWeightCost, Call, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type LocationInverter = LocationInverter<Ancestry>;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type MultiLocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
}

pub type LocalAssetTransactor = MultiCurrencyAdapter<
	Currencies,
	UnknownTokens,
	IsNativeConcrete<CurrencyId, CurrencyIdConvert>,
	AccountId,
	LocationToAccountId,
	CurrencyId,
	CurrencyIdConvert,
	DepositToAlternative<KaruraTreasuryAccount, Currencies, CurrencyId, AccountId, Balance>,
>;

pub struct CurrencyIdConvert;

impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		use primitives::TokenSymbol::*;
		use CurrencyId::{Erc20, ForeignAsset, Token};
		match id {
			Token(KSM) => Some(MultiLocation::parent()),
			Token(KAR) | Token(KUSD) | Token(LKSM) => Some(native_currency_location(ParachainInfo::get().into(), id)),
			Erc20(address) if !is_system_contract(address) => {
				Some(native_currency_location(ParachainInfo::get().into(), id))
			}
			// Bifrost native token
			Token(BNC) => Some(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::bifrost::ID),
					GeneralKey(parachains::bifrost::BNC_KEY.to_vec()),
				),
			)),
			// Bifrost Voucher Slot KSM
			Token(VSKSM) => Some(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::bifrost::ID),
					GeneralKey(parachains::bifrost::VSKSM_KEY.to_vec()),
				),
			)),
			// Phala Native token
			Token(PHA) => Some(MultiLocation::new(1, X1(Parachain(parachains::phala::ID)))),
			// Kintsugi Native token
			Token(KINT) => Some(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::kintsugi::ID),
					GeneralKey(parachains::kintsugi::KINT_KEY.to_vec()),
				),
			)),
			// Kintsugi wrapped BTC
			Token(KBTC) => Some(MultiLocation::new(
				1,
				X2(
					Parachain(parachains::kintsugi::ID),
					GeneralKey(parachains::kintsugi::KBTC_KEY.to_vec()),
				),
			)),
			ForeignAsset(foreign_asset_id) => AssetIdMaps::<Runtime>::get_multi_location(foreign_asset_id),
			_ => None,
		}
	}
}

impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<CurrencyId> {
		use primitives::TokenSymbol::*;
		use CurrencyId::{Erc20, Token};

		if location == MultiLocation::parent() {
			return Some(Token(KSM));
		}

		if let Some(currency_id) = AssetIdMaps::<Runtime>::get_currency_id(location.clone()) {
			return Some(currency_id);
		}

		match location {
			MultiLocation {
				parents: 1,
				interior: X2(Parachain(para_id), GeneralKey(key)),
			} => {
				match (para_id, &key[..]) {
					(parachains::bifrost::ID, parachains::bifrost::BNC_KEY) => Some(Token(BNC)),
					(parachains::bifrost::ID, parachains::bifrost::VSKSM_KEY) => Some(Token(VSKSM)),
					(parachains::kintsugi::ID, parachains::kintsugi::KINT_KEY) => Some(Token(KINT)),
					(parachains::kintsugi::ID, parachains::kintsugi::KBTC_KEY) => Some(Token(KBTC)),

					(id, key) if id == u32::from(ParachainInfo::get()) => {
						// Karura
						if let Ok(currency_id) = CurrencyId::decode(&mut &*key) {
							// check `currency_id` is cross-chain asset
							match currency_id {
								Token(KAR) | Token(KUSD) | Token(LKSM) => Some(currency_id),
								Erc20(address) if !is_system_contract(address) => Some(currency_id),
								_ => None,
							}
						} else {
							// invalid general key
							None
						}
					}
					_ => None,
				}
			}
			MultiLocation {
				parents: 1,
				interior: X1(Parachain(parachains::phala::ID)),
			} => Some(Token(PHA)),
			// adapt for re-anchor canonical location: https://github.com/paritytech/polkadot/pull/4470
			MultiLocation {
				parents: 0,
				interior: X1(GeneralKey(key)),
			} => {
				let currency_id = CurrencyId::decode(&mut &*key).ok()?;
				match currency_id {
					Token(KAR) | Token(KUSD) | Token(LKSM) => Some(currency_id),
					Erc20(address) if !is_system_contract(address) => Some(currency_id),
					_ => None,
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
