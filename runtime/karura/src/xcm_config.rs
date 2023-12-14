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
	constants::{fee::*, parachains},
	AccountId, AllPalletsWithSystem, AssetIdMapping, AssetIdMaps, Balance, Balances, Convert, Currencies, CurrencyId,
	EvmAddressMapping, ExistentialDeposits, FixedRateOfAsset, GetNativeCurrencyId, KaruraTreasuryAccount,
	NativeTokenExistentialDeposit, ParachainInfo, ParachainSystem, PolkadotXcm, Runtime, RuntimeCall, RuntimeEvent,
	RuntimeOrigin, UnknownTokens, XcmInterface, XcmpQueue, KAR, KUSD, LKSM, TAI, XNFT,
};
use cumulus_primitives_core::ParaId;
use frame_support::{
	parameter_types,
	traits::{ConstU32, Everything, Get, Nothing},
};
use module_asset_registry::{BuyWeightRateOfErc20, BuyWeightRateOfForeignAsset, BuyWeightRateOfStableAsset};
use module_support::HomaSubAccountXcm;
use module_transaction_payment::BuyWeightRateOfTransactionFeePool;
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key};
use orml_xcm_support::{DepositToAlternative, IsNativeConcrete, MultiCurrencyAdapter, MultiNativeAsset};
use parity_scale_codec::{Decode, Encode};
use polkadot_runtime_common::xcm_sender::NoPriceForMessageDelivery;
use primitives::evm::is_system_contract;
use runtime_common::{
	local_currency_location, native_currency_location, AcalaDropAssets, EnsureRootOrHalfGeneralCouncil,
	EnsureRootOrThreeFourthsGeneralCouncil,
};
use xcm::{prelude::*, v3::Weight as XcmWeight};
use xcm_builder::{EnsureXcmOrigin, FixedRateOfFungible, FixedWeightBounds, SignedToAccountId32};

parameter_types! {
	pub const RelayNetwork: NetworkId = NetworkId::Kusama;
	pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
	pub UniversalLocation: InteriorMultiLocation = X2(GlobalConsensus(RelayNetwork::get()), Parachain(ParachainInfo::parachain_id().into()));
	pub NftPalletLocation: InteriorMultiLocation = X1(PalletInstance(121));
	pub CheckingAccount: AccountId = PolkadotXcm::check_account();
}

pub type LocationToAccountId =
	runtime_common::xcm_config::LocationToAccountId<RelayNetwork, EvmAddressMapping<Runtime>>;

pub type XcmOriginToCallOrigin = runtime_common::xcm_config::XcmOriginToCallOrigin<
	LocationToAccountId,
	RuntimeOrigin,
	RelayChainOrigin,
	RelayNetwork,
>;

pub type Barrier = runtime_common::xcm_config::Barrier<PolkadotXcm, UniversalLocation>;

pub type ToTreasury = runtime_common::xcm_config::ToTreasury<CurrencyIdConvert, KaruraTreasuryAccount, Currencies>;

parameter_types! {
	// One XCM operation is 200_000_000 weight, cross-chain transfer ~= 2x of transfer.
	pub const UnitWeightCost: XcmWeight = XcmWeight::from_parts(200_000_000, 0);
	pub const MaxInstructions: u32 = 100;
	pub KsmPerSecond: (AssetId, u128, u128) = (
		MultiLocation::parent().into(),
		ksm_per_second(),
		0
	);
	pub KusdPerSecond: (AssetId, u128, u128) = (
		local_currency_location(KUSD).unwrap().into(),
		// kUSD:KSM = 400:1
		ksm_per_second() * 400,
		0
	);
	pub KarPerSecond: (AssetId, u128, u128) = (
		local_currency_location(KAR).unwrap().into(),
		kar_per_second(),
		0
	);
	pub LksmPerSecond: (AssetId, u128, u128) = (
		local_currency_location(LKSM).unwrap().into(),
		// LKSM:KSM = 10:1
		ksm_per_second() * 10,
		0
	);
	pub TaiPerSecond: (AssetId, u128, u128) = (
		local_currency_location(TAI).unwrap().into(),
		// TAI:taiKSM = 4340:1
		ksm_per_second() * 4340,
		0
	);
	pub PHAPerSecond: (AssetId, u128, u128) = (
		MultiLocation::new(
			1,
			X1(Parachain(parachains::phala::ID)),
		).into(),
		// PHA:KSM = 400:1
		ksm_per_second() * 400,
		0
	);
	pub BncPerSecond: (AssetId, u128, u128) = (
		native_currency_location(parachains::bifrost::ID, parachains::bifrost::BNC_KEY.to_vec()).unwrap().into(),
		// BNC:KSM = 80:1
		ksm_per_second() * 80,
		0
	);
	pub VsksmPerSecond: (AssetId, u128, u128) = (
		native_currency_location(parachains::bifrost::ID, parachains::bifrost::VSKSM_KEY.to_vec()).unwrap().into(),
		// VSKSM:KSM = 1:1
		ksm_per_second(),
		0
	);
	pub KbtcPerSecond: (AssetId, u128, u128) = (
		native_currency_location(parachains::kintsugi::ID, parachains::kintsugi::KBTC_KEY.to_vec()).unwrap().into(),
		// KBTC:KSM = 1:150 & Satoshi:Planck = 1:10_000
		ksm_per_second() / 1_500_000,
		0
	);
	pub KintPerSecond: (AssetId, u128, u128) = (
		native_currency_location(parachains::kintsugi::ID, parachains::kintsugi::KINT_KEY.to_vec()).unwrap().into(),
		// KINT:KSM = 4:3
		(ksm_per_second() * 4) / 3,
		0
	);

	pub BaseRate: u128 = kar_per_second();
}

pub type Trader = (
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>,
	FixedRateOfFungible<KarPerSecond, ToTreasury>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfForeignAsset<Runtime>>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfErc20<Runtime>>,
	FixedRateOfAsset<BaseRate, ToTreasury, BuyWeightRateOfStableAsset<Runtime>>,
	FixedRateOfFungible<VsksmPerSecond, ToTreasury>,
	FixedRateOfFungible<PHAPerSecond, ToTreasury>,
	FixedRateOfFungible<KbtcPerSecond, ToTreasury>,
	FixedRateOfFungible<KintPerSecond, ToTreasury>,
	FixedRateOfFungible<KsmPerSecond, ToTreasury>,
	FixedRateOfFungible<KusdPerSecond, ToTreasury>,
	FixedRateOfFungible<LksmPerSecond, ToTreasury>,
	FixedRateOfFungible<BncPerSecond, ToTreasury>,
	FixedRateOfFungible<TaiPerSecond, ToTreasury>,
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
	type AssetLocker = ();
	type AssetExchanger = ();
	type AssetClaims = PolkadotXcm;
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

pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

/// The means for routing XCM messages which are not for local execution into the right message
/// queues.
pub type XcmRouter = (
	// Two routers - use UMP to communicate with the relay chain:
	cumulus_primitives_utility::ParentAsUmp<ParachainSystem, PolkadotXcm, ()>,
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
	type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, ()>;
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
	type VersionWrapper = PolkadotXcm;
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOrigin = EnsureRootOrHalfGeneralCouncil;
	type ControllerOriginConverter = XcmOriginToCallOrigin;
	type WeightInfo = cumulus_pallet_xcmp_queue::weights::SubstrateWeight<Self>;
	type PriceForSiblingDelivery = NoPriceForMessageDelivery<ParaId>;
}

impl cumulus_pallet_dmp_queue::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type XcmExecutor = XcmExecutor;
	type ExecuteOverweightOrigin = EnsureRootOrHalfGeneralCouncil;
}

parameter_types! {
	pub SelfLocation: MultiLocation = MultiLocation::new(1, X1(Parachain(ParachainInfo::get().into())));
}

parameter_types! {
	pub const BaseXcmWeight: XcmWeight = XcmWeight::from_parts(100_000_000, 0);
	pub const MaxAssetsForTransfer: usize = 2;
}

parameter_type_with_key! {
	pub ParachainMinFee: |location: MultiLocation| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(parachains::asset_hub_kusama::ID))) => Some(XcmInterface::get_parachain_fee(*location)),
			_ => None,
		}
	};
}

impl orml_xtokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToMultiLocation = runtime_common::xcm_config::AccountIdToMultiLocation;
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

pub type LocalAssetTransactor = (
	XNFT,
	MultiCurrencyAdapter<
		Currencies,
		UnknownTokens,
		IsNativeConcrete<CurrencyId, CurrencyIdConvert>,
		AccountId,
		LocationToAccountId,
		CurrencyId,
		CurrencyIdConvert,
		DepositToAlternative<KaruraTreasuryAccount, Currencies, CurrencyId, AccountId, Balance>,
	>,
);

pub struct CurrencyIdConvert;

impl Convert<CurrencyId, Option<MultiLocation>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<MultiLocation> {
		use primitives::TokenSymbol::*;
		use CurrencyId::{Erc20, ForeignAsset, StableAssetPoolToken, Token};
		match id {
			Token(KSM) => Some(MultiLocation::parent()),
			Token(KAR) | Token(KUSD) | Token(LKSM) | Token(TAI) => {
				native_currency_location(ParachainInfo::get().into(), id.encode())
			}
			Erc20(address) if !is_system_contract(&address) => {
				native_currency_location(ParachainInfo::get().into(), id.encode())
			}
			StableAssetPoolToken(_pool_id) => native_currency_location(ParachainInfo::get().into(), id.encode()),
			// Bifrost native token
			Token(BNC) => native_currency_location(parachains::bifrost::ID, parachains::bifrost::BNC_KEY.to_vec()),
			// Bifrost Voucher Slot KSM
			Token(VSKSM) => native_currency_location(parachains::bifrost::ID, parachains::bifrost::VSKSM_KEY.to_vec()),
			// Phala Native token
			Token(PHA) => Some(MultiLocation::new(1, X1(Parachain(parachains::phala::ID)))),
			// Kintsugi Native token
			Token(KINT) => native_currency_location(parachains::kintsugi::ID, parachains::kintsugi::KINT_KEY.to_vec()),
			// Kintsugi wrapped BTC
			Token(KBTC) => native_currency_location(parachains::kintsugi::ID, parachains::kintsugi::KBTC_KEY.to_vec()),
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
			return Some(Token(KSM));
		}

		if let Some(currency_id) = AssetIdMaps::<Runtime>::get_currency_id(location) {
			return Some(currency_id);
		}

		match location {
			MultiLocation {
				parents: 1,
				interior: X2(Parachain(para_id), GeneralKey { data, length }),
			} => {
				match (para_id, &data[..data.len().min(length as usize)]) {
					(parachains::bifrost::ID, parachains::bifrost::BNC_KEY) => Some(Token(BNC)),
					(parachains::bifrost::ID, parachains::bifrost::VSKSM_KEY) => Some(Token(VSKSM)),
					(parachains::kintsugi::ID, parachains::kintsugi::KINT_KEY) => Some(Token(KINT)),
					(parachains::kintsugi::ID, parachains::kintsugi::KBTC_KEY) => Some(Token(KBTC)),

					(id, key) if id == u32::from(ParachainInfo::get()) => {
						// Karura
						if let Ok(currency_id) = CurrencyId::decode(&mut &*key) {
							// check `currency_id` is cross-chain asset
							match currency_id {
								Token(KAR) | Token(KUSD) | Token(LKSM) | Token(TAI) => Some(currency_id),
								Erc20(address) if !is_system_contract(&address) => Some(currency_id),
								StableAssetPoolToken(_pool_id) => Some(currency_id),
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
				interior: X1(GeneralKey { data, length }),
			} => {
				let key = &data[..data.len().min(length as usize)];
				let currency_id = CurrencyId::decode(&mut &*key).ok()?;
				match currency_id {
					Token(KAR) | Token(KUSD) | Token(LKSM) | Token(TAI) => Some(currency_id),
					Erc20(address) if !is_system_contract(&address) => Some(currency_id),
					StableAssetPoolToken(_pool_id) => Some(currency_id),
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
