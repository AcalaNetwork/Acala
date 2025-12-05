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

//! The Dev runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit.
#![recursion_limit = "512"]
#![allow(clippy::unnecessary_mut_passed)]
#![allow(clippy::or_fun_call)]
#![allow(clippy::from_over_into)]
#![allow(clippy::upper_case_acronyms)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

extern crate alloc;
use alloc::borrow::Cow;
use parity_scale_codec::{Decode, DecodeLimit, DecodeWithMemTracking, Encode};
use polkadot_parachain_primitives::primitives::Sibling;
use scale_info::TypeInfo;
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160};
use sp_runtime::{
	generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BadOrigin, BlakeTwo256, Block as BlockT, Bounded, Convert,
		IdentityLookup, SaturatedConversion, StaticLookup,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ArithmeticError, DispatchResult, FixedPointNumber, Perbill, Percent, Permill, Perquintill,
	RuntimeDebug,
};
use sp_std::prelude::*;
use xcm::{
	prelude::AssetId, Version as XcmVersion, VersionedAssetId, VersionedAssets, VersionedLocation, VersionedXcm,
};
use xcm_runtime_apis::{
	dry_run::{CallDryRunEffects, Error as XcmDryRunApiError, XcmDryRunEffects},
	fees::Error as XcmPaymentApiError,
};

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use module_asset_registry::{AssetIdMaps, EvmErc20InfoMapping};
use module_assethub::AssetHubCallBuilder;
use module_cdp_engine::CollateralCurrencyIds;
use module_currencies::BasicCurrencyAdapter;
use module_evm::{runner::RunnerExtended, CallInfo, CreateInfo, EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_support::{AddressMapping, AssetIdMapping, DispatchableTask, ExchangeRateProvider, FractionalRate, PoolId};
use module_transaction_payment::TargetedFeeAdjustment;

use cumulus_pallet_parachain_system::RelaychainDataProvider;
use orml_traits::{
	create_median_value_data_provider, define_aggregrated_parameters, parameter_type_with_key,
	parameters::ParameterStoreAdapter, DataFeeder, DataProviderExtended, GetByKey, MultiCurrency,
};
use pallet_transaction_payment::RuntimeDispatchInfo;

use frame_support::{
	construct_runtime,
	pallet_prelude::InvalidTransaction,
	parameter_types,
	traits::{
		fungible::HoldConsideration,
		tokens::{PayFromAccount, UnityAssetBalanceConversion},
		ConstBool, ConstU128, ConstU32, ConstU64, Contains, ContainsLengthBound, Currency as PalletCurrency, Currency,
		EnsureOrigin, EqualPrivilegeOnly, Get, Imbalance, InstanceFilter, LinearStoragePrice, LockIdentifier,
		OnUnbalanced, SortedMembers,
	},
	transactional,
	weights::{constants::RocksDbWeight, ConstantMultiplier, Weight},
	PalletId, MAX_EXTRINSIC_DEPTH,
};

pub use pallet_collective::MemberCount;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use authority::AuthorityConfigImpl;
pub use constants::{fee::*, parachains, time::*};
pub use primitives::{
	currency::AssetIds,
	define_combined_task,
	evm::{
		decode_gas_limit, decode_gas_price, AccessListItem, BlockLimits, EstimateResourcesRequest,
		EthereumTransactionMessage, EvmAddress,
	},
	task::TaskResult,
	unchecked_extrinsic::AcalaUncheckedExtrinsic,
	AccountId, AccountIndex, Address, Amount, AuctionId, AuthoritysOriginId, Balance, BlockNumber, CurrencyId,
	DataProviderId, EraIndex, Hash, Lease, Moment, Multiplier, Nonce, ReserveIdentifier, Share, Signature, TokenSymbol,
	TradingPair,
};
use runtime_common::{
	cent, dollar, microcent, millicent, AllPrecompiles, CheckRelayNumber, ConsensusHook, CurrencyHooks,
	EnsureRootOrAllGeneralCouncil, EnsureRootOrAllTechnicalCommittee, EnsureRootOrHalfFinancialCouncil,
	EnsureRootOrHalfGeneralCouncil, EnsureRootOrHalfHomaCouncil, EnsureRootOrOneGeneralCouncil,
	EnsureRootOrOneTechnicalCommittee, EnsureRootOrOneThirdsTechnicalCommittee, EnsureRootOrThreeFourthsGeneralCouncil,
	EnsureRootOrTwoThirdsGeneralCouncil, EnsureRootOrTwoThirdsTechnicalCommittee, ExchangeRate,
	ExistentialDepositsTimesOneHundred, FinancialCouncilInstance, FinancialCouncilMembershipInstance, GasToWeight,
	GeneralCouncilInstance, GeneralCouncilMembershipInstance, HomaCouncilInstance, HomaCouncilMembershipInstance,
	MaxTipsOfPriority, OperationalFeeMultiplier, OperatorMembershipInstanceAcala, Price, ProxyType, RandomnessSource,
	Rate, Ratio, RuntimeBlockLength, RuntimeBlockWeights, TechnicalCommitteeInstance,
	TechnicalCommitteeMembershipInstance, TimeStampedPrice, TipPerWeightStep, KAR, KSM, KUSD, LKSM, TAI,
};

/// Import the stable_asset pallet.
pub use nutsfinance_stable_asset;

mod authority;
#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;
pub mod constants;
#[cfg(feature = "genesis-builder")]
mod genesis_config_presets;
/// Weights for pallets used in the runtime.
mod weights;
pub mod xcm_config;

/// This runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: Cow::Borrowed("karura"),
	impl_name: Cow::Borrowed("karura"),
	authoring_version: 1,
	spec_version: 2330,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 2,
	system_version: 1,
};

/// The version information used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

impl_opaque_keys! {
	pub struct SessionKeys {
		pub aura: Aura,
	}
}

// Pallet accounts of runtime
parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub const LoansPalletId: PalletId = PalletId(*b"aca/loan");
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub const CDPEnginePalletId: PalletId = PalletId(*b"aca/cdpe");
	pub const HonzonTreasuryPalletId: PalletId = PalletId(*b"aca/hztr");
	pub const HomaPalletId: PalletId = PalletId(*b"aca/homa");
	pub const HomaTreasuryPalletId: PalletId = PalletId(*b"aca/hmtr");
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
	pub const CollatorPotId: PalletId = PalletId(*b"aca/cpot");
	pub const HonzonBridgePalletId: PalletId = PalletId(*b"aca/hzbg");
	pub const NomineesElectionId: LockIdentifier = *b"aca/nome";
	// Treasury reserve
	pub const TreasuryReservePalletId: PalletId = PalletId(*b"aca/reve");
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
	pub const XnftPalletId: PalletId = PalletId(*b"aca/xNFT");
	// Vault all unrleased native token.
	pub UnreleasedNativeVaultAccountId: AccountId = PalletId(*b"aca/urls").into_account_truncating();
	// This Pallet is only used to payment fee pool, it's not added to whitelist by design.
	// because transaction payment pallet will ensure the accounts always have enough ED.
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	// Ecosystem modules
	pub const StableAssetPalletId: PalletId = PalletId(*b"nuts/sta");
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![
		LoansPalletId::get().into_account_truncating(),
		CDPEnginePalletId::get().into_account_truncating(),
		CDPTreasuryPalletId::get().into_account_truncating(),
		CollatorPotId::get().into_account_truncating(),
		DEXPalletId::get().into_account_truncating(),
		HomaPalletId::get().into_account_truncating(),
		HomaTreasuryPalletId::get().into_account_truncating(),
		HonzonTreasuryPalletId::get().into_account_truncating(),
		IncentivesPalletId::get().into_account_truncating(),
		TreasuryPalletId::get().into_account_truncating(),
		TreasuryReservePalletId::get().into_account_truncating(),
		UnreleasedNativeVaultAccountId::get(),
		StableAssetPalletId::get().into_account_truncating(),
		HonzonBridgePalletId::get().into_account_truncating(),
	]
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 1200; // mortal tx can be valid up to 4 hour after signing
	pub const Version: RuntimeVersion = VERSION;
	pub const SS58Prefix: u8 = 8; // Ss58AddressFormat::KaruraAccount
}

pub struct BaseCallFilter;
impl Contains<RuntimeCall> for BaseCallFilter {
	fn contains(call: &RuntimeCall) -> bool {
		let is_core_call = matches!(
			call,
			RuntimeCall::System(_) | RuntimeCall::Timestamp(_) | RuntimeCall::ParachainSystem(_)
		);
		if is_core_call {
			// always allow core call
			return true;
		}

		let is_paused = module_transaction_pause::PausedTransactionFilter::<Runtime>::contains(call);
		if is_paused {
			// no paused call
			return false;
		}

		if let RuntimeCall::PolkadotXcm(xcm_method) = call {
			match xcm_method {
				// xcm transfers, use xtokens
				pallet_xcm::Call::send { .. }
				| pallet_xcm::Call::execute { .. }
				| pallet_xcm::Call::teleport_assets { .. }
				| pallet_xcm::Call::reserve_transfer_assets { .. }
				| pallet_xcm::Call::limited_reserve_transfer_assets { .. }
				| pallet_xcm::Call::limited_teleport_assets { .. } => {
					return false;
				}
				// user xcm calls
				pallet_xcm::Call::claim_assets { .. }
				| pallet_xcm::Call::transfer_assets { .. }
				| pallet_xcm::Call::transfer_assets_using_type_and_then { .. } => {
					return true;
				}
				pallet_xcm::Call::add_authorized_alias { .. }
				| pallet_xcm::Call::remove_authorized_alias { .. }
				| pallet_xcm::Call::remove_all_authorized_aliases { .. } => {
					return false;
				}
				// xcm operations call
				pallet_xcm::Call::force_xcm_version { .. }
				| pallet_xcm::Call::force_default_xcm_version { .. }
				| pallet_xcm::Call::force_subscribe_version_notify { .. }
				| pallet_xcm::Call::force_unsubscribe_version_notify { .. }
				| pallet_xcm::Call::force_suspension { .. } => {
					return true;
				}
				pallet_xcm::Call::__Ignore { .. } => {
					unimplemented!()
				}
			}
		}

		true
	}
}

impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type RuntimeCall = RuntimeCall;
	type Lookup = (AccountIdLookup<AccountId, AccountIndex>, EvmAccounts);
	type Nonce = Nonce;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type Block = Block;
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = RuntimeBlockLength;
	type Version = Version;
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = (
		module_evm::CallKillAccount<Runtime>,
		module_evm_accounts::CallKillAccount<Runtime>,
	);
	type DbWeight = RocksDbWeight;
	type BaseCallFilter = BaseCallFilter;
	type SystemWeightInfo = ();
	type ExtensionsWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
	type MaxConsumers = ConstU32<16>;
	type RuntimeTask = ();
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

impl pallet_aura::Config for Runtime {
	type AuthorityId = AuraId;
	type DisabledValidators = ();
	type MaxAuthorities = ConstU32<32>;
	type AllowMultipleBlocksPerSlot = ConstBool<false>;
	type SlotDuration = ConstU64<SLOT_DURATION>;
}

impl pallet_authorship::Config for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type EventHandler = CollatorSelection;
}

parameter_types! {
	pub const SessionDuration: BlockNumber = 6 * HOURS; // used in SessionManagerConfig of genesis
}

impl pallet_session::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type ValidatorId = <Self as frame_system::Config>::AccountId;
	// we don't have stash and controller, thus we don't need the convert as well.
	type ValidatorIdOf = module_collator_selection::IdentityCollator;
	type ShouldEndSession = SessionManager;
	type NextSessionRotation = SessionManager;
	type SessionManager = CollatorSelection;
	// Essentially just Aura, but lets be pedantic.
	type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
	type Keys = SessionKeys;
	type WeightInfo = ();
	type DisablingStrategy = ();
}

parameter_types! {
	pub const CollatorKickThreshold: Permill = Permill::from_percent(75);
}

impl module_collator_selection::Config for Runtime {
	type Currency = Balances;
	type ValidatorSet = Session;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type PotId = CollatorPotId;
	type MinCandidates = ConstU32<1>;
	type MaxCandidates = ConstU32<50>;
	type MaxInvulnerables = ConstU32<10>;
	type KickPenaltySessionLength = ConstU32<8>;
	type CollatorKickThreshold = CollatorKickThreshold;
	type MinRewardDistributeAmount = ConstU128<0>;
	type WeightInfo = weights::module_collator_selection::WeightInfo<Runtime>;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = Aura;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

// pallet-treasury did not impl OnUnbalanced<Credit>, need an adapter to handle dust.
type CreditOf = frame_support::traits::fungible::Credit<<Runtime as frame_system::Config>::AccountId, Balances>;
pub struct DustRemovalAdapter;
impl OnUnbalanced<CreditOf> for DustRemovalAdapter {
	fn on_nonzero_unbalanced(amount: CreditOf) {
		let new_amount = NegativeImbalance::new(amount.peek());
		Treasury::on_nonzero_unbalanced(new_amount);
	}
}

parameter_types! {
	pub NativeTokenExistentialDeposit: Balance = 10 * cent(KAR);	// 0.1 KAR
	pub const MaxReserves: u32 = ReserveIdentifier::Count as u32;
	// For weight estimation, we assume that the most locks on an individual account will be 50.
	// This number may need to be adjusted in the future if this assumption no longer holds true.
	pub const MaxLocks: u32 = 50;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = DustRemovalAdapter;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = NativeTokenExistentialDeposit;
	type AccountStore = module_support::SystemAccountStore<Runtime>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type DoneSlashHandler = ();
}

parameter_types! {
	/// The fee to be paid for making a transaction; the per-byte portion.
	pub TransactionByteFee: Balance = millicent(KAR);
	/// The portion of the `NORMAL_DISPATCH_RATIO` that we adjust the fees with. Blocks filled less
	/// than this will decrease the weight and more will increase.
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	/// The adjustment variable of the runtime. Higher values will cause `TargetBlockFullness` to
	/// change the fees more rapidly.
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(3, 100_000);
	/// Minimum amount of the multiplier. This value cannot be too low. A test case should ensure
	/// that combined with `AdjustmentVariable`, we can recover from the minimum.
	/// See `multiplier_can_grow_from_zero`.
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000u128);
	pub MaximumMultiplier: Multiplier = Bounded::max_value();
}

pub type SlowAdjustingFeeUpdate<R> =
	TargetedFeeAdjustment<R, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier, MaximumMultiplier>;

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = ();
}

parameter_types! {
	pub const GeneralCouncilMotionDuration: BlockNumber = 3 * DAYS;
	pub const CouncilDefaultMaxProposals: u32 = 20;
	pub const CouncilDefaultMaxMembers: u32 = 30;
	pub MaxProposalWeight: Weight = Perbill::from_percent(60) * RuntimeBlockWeights::get().max_block;
}

impl pallet_collective::Config<GeneralCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = GeneralCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
	type DisapproveOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type Consideration = ();
}

impl pallet_membership::Config<GeneralCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type SwapOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type ResetOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type MembershipInitialized = GeneralCouncil;
	type MembershipChanged = GeneralCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const FinancialCouncilMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<FinancialCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = FinancialCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
	type DisapproveOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type Consideration = ();
}

impl pallet_membership::Config<FinancialCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = FinancialCouncil;
	type MembershipChanged = FinancialCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const HomaCouncilMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<HomaCouncilInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = HomaCouncilMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
	type DisapproveOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type Consideration = ();
}

impl pallet_membership::Config<HomaCouncilMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = HomaCouncil;
	type MembershipChanged = HomaCouncil;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

parameter_types! {
	pub const TechnicalCommitteeMotionDuration: BlockNumber = 3 * DAYS;
}

impl pallet_collective::Config<TechnicalCommitteeInstance> for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Proposal = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type MotionDuration = TechnicalCommitteeMotionDuration;
	type MaxProposals = CouncilDefaultMaxProposals;
	type MaxMembers = CouncilDefaultMaxMembers;
	type DefaultVote = pallet_collective::PrimeDefaultVote;
	type SetMembersOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
	type MaxProposalWeight = MaxProposalWeight;
	type DisapproveOrigin = EnsureRoot<AccountId>;
	type KillOrigin = EnsureRoot<AccountId>;
	type Consideration = ();
}

impl pallet_membership::Config<TechnicalCommitteeMembershipInstance> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = TechnicalCommittee;
	type MembershipChanged = TechnicalCommittee;
	type MaxMembers = CouncilDefaultMaxMembers;
	type WeightInfo = ();
}

impl pallet_membership::Config<OperatorMembershipInstanceAcala> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AddOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type SwapOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type ResetOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type MembershipInitialized = ();
	type MembershipChanged = AcalaOracle;
	type MaxMembers = ConstU32<50>;
	type WeightInfo = ();
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

parameter_types! {
	pub MultisigDepositBase: Balance = deposit(1, 88);
	pub MultisigDepositFactor: Balance = deposit(0, 32);
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = MultisigDepositBase;
	type DepositFactor = MultisigDepositFactor;
	type MaxSignatories = ConstU32<100>;
	type WeightInfo = ();
	type BlockNumberProvider = System;
}

pub struct GeneralCouncilProvider;
impl SortedMembers<AccountId> for GeneralCouncilProvider {
	fn contains(who: &AccountId) -> bool {
		GeneralCouncil::is_member(who)
	}

	fn sorted_members() -> Vec<AccountId> {
		pallet_collective::Members::<Runtime, pallet_collective::Instance1>::get() // GeneralCouncil::members()
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add(_: &AccountId) {
		unimplemented!()
	}
}

impl ContainsLengthBound for GeneralCouncilProvider {
	fn max_len() -> usize {
		CouncilDefaultMaxMembers::get() as usize
	}
	fn min_len() -> usize {
		0
	}
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub ProposalBondMinimum: Balance = 5 * dollar(KAR);
	pub ProposalBondMaximum: Balance = 25 * dollar(KAR);
	pub const SpendPeriod: BlockNumber = 30 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);

	pub const TipCountdown: BlockNumber = DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(5);
	pub TipReportDepositBase: Balance = deposit(1, 0);
	pub BountyDepositBase: Balance = deposit(1, 0);
	pub const BountyDepositPayoutDelay: BlockNumber = 4 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 35 * DAYS;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub CuratorDepositMin: Balance = dollar(KAR);
	pub CuratorDepositMax: Balance = 100 * dollar(KAR);
	pub BountyValueMinimum: Balance = 5 * dollar(KAR);
	pub DataDepositPerByte: Balance = deposit(0, 1);
	pub const MaximumReasonLength: u32 = 8192;
	pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;

	pub const SevenDays: BlockNumber = 7 * DAYS;
	pub const OneDay: BlockNumber = DAYS;
}

impl pallet_treasury::Config for Runtime {
	type PalletId = TreasuryPalletId;
	type Currency = Balances;
	type RejectOrigin = EnsureRootOrHalfGeneralCouncil;
	type SpendOrigin = frame_support::traits::NeverEnsureOrigin<Balance>;
	type RuntimeEvent = RuntimeEvent;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type SpendFunds = Bounties;
	type WeightInfo = ();
	type MaxApprovals = ConstU32<30>;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
	type Paymaster = PayFromAccount<Balances, KaruraTreasuryAccount>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutSpendPeriod;
	type BlockNumberProvider = System;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

impl pallet_bounties::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type BountyDepositBase = BountyDepositBase;
	type BountyDepositPayoutDelay = BountyDepositPayoutDelay;
	type BountyUpdatePeriod = BountyUpdatePeriod;
	type BountyValueMinimum = BountyValueMinimum;
	type CuratorDepositMultiplier = CuratorDepositMultiplier;
	type CuratorDepositMin = CuratorDepositMin;
	type CuratorDepositMax = CuratorDepositMax;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type WeightInfo = ();
	type ChildBountyManager = ();
	type OnSlash = Treasury;
}

impl pallet_tips::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type DataDepositPerByte = DataDepositPerByte;
	type MaximumReasonLength = MaximumReasonLength;
	type Tippers = GeneralCouncilProvider;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type MaxTipAmount = ();
	type WeightInfo = ();
	type OnSlash = Treasury;
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 5 * DAYS;
	pub const VotingPeriod: BlockNumber = 5 * DAYS;
	pub const FastTrackVotingPeriod: BlockNumber = 3 * HOURS;
	pub MinimumDeposit: Balance = 100 * dollar(KAR);
	pub const EnactmentPeriod: BlockNumber = 2 * DAYS;
	pub const VoteLockingPeriod: BlockNumber = 7 * DAYS;
	pub const CooloffPeriod: BlockNumber = 7 * DAYS;
}

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = VoteLockingPeriod;
	type MinimumDeposit = MinimumDeposit;
	/// A straight majority of the council can decide what their next motion is.
	type ExternalOrigin = EnsureRootOrHalfGeneralCouncil;
	/// A majority can have the next scheduled referendum be a straight majority-carries vote.
	type ExternalMajorityOrigin = EnsureRootOrHalfGeneralCouncil;
	/// A unanimous council can have the next scheduled referendum be a straight default-carries
	/// (NTB) vote.
	type ExternalDefaultOrigin = EnsureRootOrAllGeneralCouncil;
	/// Two thirds of the technical committee can have an ExternalMajority/ExternalDefault vote
	/// be tabled immediately and with a shorter voting/enactment period.
	type FastTrackOrigin = EnsureRootOrTwoThirdsTechnicalCommittee;
	type InstantOrigin = EnsureRootOrAllTechnicalCommittee;
	type InstantAllowed = ConstBool<true>;
	type FastTrackVotingPeriod = FastTrackVotingPeriod;
	// To cancel a proposal which has been passed, 2/3 of the council must agree to it.
	type CancellationOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type BlacklistOrigin = EnsureRoot<AccountId>;
	// To cancel a proposal before it has been passed, the technical committee must be unanimous or
	// Root must agree.
	type CancelProposalOrigin = EnsureRootOrAllTechnicalCommittee;
	// Any single technical committee member may veto a coming council proposal, however they can
	// only do it once and it lasts only for the cooloff period.
	type VetoOrigin = pallet_collective::EnsureMember<AccountId, TechnicalCommitteeInstance>;
	type CooloffPeriod = CooloffPeriod;
	type Slash = Treasury;
	type Scheduler = Scheduler;
	type PalletsOrigin = OriginCaller;
	type MaxVotes = ConstU32<100>;
	//TODO: might need to weight for Karura
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = ConstU32<100>;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
	type SubmitOrigin = EnsureSigned<AccountId>;
}

impl orml_auction::Config for Runtime {
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManager;
	type WeightInfo = weights::orml_auction::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl orml_authority::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type Scheduler = Scheduler;
	type AsOriginId = AuthoritysOriginId;
	type AuthorityConfig = AuthorityConfigImpl;
	type WeightInfo = weights::orml_authority::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub const MinimumCount: u32 = 5;
	pub const ExpiresIn: Moment = 1000 * 60 * 60; // 1 hours
	pub RootOperatorAccountId: AccountId = AccountId::from([0xffu8; 32]);
	pub const MaxFeedValues: u32 = 10; // max 10 values allowed to feed in one call.
}

type AcalaDataProvider = orml_oracle::Instance1;
impl orml_oracle::Config<AcalaDataProvider> for Runtime {
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Runtime, MinimumCount, ExpiresIn, AcalaDataProvider>;
	type Time = Timestamp;
	type OracleKey = CurrencyId;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
	type Members = OperatorMembershipAcala;
	type MaxHasDispatchedSize = ConstU32<20>;
	type WeightInfo = weights::orml_oracle::WeightInfo<Runtime>;
	type MaxFeedValues = MaxFeedValues;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkInstanceHelper<Runtime, AcalaDataProvider>;
}

create_median_value_data_provider!(
	AggregatedDataProvider,
	CurrencyId,
	Price,
	TimeStampedPrice,
	[AcalaOracle]
);
// Aggregated data provider cannot feed.
impl DataFeeder<CurrencyId, Price, AccountId> for AggregatedDataProvider {
	fn feed_value(_: Option<AccountId>, _: CurrencyId, _: Price) -> DispatchResult {
		Err("Not supported".into())
	}
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			CurrencyId::Token(symbol) => match symbol {
				TokenSymbol::KUSD => cent(*currency_id),
				TokenSymbol::KSM => 10 * millicent(*currency_id),
				TokenSymbol::LKSM => 50 * millicent(*currency_id),
				TokenSymbol::BNC => 800 * millicent(*currency_id), // 80BNC = 1KSM
				TokenSymbol::VSKSM => 10 * millicent(*currency_id), // 1VSKSM = 1KSM
				TokenSymbol::PHA => 4000 * millicent(*currency_id), // 400PHA = 1KSM
				TokenSymbol::KINT => 13333 * microcent(*currency_id), // 1.33 KINT = 1 KSM
				TokenSymbol::KBTC => 66 * microcent(*currency_id), // 1KBTC = 150 KSM
				TokenSymbol::TAI => dollar(*currency_id), // 1 KUSD = 100 TAI

				TokenSymbol::ACA |
				TokenSymbol::AUSD |
				TokenSymbol::DOT |
				TokenSymbol::LDOT |
				TokenSymbol::KAR |
				TokenSymbol::TAP => Balance::MAX // unsupported
			},
			CurrencyId::DexShare(dex_share_0, _) => {
				let currency_id_0: CurrencyId = (*dex_share_0).into();

				// initial dex share amount is calculated based on currency_id_0,
				// use the ED of currency_id_0 as the ED of lp token.
				if currency_id_0 == GetNativeCurrencyId::get() {
					NativeTokenExistentialDeposit::get()
				} else if let CurrencyId::Erc20(address) = currency_id_0 {
					// LP token with erc20
					AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::Erc20(address)).
						map_or(Balance::MAX, |metadata| metadata.minimal_balance)
				} else {
					Self::get(&currency_id_0)
				}
			},
			CurrencyId::Erc20(address) => AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::Erc20(*address)).map_or(Balance::MAX, |metadata| metadata.minimal_balance),
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::StableAssetId(*stable_asset_id)).
					map_or(Balance::MAX, |metadata| metadata.minimal_balance)
			},
			CurrencyId::LiquidCrowdloan(_) => ExistentialDeposits::get(&CurrencyId::Token(TokenSymbol::KSM)), // the same as KSM
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::ForeignAssetId(*foreign_asset_id)).
					map_or(Balance::MAX, |metadata| metadata.minimal_balance)
			},
		}
	};
}

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a)
	}
}

parameter_types! {
	pub KaruraTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl orml_tokens::Config for Runtime {
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = weights::orml_tokens::WeightInfo<Runtime>;
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime, KaruraTreasuryAccount>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = DustRemovalWhitelist;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_type_with_key! {
	pub LiquidCrowdloanLeaseBlockNumber: |_lease: Lease| -> Option<BlockNumber> {
		None
	};
}

parameter_type_with_key! {
	pub PricingPegged: |currency_id: CurrencyId| -> Option<CurrencyId> {
		match currency_id {
			// taiKSM
			CurrencyId::StableAssetPoolToken(0) => Some(KSM),
			_ => None,
		}
	};
}

parameter_types! {
	pub StableCurrencyFixedPrice: Price = Price::saturating_from_rational(1, 1);
	pub RewardRatePerRelaychainBlock: Rate = Rate::saturating_from_rational(3_068, 100_000_000_000u128);	// 17.5% annual staking reward rate of Kusama
}

impl module_prices::Config for Runtime {
	type Source = AggregatedDataProvider;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type LiquidStakingExchangeRateProvider = Homa;
	type DEX = Dex;
	type Currency = Currencies;
	type Erc20InfoMapping = EvmErc20InfoMapping<Runtime>;
	type LiquidCrowdloanLeaseBlockNumber = LiquidCrowdloanLeaseBlockNumber;
	type RelayChainBlockNumber = RelaychainDataProvider<Runtime>;
	type RewardRatePerRelaychainBlock = RewardRatePerRelaychainBlock;
	type PricingPegged = PricingPegged;
	type WeightInfo = weights::module_prices::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = KAR;
	pub const GetStableCurrencyId: CurrencyId = KUSD;
	pub const GetLiquidCurrencyId: CurrencyId = LKSM;
	pub const GetStakingCurrencyId: CurrencyId = KSM;
	pub Erc20HoldingAccount: H160 = primitives::evm::ERC20_HOLDING_ACCOUNT;
}

impl module_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = weights::module_currencies::WeightInfo<Runtime>;
	type AddressMapping = EvmAddressMapping<Runtime>;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type GasToWeight = GasToWeight;
	type SweepOrigin = EnsureRootOrOneGeneralCouncil;
	type OnDust = module_currencies::TransferDust<Runtime, KaruraTreasuryAccount>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub KaruraFoundationAccounts: Vec<AccountId> = vec![
		hex_literal::hex!["efd29d0d6e63911ae3727fc71506bc3365c5d3b39e3a1680c857b4457cf8afad"].into(),	// tij5W2NzmtxxAbwudwiZpif9ScmZfgFYdzrJWKYq6oNbSNH
		hex_literal::hex!["41dd2515ea11692c02306b68a2c6ff69b6606ebddaac40682789cfab300971c4"].into(),	// pndshZqDAC9GutDvv7LzhGhgWeGv5YX9puFA8xDidHXCyjd
		hex_literal::hex!["dad0a28c620ba73b51234b1b2ae35064d90ee847e2c37f9268294646c5af65eb"].into(),	// tFBV65Ts7wpQPxGM6PET9euNzp4pXdi9DVtgLZDJoFveR9F
		TreasuryPalletId::get().into_account_truncating(),
		TreasuryReservePalletId::get().into_account_truncating(),
	];
}

pub struct EnsureKaruraFoundation;
impl EnsureOrigin<RuntimeOrigin> for EnsureKaruraFoundation {
	type Success = AccountId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o).and_then(|o| match o {
			RawOrigin::Signed(caller) => {
				if KaruraFoundationAccounts::get().contains(&caller) {
					Ok(caller)
				} else {
					Err(RuntimeOrigin::from(Some(caller)))
				}
			}
			r => Err(RuntimeOrigin::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn try_successful_origin() -> Result<RuntimeOrigin, ()> {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		Ok(RuntimeOrigin::from(RawOrigin::Signed(zero_account_id)))
	}
}

impl orml_vesting::Config for Runtime {
	type Currency = pallet_balances::Pallet<Runtime>;
	type MinVestedTransfer = ConstU128<0>;
	type VestedTransferOrigin = EnsureKaruraFoundation;
	type WeightInfo = weights::orml_vesting::WeightInfo<Runtime>;
	type MaxVestingSchedules = ConstU32<100>;
	type BlockNumberProvider = RelaychainDataProvider<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(80) * RuntimeBlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = ConstU32<10>;
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type Preimages = Preimage;
	type BlockNumberProvider = System;
}

parameter_types! {
	pub PreimageBaseDeposit: Balance = deposit(2, 64);
	pub PreimageByteDeposit: Balance = deposit(0, 1);
	pub const PreimageHoldReason: RuntimeHoldReason = RuntimeHoldReason::Preimage(pallet_preimage::HoldReason::Preimage);
}

impl pallet_preimage::Config for Runtime {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ManagerOrigin = EnsureRoot<AccountId>;
	type Consideration = HoldConsideration<
		AccountId,
		Balances,
		PreimageHoldReason,
		LinearStoragePrice<PreimageBaseDeposit, PreimageByteDeposit, Balance>,
	>;
}

parameter_types! {
	pub MinimumIncrementSize: Rate = Rate::saturating_from_rational(2, 100);
	pub const AuctionTimeToClose: BlockNumber = 15 * MINUTES;
	pub const AuctionDurationSoftCap: BlockNumber = 2 * HOURS;
}

impl module_auction_manager::Config for Runtime {
	type Currency = Currencies;
	type Auction = Auction;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CdpTreasury;
	type PriceSource = module_prices::PriorityLockedPriceProvider<Runtime>;
	type UnsignedPriority = runtime_common::AuctionManagerUnsignedPriority;
	type EmergencyShutdown = EmergencyShutdown;
	type WeightInfo = weights::module_auction_manager::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl module_loans::Config for Runtime {
	type Currency = Currencies;
	type RiskManager = CdpEngine;
	type CDPTreasury = CdpTreasury;
	type PalletId = LoansPalletId;
	type OnUpdateLoan = module_incentives::OnUpdateLoan<Runtime>;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_signed_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		public: <Signature as sp_runtime::traits::Verify>::Signer,
		account: AccountId,
		nonce: Nonce,
	) -> Option<UncheckedExtrinsic> {
		// take the biggest period possible.
		let period = BlockHashCount::get()
			.checked_next_power_of_two()
			.map(|c| c / 2)
			.unwrap_or(2) as u64;
		let current_block = System::block_number()
			.saturated_into::<u64>()
			// The `System::block_number` is initialized with `n+1`,
			// so the actual block number is `n`.
			.saturating_sub(1);
		let tip = 0;
		let tx_ext: TxExtension = (
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			runtime_common::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			frame_metadata_hash_extension::CheckMetadataHash::<Runtime>::new(true),
			module_evm::SetEvmOrigin::<Runtime>::new(),
			module_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, tx_ext)
			.map_err(|e| {
				log::warn!("Unable to create signed payload: {e:?}");
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let address = AccountIdLookup::unlookup(account);
		let (call, tx_ext, _) = raw_payload.deconstruct();
		let transaction = UncheckedExtrinsic::new_signed(call, address, signature, tx_ext);
		Some(transaction)
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::CreateTransactionBase<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

impl<LocalCall> frame_system::offchain::CreateTransaction<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	type Extension = TxExtension;

	fn create_transaction(call: RuntimeCall, extension: TxExtension) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_transaction(call, extension)
	}
}

impl<LocalCall> frame_system::offchain::CreateBare<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: RuntimeCall) -> UncheckedExtrinsic {
		UncheckedExtrinsic::new_bare(call)
	}
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(150, 100);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: FractionalRate = FractionalRate::try_from(Rate::saturating_from_rational(8, 100))
		.expect("Rate is in range; qed");
	pub MinimumDebitValue: Balance = 50 * dollar(KUSD);
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::saturating_from_rational(10, 100);
	pub MaxLiquidationContractSlippage: Ratio = Ratio::saturating_from_rational(15, 100);
	pub SettleErc20EvmOrigin: AccountId = AccountId::from(hex_literal::hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")); // `u5wKvsdTcsYQXeB9nvDQ7PppNHeVefghTzBY9niAhMPXpyo`
}

impl module_cdp_engine::Config for Runtime {
	type PriceSource = module_prices::PriorityLockedPriceProvider<Runtime>;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = MinimumDebitValue;
	type MinimumCollateralAmount =
		ExistentialDepositsTimesOneHundred<GetNativeCurrencyId, NativeTokenExistentialDeposit, ExistentialDeposits>;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CdpTreasury;
	type UpdateOrigin = EnsureRootOrHalfFinancialCouncil;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type UnsignedPriority = runtime_common::CdpEngineUnsignedPriority;
	type EmergencyShutdown = EmergencyShutdown;
	type UnixTime = Timestamp;
	type Currency = Currencies;
	type DEX = Dex;
	type LiquidationContractsUpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type MaxLiquidationContractSlippage = MaxLiquidationContractSlippage;
	type MaxLiquidationContracts = ConstU32<10>;
	type LiquidationEvmBridge = module_evm_bridge::LiquidationEvmBridge<Runtime>;
	type PalletId = CDPEnginePalletId;
	type EvmAddressMapping = module_evm_accounts::EvmAddressMapping<Runtime>;
	type Swap = AcalaSwap;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type SettleErc20EvmOrigin = SettleErc20EvmOrigin;
	type WeightInfo = weights::module_cdp_engine::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub DepositPerAuthorization: Balance = deposit(1, 64);
}

impl module_honzon::Config for Runtime {
	type Currency = Balances;
	type DepositPerAuthorization = DepositPerAuthorization;
	type CollateralCurrencyIds = CollateralCurrencyIds<Runtime>;
	type WeightInfo = weights::module_honzon::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl module_emergency_shutdown::Config for Runtime {
	type CollateralCurrencyIds = CollateralCurrencyIds<Runtime>;
	type PriceSource = Prices;
	type CDPTreasury = CdpTreasury;
	type AuctionManagerHandler = AuctionManager;
	type ShutdownOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::module_emergency_shutdown::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (3, 1000);	// 0.3%
	pub const ExtendedProvisioningBlocks: BlockNumber = 2 * DAYS;
	pub const TradingPathLimit: u32 = 4;
}

impl module_dex::Config for Runtime {
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20InfoMapping = EvmErc20InfoMapping<Runtime>;
	type DEXIncentives = Incentives;
	type WeightInfo = weights::module_dex::WeightInfo<Runtime>;
	type ListingOrigin = EnsureRootOrHalfGeneralCouncil;
	type ExtendedProvisioningBlocks = ExtendedProvisioningBlocks;
	type OnLiquidityPoolUpdated = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl module_aggregated_dex::Config for Runtime {
	type DEX = Dex;
	type StableAsset = RebasedStableAsset;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type DexSwapJointList = AlternativeSwapPathJointList;
	type SwapPathLimit = ConstU32<3>;
	type WeightInfo = weights::module_aggregated_dex::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

pub type RebasedStableAsset = module_support::RebasedStableAsset<
	StableAsset,
	ConvertBalanceHoma,
	module_aggregated_dex::RebasedStableAssetErrorConvertor<Runtime>,
>;

pub type AcalaSwap = module_aggregated_dex::AggregatedSwap<Runtime>;

impl module_dex_oracle::Config for Runtime {
	type DEX = Dex;
	type Time = Timestamp;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_dex_oracle::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub HonzonTreasuryAccount: AccountId = HonzonTreasuryPalletId::get().into_account_truncating();
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![KSM],
		vec![LKSM],
		vec![KUSD],
	];
}

impl module_cdp_treasury::Config for Runtime {
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManager;
	type UpdateOrigin = EnsureRootOrHalfFinancialCouncil;
	type DEX = Dex;
	type Swap = AcalaSwap;
	type MaxAuctionsCount = ConstU32<50>;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = HonzonTreasuryAccount;
	type WeightInfo = weights::module_cdp_treasury::WeightInfo<Runtime>;
	type StableAsset = RebasedStableAsset;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl module_transaction_pause::Config for Runtime {
	type UpdateOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type WeightInfo = weights::module_transaction_pause::WeightInfo<Runtime>;
}

parameter_types! {
	pub DefaultFeeTokens: Vec<CurrencyId> = vec![KUSD, KSM, LKSM];
	pub const CustomFeeSurplus: Percent = Percent::from_percent(50);
	pub const AlternativeFeeSurplus: Percent = Percent::from_percent(25);
}

type NegativeImbalance = <Balances as PalletCurrency<AccountId>>::NegativeImbalance;
pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds(mut fees_then_tips: impl Iterator<Item = NegativeImbalance>) {
		if let Some(mut fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut fees);
			}
			// for fees and tips, 100% to treasury reserve
			<Balances as Currency<AccountId>>::resolve_creating(
				&TreasuryReservePalletId::get().into_account_truncating(),
				fees,
			);
		}
	}
}

impl module_transaction_payment::Config for Runtime {
	type RuntimeCall = RuntimeCall;
	type NativeCurrencyId = GetNativeCurrencyId;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = DealWithFees;
	type AlternativeFeeSwapDeposit = NativeTokenExistentialDeposit;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type TipPerWeightStep = TipPerWeightStep;
	type MaxTipsOfPriority = MaxTipsOfPriority;
	type WeightToFee = WeightToFee;
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type Swap = AcalaSwap;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = module_prices::RealTimePriceProvider<Runtime>;
	type WeightInfo = weights::module_transaction_payment::WeightInfo<Runtime>;
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = KaruraTreasuryAccount;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type CustomFeeSurplus = CustomFeeSurplus;
	type AlternativeFeeSurplus = AlternativeFeeSurplus;
	type DefaultFeeTokens = DefaultFeeTokens;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

impl module_evm_accounts::Config for Runtime {
	type Currency = Balances;
	type AddressMapping = EvmAddressMapping<Runtime>;
	type TransferAll = Currencies;
	type ChainId = EvmChainId<Runtime>;
	type WeightInfo = weights::module_evm_accounts::WeightInfo<Runtime>;
}

impl module_asset_registry::Config for Runtime {
	type Currency = Balances;
	type StakingCurrencyId = GetStakingCurrencyId;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type RegisterOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_asset_registry::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_type_with_key! {
	pub MinimalShares: |pool_id: PoolId| -> Balance {
		match pool_id {
			PoolId::Loans(currency_id) | PoolId::Dex(currency_id) | PoolId::Earning(currency_id) => {
				if *currency_id == GetNativeCurrencyId::get() {
					NativeTokenExistentialDeposit::get()
				} else {
					ExistentialDeposits::get(currency_id)
				}
			}
			PoolId::NomineesElection => {
				ExistentialDeposits::get(&GetLiquidCurrencyId::get())
			}
		}
	};
}

impl orml_rewards::Config for Runtime {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId;
	type CurrencyId = CurrencyId;
	type MinimalShares = MinimalShares;
	type Handler = Incentives;
}

parameter_types! {
	pub const AccumulatePeriod: BlockNumber = MINUTES;
}

impl module_incentives::Config for Runtime {
	type RewardsSource = UnreleasedNativeVaultAccountId;
	type NativeCurrencyId = GetNativeCurrencyId;
	type AccumulatePeriod = AccumulatePeriod;
	type UpdateOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type Currency = Currencies;
	type EmergencyShutdown = EmergencyShutdown;
	type PalletId = IncentivesPalletId;
	type WeightInfo = weights::module_incentives::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub CreateClassDeposit: Balance = 50 * dollar(KAR);
	pub CreateTokenDeposit: Balance = 20 * cent(KAR);
}

impl module_nft::Config for Runtime {
	type Currency = Balances;
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type PalletId = NftPalletId;
	type MaxAttributesBytes = ConstU32<2048>;
	type WeightInfo = weights::module_nft::WeightInfo<Runtime>;
}

impl orml_nft::Config for Runtime {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = module_nft::ClassData<Balance>;
	type TokenData = module_nft::TokenData<Balance>;
	type MaxClassMetadata = ConstU32<1024>;
	type MaxTokenMetadata = ConstU32<1024>;
}

impl module_xnft::Config for Runtime {
	type PalletId = XnftPalletId;
	type LocationToAccountId = xcm_config::LocationToAccountId;
	type SelfParaId = ParachainInfo;
	type NtfPalletLocation = xcm_config::NftPalletLocation;
	type RegisterOrigin = EnsureRootOrOneTechnicalCommittee;
}

impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			// Always allowed RuntimeCall::Utility no matter type.
			// Only transactions allowed by Proxy.filter can be executed,
			// otherwise `BadOrigin` will be returned in RuntimeCall::Utility.
			_ if matches!(c, RuntimeCall::Utility(..)) => true,
			ProxyType::Any => true,
			ProxyType::CancelProxy => matches!(c, RuntimeCall::Proxy(pallet_proxy::Call::reject_announcement { .. })),
			ProxyType::Governance => {
				matches!(
					c,
					RuntimeCall::Authority(..)
						| RuntimeCall::Democracy(..)
						| RuntimeCall::GeneralCouncil(..)
						| RuntimeCall::FinancialCouncil(..)
						| RuntimeCall::HomaCouncil(..)
						| RuntimeCall::TechnicalCommittee(..)
						| RuntimeCall::Treasury(..)
						| RuntimeCall::Bounties(..)
						| RuntimeCall::Tips(..)
				)
			}
			ProxyType::Auction => {
				matches!(c, RuntimeCall::Auction(orml_auction::Call::bid { .. }))
			}
			ProxyType::Swap => {
				matches!(
					c,
					RuntimeCall::Dex(module_dex::Call::swap_with_exact_supply { .. })
						| RuntimeCall::Dex(module_dex::Call::swap_with_exact_target { .. })
						| RuntimeCall::AggregatedDex(module_aggregated_dex::Call::swap_with_exact_supply { .. })
						| RuntimeCall::AggregatedDex(module_aggregated_dex::Call::swap_with_exact_target { .. })
				)
			}
			ProxyType::Loan => {
				matches!(
					c,
					RuntimeCall::Honzon(module_honzon::Call::adjust_loan { .. })
						| RuntimeCall::Honzon(module_honzon::Call::close_loan_has_debit_by_dex { .. })
						| RuntimeCall::Honzon(module_honzon::Call::adjust_loan_by_debit_value { .. })
						| RuntimeCall::Honzon(module_honzon::Call::transfer_debit { .. })
				)
			}
			ProxyType::DexLiquidity => {
				matches!(
					c,
					RuntimeCall::Dex(module_dex::Call::add_liquidity { .. })
						| RuntimeCall::Dex(module_dex::Call::remove_liquidity { .. })
				)
			}
			ProxyType::StableAssetSwap => {
				matches!(c, RuntimeCall::StableAsset(nutsfinance_stable_asset::Call::swap { .. }))
			}
			ProxyType::StableAssetLiquidity => {
				matches!(
					c,
					RuntimeCall::StableAsset(nutsfinance_stable_asset::Call::mint { .. })
						| RuntimeCall::StableAsset(nutsfinance_stable_asset::Call::redeem_proportion { .. })
						| RuntimeCall::StableAsset(nutsfinance_stable_asset::Call::redeem_single { .. })
						| RuntimeCall::StableAsset(nutsfinance_stable_asset::Call::redeem_multi { .. })
				)
			}
			ProxyType::Homa => {
				matches!(
					c,
					RuntimeCall::Homa(module_homa::Call::mint { .. })
						| RuntimeCall::Homa(module_homa::Call::request_redeem { .. })
				)
			}
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		match (self, o) {
			(x, y) if x == y => true,
			(ProxyType::Any, _) => true,
			(_, ProxyType::Any) => false,
			_ => false,
		}
	}
}

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub ProxyDepositFactor: Balance = deposit(0, 33);
	pub AnnouncementDepositBase: Balance = deposit(1, 8);
	pub AnnouncementDepositFactor: Balance = deposit(0, 66);
}

impl pallet_proxy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = ConstU32<32>;
	type WeightInfo = ();
	type MaxPending = ConstU32<32>;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
	type BlockNumberProvider = System;
}

parameter_types! {
	pub const NewContractExtraBytes: u32 = 10_000;
	pub NetworkContractSource: H160 = H160::from_low_u64_be(0);
	pub DeveloperDeposit: Balance = 50 * dollar(KAR);
	pub PublicationFee: Balance = 10 * dollar(KAR);
	pub PrecompilesValue: AllPrecompiles<Runtime, module_transaction_pause::PausedPrecompileFilter<Runtime>, ()> = AllPrecompiles::<_, _, _>::karura();
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct StorageDepositPerByte;
impl<I: From<Balance>> frame_support::traits::Get<I> for StorageDepositPerByte {
	fn get() -> I {
		// NOTE: KAR decimals is 12, convert to 18.
		// 10 * millicent(KAR) * 10^6
		I::from(100_000_000_000_000)
	}
}

// TODO: remove
#[derive(Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct TxFeePerGas;
impl<I: From<Balance>> frame_support::traits::Get<I> for TxFeePerGas {
	fn get() -> I {
		// NOTE: 200 GWei
		// ensure suffix is 0x0000
		I::from(200u128.saturating_mul(10u128.saturating_pow(9)) & !0xffff)
	}
}

#[derive(Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct TxFeePerGasV2;
impl<I: From<Balance>> frame_support::traits::Get<I> for TxFeePerGasV2 {
	fn get() -> I {
		// NOTE: 100 GWei
		I::from(100_000_000_000u128)
	}
}

impl module_evm::Config for Runtime {
	type AddressMapping = EvmAddressMapping<Runtime>;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = TxFeePerGas;
	type PrecompilesType = AllPrecompiles<Self, module_transaction_pause::PausedPrecompileFilter<Self>, ()>;
	type PrecompilesValue = PrecompilesValue;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;
	type NetworkContractOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type PublicationFee = PublicationFee;
	type TreasuryAccount = KaruraTreasuryAccount;
	type FreePublicationOrigin = EnsureRootOrHalfGeneralCouncil;
	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type Randomness = RandomnessSource<Runtime>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = weights::module_evm::WeightInfo<Runtime>;
}

impl module_evm_bridge::Config for Runtime {
	type EVM = EVM;
}

impl module_session_manager::Config for Runtime {
	type ValidatorSet = Session;
	type WeightInfo = weights::module_session_manager::WeightInfo<Runtime>;
}

parameter_types! {
	pub ReservedXcmpWeight: Weight = RuntimeBlockWeights::get().max_block.saturating_div(4);
	pub ReservedDmpWeight: Weight = RuntimeBlockWeights::get().max_block.saturating_div(4);
}

impl cumulus_pallet_parachain_system::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnSystemEvent = ();
	type SelfParaId = ParachainInfo;
	type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, xcm_config::RelayOrigin>;
	type ReservedDmpWeight = ReservedDmpWeight;
	type OutboundXcmpMessageSource = XcmpQueue;
	type XcmpMessageHandler = XcmpQueue;
	type ReservedXcmpWeight = ReservedXcmpWeight;
	type CheckAssociatedRelayNumber =
		CheckRelayNumber<EvmChainId<Runtime>, cumulus_pallet_parachain_system::RelayNumberStrictlyIncreases>;
	type WeightInfo = cumulus_pallet_parachain_system::weights::SubstrateWeight<Runtime>;
	type ConsensusHook = ConsensusHook<Runtime>;
	type SelectCore = cumulus_pallet_parachain_system::DefaultCoreSelector<Runtime>;
	type RelayParentOffset = ConstU32<0>;
}

impl parachain_info::Config for Runtime {}

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub HomaTreasuryAccount: AccountId = HomaTreasuryPalletId::get().into_account_truncating();
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![
		0,  // EQFY1VnjdYUSJ9oWoNgunPHeqLFooyPF3r6ZkTbCU5a6Eez
		1,  // CdRsH6LC4yVvSH4XC2qmtPhuAoQEqedBdmnfaYPSVh5TDZ7
		2,  // HZyWwhbi8yWGK6HJGjgYWQtszPvggLUrQDX9PR6StbmPwHP
	];
	pub MintThreshold: Balance = 10 * cent(KSM);
	pub RedeemThreshold: Balance = 50 * cent(LKSM);
	pub const BondingDuration: EraIndex = 28;
}

impl module_homa::Config for Runtime {
	type Currency = Currencies;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type StakingCurrencyId = GetStakingCurrencyId;
	type LiquidCurrencyId = GetLiquidCurrencyId;
	type PalletId = HomaPalletId;
	type TreasuryAccount = HomaTreasuryAccount;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ActiveSubAccountsIndexList = ActiveSubAccountsIndexList;
	type BondingDuration = BondingDuration;
	type MintThreshold = MintThreshold;
	type RedeemThreshold = RedeemThreshold;
	type RelayChainBlockNumber = RelaychainDataProvider<Runtime>;
	type XcmInterface = XcmInterface;
	type WeightInfo = weights::module_homa::WeightInfo<Runtime>;
	type NominationsProvider = NomineesElection;
	type ProcessRedeemRequestsLimit = ConstU32<1_000>;
}

parameter_types! {
	pub MinBondAmount: Balance = 100 * dollar(LKSM);
	pub ValidatorInsuranceThreshold: Balance = 1_000 * dollar(LKSM);
}

impl module_homa_validator_list::Config for Runtime {
	type ValidatorId = AccountId;
	type LiquidTokenCurrency = module_currencies::Currency<Runtime, GetLiquidCurrencyId>;
	type MinBondAmount = MinBondAmount;
	type BondingDuration = BondingDuration;
	type ValidatorInsuranceThreshold = ValidatorInsuranceThreshold;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type LiquidStakingExchangeRateProvider = Homa;
	type CurrentEra = Homa;
	type WeightInfo = weights::module_homa_validator_list::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub MinNomineesElectionBondThreshold: Balance = dollar(LKSM);
	pub const MaxNominateesCount: u32 = 24;
}

impl module_nominees_election::Config for Runtime {
	type Currency = module_currencies::Currency<Runtime, GetLiquidCurrencyId>;
	type NomineeId = AccountId;
	type PalletId = NomineesElectionId;
	type MinBond = MinNomineesElectionBondThreshold;
	type BondingDuration = BondingDuration;
	type MaxNominateesCount = MaxNominateesCount;
	type MaxUnbondingChunks = ConstU32<7>;
	type NomineeFilter = HomaValidatorList;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type OnBonded = module_incentives::OnNomineesElectionBonded<Runtime>;
	type OnUnbonded = module_incentives::OnNomineesElectionUnbonded<Runtime>;
	type CurrentEra = Homa;
	type WeightInfo = weights::module_nominees_election::WeightInfo<Runtime>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

pub struct SubAccountIndexAccountIdConvertor;
impl Convert<u16, AccountId> for SubAccountIndexAccountIdConvertor {
	fn convert(sub_account_index: u16) -> AccountId {
		Utility::derivative_account_id(
			Sibling::from(ParachainInfo::get()).into_account_truncating(),
			sub_account_index,
		)
	}
}

parameter_types! {
	pub ParachainAccount: AccountId = Sibling::from(ParachainInfo::get()).into_account_truncating();
}

impl module_xcm_interface::Config for Runtime {
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type ParachainAccount = ParachainAccount;
	type AssetHubUnbondingSlashingSpans = ConstU32<5>;
	type SovereignSubAccountIdConvert = SubAccountIndexAccountIdConvertor;
	type AssetHubCallBuilder = AssetHubCallBuilder<ParachainInfo, module_assethub::KusamaAssetHubCall>;
	type AssetHubLocation = xcm_config::AssetHubLocation;
	type AccountIdToLocation = runtime_common::xcm_config::AccountIdToLocation;
}

impl orml_unknown_tokens::Config for Runtime {}

impl orml_xcm::Config for Runtime {
	type SovereignOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
}

define_combined_task! {
	#[derive(Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<Runtime>),
	}
}

parameter_types!(
	// At least 2% of max block weight should remain before idle tasks are dispatched.
	pub MinimumWeightRemainInBlock: Weight = RuntimeBlockWeights::get().max_block / 50;
);

impl module_idle_scheduler::Config for Runtime {
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = RelaychainDataProvider<Runtime>;
	// Number of relay chain blocks produced with no parachain blocks finalized,
	// once this number is reached idle scheduler is disabled as block production is slow
	type DisableBlockThreshold = ConstU32<6>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub const StableCoinCurrencyId: CurrencyId = KUSD;
	pub HonzonBridgeAccount: AccountId = HonzonBridgePalletId::get().into_account_truncating();
}

impl module_honzon_bridge::Config for Runtime {
	type Currency = Currencies;
	type StableCoinCurrencyId = StableCoinCurrencyId;
	type HonzonBridgeAccount = HonzonBridgeAccount;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_honzon_bridge::WeightInfo<Runtime>;
}

pub struct EnsurePoolAssetId;
impl nutsfinance_stable_asset::traits::ValidateAssetId<CurrencyId> for EnsurePoolAssetId {
	fn validate(currency_id: CurrencyId) -> bool {
		matches!(currency_id, CurrencyId::StableAssetPoolToken(_))
	}
}

pub struct ConvertBalanceHoma;
impl orml_tokens::ConvertBalance<Balance, Balance> for ConvertBalanceHoma {
	type AssetId = CurrencyId;

	fn convert_balance(balance: Balance, asset_id: CurrencyId) -> Result<Balance, ArithmeticError> {
		Ok(match asset_id {
			CurrencyId::Token(TokenSymbol::LKSM) => Homa::get_exchange_rate()
				.checked_mul_int(balance)
				.ok_or(ArithmeticError::Overflow)?,
			_ => balance,
		})
	}

	fn convert_balance_back(balance: Balance, asset_id: CurrencyId) -> Result<Balance, ArithmeticError> {
		Ok(match asset_id {
			CurrencyId::Token(TokenSymbol::LKSM) => Homa::get_exchange_rate()
				.reciprocal()
				.and_then(|x| x.checked_mul_int(balance))
				.ok_or(ArithmeticError::Overflow)?,
			_ => balance,
		})
	}
}

pub struct IsLiquidToken;
impl Contains<CurrencyId> for IsLiquidToken {
	fn contains(currency_id: &CurrencyId) -> bool {
		matches!(currency_id, CurrencyId::Token(TokenSymbol::LKSM))
	}
}

type RebaseTokens = orml_tokens::Combiner<
	AccountId,
	IsLiquidToken,
	orml_tokens::Mapper<AccountId, Currencies, ConvertBalanceHoma, Balance, GetLiquidCurrencyId>,
	Currencies,
>;

impl nutsfinance_stable_asset::Config for Runtime {
	type AssetId = CurrencyId;
	type Balance = Balance;
	type Assets = RebaseTokens;
	type PalletId = StableAssetPalletId;

	type AtLeast64BitUnsigned = u128;
	type FeePrecision = ConstU128<10_000_000_000>; // 10 decimals
	type APrecision = ConstU128<100>; // 2 decimals
	type PoolAssetLimit = ConstU32<5>;
	type SwapExactOverAmount = ConstU128<100>;
	type WeightInfo = weights::nutsfinance_stable_asset::WeightInfo<Runtime>;
	type ListingOrigin = EnsureRootOrHalfGeneralCouncil;
	type EnsurePoolAssetId = EnsurePoolAssetId;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

parameter_types! {
	pub MinBond: Balance = 10 * dollar(KAR);
	pub const UnbondingPeriod: BlockNumber = 8 * DAYS;
	pub const EarningLockIdentifier: LockIdentifier = *b"aca/earn";
}

impl module_earning::Config for Runtime {
	type Currency = Balances;
	type ParameterStore = ParameterStoreAdapter<Parameters, module_earning::Parameters>;
	type OnBonded = module_incentives::OnEarningBonded<Runtime>;
	type OnUnbonded = module_incentives::OnEarningUnbonded<Runtime>;
	type OnUnstakeFee = Treasury; // fee goes to treasury
	type MinBond = MinBond;
	type UnbondingPeriod = UnbondingPeriod;
	type MaxUnbondingChunks = ConstU32<10>;
	type LockIdentifier = EarningLockIdentifier;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = benchmarks::common::BenchmarkHelper<Runtime>;
}

define_aggregrated_parameters! {
	pub RuntimeParameters = {
		Earning: module_earning::Parameters = 0,
	}
}

impl orml_parameters::Config for Runtime {
	type AggregratedKeyValue = RuntimeParameters;
	type AdminOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type WeightInfo = ();
}

construct_runtime!(
	pub enum Runtime {
		// Core & Utility
		System: frame_system = 0,
		Timestamp: pallet_timestamp = 1,
		Scheduler: pallet_scheduler = 2,
		Utility: pallet_utility = 3,
		Multisig: pallet_multisig = 4,
		Proxy: pallet_proxy = 5,
		TransactionPause: module_transaction_pause = 6,
		// NOTE: IdleScheduler must be put before ParachainSystem in order to read relaychain blocknumber
		IdleScheduler: module_idle_scheduler = 7,
		Preimage: pallet_preimage = 8,

		// Tokens & Related
		Balances: pallet_balances = 10,
		Tokens: orml_tokens exclude_parts { Call } = 11,
		Currencies: module_currencies = 12,
		Vesting: orml_vesting = 13,
		TransactionPayment: module_transaction_payment = 14,

		// Treasury
		Treasury: pallet_treasury = 20,
		Bounties: pallet_bounties = 21,
		Tips: pallet_tips = 22,

		// Parachain
		ParachainInfo: parachain_info exclude_parts { Call } = 31,

		// Collator. The order of the 4 below are important and shall not change.
		Authorship: pallet_authorship = 40,
		CollatorSelection: module_collator_selection = 41,
		Session: pallet_session = 42,
		Aura: pallet_aura = 43,
		AuraExt: cumulus_pallet_aura_ext = 44,
		SessionManager: module_session_manager = 45,

		// XCM
		XcmpQueue: cumulus_pallet_xcmp_queue = 50,
		PolkadotXcm: pallet_xcm = 51,
		CumulusXcm: cumulus_pallet_xcm exclude_parts { Call } = 52,
		// 53 was used by DmpQueue which is now replaced by MessageQueue
		XTokens: orml_xtokens = 54,
		UnknownTokens: orml_unknown_tokens = 55,
		OrmlXcm: orml_xcm = 56,
		MessageQueue: pallet_message_queue = 57,

		// Governance
		Authority: orml_authority = 60,
		GeneralCouncil: pallet_collective::<Instance1> = 61,
		GeneralCouncilMembership: pallet_membership::<Instance1> = 62,
		FinancialCouncil: pallet_collective::<Instance2> = 63,
		FinancialCouncilMembership: pallet_membership::<Instance2> = 64,
		HomaCouncil: pallet_collective::<Instance3> = 65,
		HomaCouncilMembership: pallet_membership::<Instance3> = 66,
		TechnicalCommittee: pallet_collective::<Instance4> = 67,
		TechnicalCommitteeMembership: pallet_membership::<Instance4> = 68,
		Democracy: pallet_democracy = 69,

		// Oracle
		//
		// NOTE: OperatorMembership must be placed after Oracle or else will have race condition on initialization
		AcalaOracle: orml_oracle::<Instance1> = 70,
		OperatorMembershipAcala: pallet_membership::<Instance5> = 71,

		// ORML Core
		Auction: orml_auction = 80,
		Rewards: orml_rewards = 81,
		OrmlNFT: orml_nft exclude_parts { Call } = 82,
		Parameters: orml_parameters = 83,

		// Karura Core
		Prices: module_prices = 90,
		Dex: module_dex = 91,
		DexOracle: module_dex_oracle = 92,
		AggregatedDex: module_aggregated_dex = 93,
		Earning: module_earning = 94,

		// Honzon
		AuctionManager: module_auction_manager = 100,
		Loans: module_loans = 101,
		Honzon: module_honzon = 102,
		CdpTreasury: module_cdp_treasury = 103,
		CdpEngine: module_cdp_engine = 104,
		EmergencyShutdown: module_emergency_shutdown = 105,
		HonzonBridge: module_honzon_bridge = 106,

		// Homa
		Homa: module_homa = 116,
		XcmInterface: module_xcm_interface = 117,
		HomaValidatorList: module_homa_validator_list = 118,
		NomineesElection: module_nominees_election = 119,

		// Karura Other
		Incentives: module_incentives = 120,
		NFT: module_nft = 121,
		AssetRegistry: module_asset_registry = 122,
		XNFT: module_xnft = 123,

		// Smart contracts
		EVM: module_evm = 130,
		EVMBridge: module_evm_bridge exclude_parts { Call } = 131,
		EvmAccounts: module_evm_accounts = 132,

		// Stable asset
		StableAsset: nutsfinance_stable_asset = 200,

		// Parachain System, always put it at the end
		ParachainSystem: cumulus_pallet_parachain_system = 30,

		// Temporary
		Sudo: pallet_sudo = 255,
	}
);

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The extension to the basic transaction logic.
pub type TxExtension = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	runtime_common::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	frame_metadata_hash_extension::CheckMetadataHash<Runtime>,
	// `SetEvmOrigin` needs ahead of `ChargeTransactionPayment`, we set origin in `SetEvmOrigin::validate()`, then
	// `ChargeTransactionPayment::validate()` can process erc20 token transfer successfully in the case of using erc20
	// as fee token.
	module_evm::SetEvmOrigin<Runtime>,
	module_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	AcalaUncheckedExtrinsic<RuntimeCall, TxExtension, ConvertEthereumTx, StorageDepositPerByte, TxFeePerGas>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, TxExtension>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, TxExtension>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
	Runtime,
	Block,
	frame_system::ChainContext<Runtime>,
	Runtime,
	AllPalletsWithSystem,
	Migrations,
>;

#[allow(unused_parens)]
type Migrations = ();

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	use super::*;
	use alloc::boxed::Box;

	frame_benchmarking::define_benchmarks!(
		[module_aggregated_dex, AggregatedDex]
		[module_asset_registry, AssetRegistry]
		[module_auction_manager, AuctionManager]
		[module_cdp_engine, CdpEngine]
		[module_cdp_treasury, CdpTreasury]
		[module_collator_selection, CollatorSelection]
		[module_currencies, Currencies]
		[module_dex, Dex]
		[module_dex_oracle, DexOracle]
		[module_emergency_shutdown, EmergencyShutdown]
		[module_evm, EVM]
		[module_evm_accounts, EvmAccounts]
		[module_homa, Homa]
		[module_homa_validator_list, HomaValidatorList]
		[module_honzon, Honzon]
		[module_honzon_bridge, HonzonBridge]
		[module_idle_scheduler, IdleScheduler]
		[module_incentives, Incentives]
		[module_nominees_election, NomineesElection]
		[module_prices, Prices]
		[module_session_manager, SessionManager]
		[module_transaction_pause, TransactionPause]
		[module_transaction_payment, TransactionPayment]
		[orml_auction, Auction]
		[orml_authority, Authority]
		[orml_tokens, Tokens]
		[orml_vesting, Vesting]
		// Acala Ecosystem Modules
		[nutsfinance_stable_asset, StableAsset]
		// XCM
		[pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
	);

	use crate::xcm_config::AssetHubLocation;
	use cumulus_primitives_core::{Asset, Assets, Fungible, GeneralKey, Location, ParaId, Parachain, ParentThen};
	use frame_benchmarking::BenchmarkError;

	const KSM_UNITS: Balance = 100_000_000_000_000;
	const KSM_CENTS: Balance = KSM_UNITS / 100;

	parameter_types! {
		pub AssetHubParaId: ParaId = ParaId::from(parachains::asset_hub_kusama::ID);
		pub FeeAssetId: AssetId = AssetId(Location::parent());
		pub const ToSiblingBaseDeliveryFee: u128 = KSM_CENTS.saturating_mul(3);
	}

	pub type PriceForSiblingParachainDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
		FeeAssetId,
		ToSiblingBaseDeliveryFee,
		TransactionByteFee,
		XcmpQueue,
	>;

	parameter_types! {
		pub ExistentialDepositAsset: Option<Asset> = Some((
			Location::parent(),
			NativeTokenExistentialDeposit::get()
		).into());
		pub const RandomParaId: ParaId = ParaId::new(43211234);
	}

	impl pallet_xcm::benchmarking::Config for Runtime {
		type DeliveryHelper = (
			polkadot_runtime_common::xcm_sender::ToParachainDeliveryHelper<
				xcm_config::XcmConfig,
				ExistentialDepositAsset,
				PriceForSiblingParachainDelivery,
				RandomParaId,
				ParachainSystem,
			>,
			polkadot_runtime_common::xcm_sender::ToParachainDeliveryHelper<
				xcm_config::XcmConfig,
				ExistentialDepositAsset,
				PriceForSiblingParachainDelivery,
				AssetHubParaId,
				ParachainSystem,
			>,
		);

		fn reachable_dest() -> Option<Location> {
			Some(AssetHubLocation::get())
		}

		fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
			// XcmTeleportFilter is Nothing
			None
		}

		fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
			ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(RandomParaId::get());

			let encoded = KAR.encode();
			let mut data = [0u8; 32];
			let len = encoded.len().min(32);
			data[..len].copy_from_slice(&encoded[..len]);

			Some((
				Asset {
					id: AssetId(Location::new(
						0,
						GeneralKey {
							data: data,
							length: encoded.len() as u8,
						},
					)),
					fun: Fungible(NativeTokenExistentialDeposit::get() * 100),
				},
				ParentThen(Parachain(RandomParaId::get().into()).into()).into(),
			))
		}

		fn set_up_complex_asset_transfer() -> Option<(Assets, u32, Location, Box<dyn FnOnce()>)> {
			// transfer KUSD from this parachain to RandomParaId parachain
			ParachainSystem::open_outbound_hrmp_channel_for_benchmarks_or_tests(RandomParaId::get());

			let dest = Location::new(1, Parachain(RandomParaId::get().into()));

			// fee asset
			let fee_amount = NativeTokenExistentialDeposit::get();
			let kar_encoded = KAR.encode();
			let mut kar_data = [0u8; 32];
			let len = kar_encoded.len().min(32);
			kar_data[..len].copy_from_slice(&kar_encoded[..len]);
			let fee_location = Location::new(
				0,
				GeneralKey {
					data: kar_data,
					length: kar_encoded.len() as u8,
				},
			);
			let fee_asset: Asset = (fee_location, fee_amount).into();

			// asset to transfer
			let asset_amount = 1_000_000_000_000u128;
			let asset_balance = asset_amount * 10;
			let kusd_encoded = KUSD.encode();
			let mut kusd_data = [0u8; 32];
			let len = kusd_encoded.len().min(32);
			kusd_data[..len].copy_from_slice(&kusd_encoded[..len]);
			let asset_location = Location::new(
				0,
				GeneralKey {
					data: kusd_data,
					length: kusd_encoded.len() as u8,
				},
			);
			let transfer_asset: Asset = (asset_location, asset_amount).into();

			let assets: Assets = vec![fee_asset.clone(), transfer_asset].into();

			let who = frame_benchmarking::whitelisted_caller();
			// Give some multiple of the existential deposit
			let native_balance = NativeTokenExistentialDeposit::get() * 1000;
			let _ = <Balances as frame_support::traits::Currency<_>>::make_free_balance_be(&who, native_balance);
			let _ = Tokens::deposit(KUSD, &who, asset_balance);
			// verify initial balance
			assert_eq!(Balances::free_balance(&who), native_balance);
			assert_eq!(Tokens::free_balance(KUSD, &who), asset_balance);

			let fee_index = if assets.get(0).unwrap().eq(&fee_asset) { 0 } else { 1 };

			// verify transferred successfully
			let verify = Box::new(move || {
				// verify native balance after transfer, decreased by transferred fee amount
				// (plus transport fees)
				assert!(Balances::free_balance(&who) <= native_balance - fee_amount);
				// verify asset balance after transfer, decreased by transferred asset amount
				assert_eq!(Tokens::free_balance(KUSD, &who), asset_balance - asset_amount);
			});
			Some((assets, fee_index as u32, dest, verify))
		}

		fn get_asset() -> Asset {
			Asset {
				id: AssetId(Location::parent()),
				fun: Fungible(KSM_UNITS),
			}
		}
	}

	impl pallet_xcm_benchmarks::Config for Runtime {
		type XcmConfig = xcm_config::XcmConfig;
		type AccountIdConverter = xcm_config::LocationToAccountId;
		type DeliveryHelper = polkadot_runtime_common::xcm_sender::ToParachainDeliveryHelper<
			xcm_config::XcmConfig,
			ExistentialDepositAsset,
			PriceForSiblingParachainDelivery,
			AssetHubParaId,
			ParachainSystem,
		>;
		fn valid_destination() -> Result<Location, BenchmarkError> {
			Ok(AssetHubLocation::get())
		}
		fn worst_case_holding(_depositable_count: u32) -> Assets {
			let assets: Vec<Asset> = vec![Asset {
				id: AssetId(Location::parent()),
				fun: Fungible(1_000 * KSM_UNITS),
			}];
			assets.into()
		}
	}

	pub use frame_benchmarking::{BenchmarkBatch, BenchmarkList};
	pub use frame_support::traits::{StorageInfoTrait, WhitelistedStorageKeys};
	pub use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;
	pub use sp_storage::TrackedStorageKey;
}

#[cfg(feature = "runtime-benchmarks")]
use benches::*;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			OpaqueMetadata::new(Runtime::metadata().into())
		}

		fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
			Runtime::metadata_at_version(version)
		}

		fn metadata_versions() -> sp_std::vec::Vec<u32> {
			Runtime::metadata_versions()
		}
	}

	impl sp_block_builder::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: sp_inherents::InherentData,
		) -> sp_inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
			block_hash: <Block as BlockT>::Hash,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx, block_hash)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
		fn slot_duration() -> sp_consensus_aura::SlotDuration {
			sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
		}

		fn authorities() -> Vec<AuraId> {
			pallet_aura::Authorities::<Runtime>::get().into_inner()
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
		fn account_nonce(account: AccountId) -> Nonce {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
	> for Runtime {
		fn query_info(uxt: <Block as BlockT>::Extrinsic, len: u32) -> RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt.0, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt.0, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
		for Runtime
	{
		fn query_call_info(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_call_info(call, len)
		}
		fn query_call_fee_details(
			call: RuntimeCall,
			len: u32,
		) -> pallet_transaction_payment::FeeDetails<Balance> {
			TransactionPayment::query_call_fee_details(call, len)
		}
		fn query_weight_to_fee(weight: Weight) -> Balance {
			TransactionPayment::weight_to_fee(weight)
		}
		fn query_length_to_fee(length: u32) -> Balance {
			TransactionPayment::length_to_fee(length)
		}
	}

	impl orml_oracle_runtime_api::OracleApi<
		Block,
		DataProviderId,
		CurrencyId,
		TimeStampedPrice,
	> for Runtime {
		fn get_value(provider_id: DataProviderId, key: CurrencyId) -> Option<TimeStampedPrice> {
			match provider_id {
				DataProviderId::Acala => AcalaOracle::get_no_op(&key),
				DataProviderId::Aggregated => <AggregatedDataProvider as DataProviderExtended<_, _>>::get_no_op(&key)
			}
		}

		fn get_all_values(provider_id: DataProviderId) -> Vec<(CurrencyId, Option<TimeStampedPrice>)> {
			match provider_id {
				DataProviderId::Acala => AcalaOracle::get_all_values(),
				DataProviderId::Aggregated => <AggregatedDataProvider as DataProviderExtended<_, _>>::get_all_values()
			}
		}
	}

	impl orml_tokens_runtime_api::TokensApi<
		Block,
		CurrencyId,
		Balance,
	> for Runtime {
		fn query_existential_deposit(key: CurrencyId) -> Balance {
			if key == GetNativeCurrencyId::get() {
				NativeTokenExistentialDeposit::get()
			} else {
				ExistentialDeposits::get(&key)
			}
		}
	}

	impl module_currencies_runtime_api::CurrenciesApi<
		Block,
		CurrencyId,
		AccountId,
		Balance,
	> for Runtime {
		fn query_free_balance(currency_id: CurrencyId, who: AccountId) -> Balance {
			Currencies::free_balance(currency_id, &who)
		}
	}

	impl module_evm_rpc_runtime_api::EVMRuntimeRPCApi<Block, Balance, AccountId> for Runtime {
		fn block_limits() -> BlockLimits {
			BlockLimits {
				max_gas_limit: runtime_common::EvmLimits::<Runtime>::max_gas_limit(),
				max_storage_limit: runtime_common::EvmLimits::<Runtime>::max_storage_limit(),
			}
		}

		// required by xtokens precompile
		#[transactional]
		fn call(
			from: H160,
			to: H160,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u64,
			storage_limit: u32,
			access_list: Option<Vec<AccessListItem>>,
			_estimate: bool,
		) -> Result<CallInfo, sp_runtime::DispatchError> {
			<Runtime as module_evm::Config>::Runner::rpc_call(
				from,
				from,
				to,
				data,
				value,
				gas_limit,
				storage_limit,
				access_list.unwrap_or_default().into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				vec![],
				<Runtime as module_evm::Config>::config(),
			)
		}

		fn create(
			from: H160,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u64,
			storage_limit: u32,
			access_list: Option<Vec<AccessListItem>>,
			_estimate: bool,
		) -> Result<CreateInfo, sp_runtime::DispatchError> {
			<Runtime as module_evm::Config>::Runner::rpc_create(
				from,
				data,
				value,
				gas_limit,
				storage_limit,
				access_list.unwrap_or_default().into_iter().map(|v| (v.address, v.storage_keys)).collect(),
				vec![],
				<Runtime as module_evm::Config>::config(),
			)
		}

		fn get_estimate_resources_request(extrinsic: Vec<u8>) -> Result<EstimateResourcesRequest, sp_runtime::DispatchError> {
			let utx = UncheckedExtrinsic::decode_all_with_depth_limit(MAX_EXTRINSIC_DEPTH, &mut &*extrinsic)
				.map_err(|_| sp_runtime::DispatchError::Other("Invalid parameter extrinsic, decode failed"))?;

			let request = match utx.0.function {
				RuntimeCall::EVM(module_evm::Call::call{target, input, value, gas_limit, storage_limit, access_list}) => {
					Some(EstimateResourcesRequest {
						from: None,
						to: Some(target),
						gas_limit: Some(gas_limit),
						storage_limit: Some(storage_limit),
						value: Some(value),
						data: Some(input),
						access_list: Some(access_list)
					})
				}
				RuntimeCall::EVM(module_evm::Call::create{input, value, gas_limit, storage_limit, access_list}) => {
					Some(EstimateResourcesRequest {
						from: None,
						to: None,
						gas_limit: Some(gas_limit),
						storage_limit: Some(storage_limit),
						value: Some(value),
						data: Some(input),
						access_list: Some(access_list)
					})
				}
				_ => None,
			};

			request.ok_or(sp_runtime::DispatchError::Other("Invalid parameter extrinsic, not evm Call"))
		}

		// required by xtokens precompile
		#[transactional]
		fn account_call(
			from: AccountId,
			to: H160,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u64,
			storage_limit: u32,
			access_list: Option<Vec<AccessListItem>>,
			estimate: bool,
		) -> Result<CallInfo, sp_runtime::DispatchError> {
			let from = EvmAddressMapping::<Runtime>::get_or_create_evm_address(&from);

			Self::call(from, to, data, value, gas_limit, storage_limit, access_list, estimate)
		}

		fn account_create(
			from: AccountId,
			data: Vec<u8>,
			value: Balance,
			gas_limit: u64,
			storage_limit: u32,
			access_list: Option<Vec<AccessListItem>>,
			estimate: bool,
		) -> Result<CreateInfo, sp_runtime::DispatchError> {
			let from = EvmAddressMapping::<Runtime>::get_or_create_evm_address(&from);

			Self::create(from, data, value, gas_limit, storage_limit, access_list, estimate)
		}
	}

	#[cfg(feature = "tracing")]
	impl module_evm_rpc_runtime_api::EVMTraceApi<Block> for Runtime {
		fn trace_extrinsic(
			extrinsic: <Block as BlockT>::Extrinsic,
			tracer_config: primitives::evm::tracing::TracerConfig,
		) -> Result<module_evm::runner::tracing::TraceOutcome, sp_runtime::transaction_validity::TransactionValidityError> {
			let mut tracer = module_evm::runner::tracing::Tracer::new(tracer_config);
			module_evm::runner::tracing::using(&mut tracer, || {
				Executive::apply_extrinsic(extrinsic)
			}).map(|_| tracer.finalize())
		}
	}

	impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
		fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
			ParachainSystem::collect_collation_info(header)
		}
	}

	impl xcm_runtime_apis::fees::XcmPaymentApi<Block> for Runtime {
		fn query_acceptable_payment_assets(xcm_version: XcmVersion) -> Result<Vec<VersionedAssetId>, XcmPaymentApiError> {
			let acceptable_assets = vec![AssetId(xcm_config::SelfLocation::get())];
			PolkadotXcm::query_acceptable_payment_assets(xcm_version, acceptable_assets)
		}

		fn query_weight_to_asset_fee(weight: Weight, asset: VersionedAssetId) -> Result<u128, XcmPaymentApiError> {
			use crate::xcm_config::XcmConfig;
			type Trader = <XcmConfig as xcm_executor::Config>::Trader;
			PolkadotXcm::query_weight_to_asset_fee::<Trader>(weight, asset)
		}

		fn query_xcm_weight(message: VersionedXcm<()>) -> Result<Weight, XcmPaymentApiError> {
			PolkadotXcm::query_xcm_weight(message)
		}

		fn query_delivery_fees(destination: VersionedLocation, message: VersionedXcm<()>) -> Result<VersionedAssets, XcmPaymentApiError> {
			PolkadotXcm::query_delivery_fees(destination, message)
		}
	}

	impl xcm_runtime_apis::dry_run::DryRunApi<Block, RuntimeCall, RuntimeEvent, OriginCaller> for Runtime {
		fn dry_run_call(origin: OriginCaller, call: RuntimeCall, result_xcms_version: XcmVersion) -> Result<CallDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_call::<Runtime, xcm_config::XcmRouter, OriginCaller, RuntimeCall>(origin, call, result_xcms_version)
		}

		fn dry_run_xcm(origin_location: VersionedLocation, xcm: VersionedXcm<RuntimeCall>) -> Result<XcmDryRunEffects<RuntimeEvent>, XcmDryRunApiError> {
			PolkadotXcm::dry_run_xcm::<Runtime, xcm_config::XcmRouter, RuntimeCall, xcm_config::XcmConfig>(origin_location, xcm)
		}
	}

	#[cfg(feature = "try-runtime")]
	impl frame_try_runtime::TryRuntime<Block> for Runtime {
		fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
			log::info!("try-runtime::on_runtime_upgrade");
			let weight = Executive::try_runtime_upgrade(checks).unwrap();
			(weight, RuntimeBlockWeights::get().max_block)
		}

		fn execute_block(
			block: Block,
			state_root_check: bool,
			signature_check: bool,
			select: frame_try_runtime::TryStateSelect
		) -> Weight {
			log::info!(
				target: "node-runtime",
				"try-runtime: executing block {:?} / root checks: {:?} / signature check: {:?} / try-state-select: {:?}",
				block.header.hash(),
				state_root_check,
				signature_check,
				select,
			);
			Executive::try_execute_block(block, state_root_check, signature_check, select).expect("try_execute_block failed")
		}
	}

	// benchmarks for acala modules
	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn benchmark_metadata(extra: bool) -> (
			Vec<frame_benchmarking::BenchmarkList>,
			Vec<frame_support::traits::StorageInfo>,
		) {
			let mut list = Vec::<BenchmarkList>::new();
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();
			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
			let mut whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

			// Treasury Account
			// TODO: this is manual for now, someday we might be able to use a
			// macro for this particular key
			let treasury_key = frame_system::Account::<Runtime>::hashed_key_for(Treasury::account_id());
			whitelist.push(treasury_key.to_vec().into());

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			add_benchmarks!(params, batches);

			if batches.is_empty() { return Err("Benchmark not found for this module.".into()) }
			Ok(batches)
		}
	}

	#[cfg(feature = "genesis-builder")]
	impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
		fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
			frame_support::genesis_builder_helper::build_state::<RuntimeGenesisConfig>(config)
		}

		fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
			frame_support::genesis_builder_helper::get_preset::<RuntimeGenesisConfig>(id, &genesis_config_presets::get_preset)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			genesis_config_presets::preset_names()
		}
	}
}

cumulus_pallet_parachain_system::register_validate_block!(
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
);

#[derive(Clone, Encode, Decode, DecodeWithMemTracking, PartialEq, Eq, RuntimeDebug)]
pub struct ConvertEthereumTx;

impl Convert<(RuntimeCall, TxExtension), Result<(EthereumTransactionMessage, TxExtension), InvalidTransaction>>
	for ConvertEthereumTx
{
	fn convert(
		(call, mut extra): (RuntimeCall, TxExtension),
	) -> Result<(EthereumTransactionMessage, TxExtension), InvalidTransaction> {
		match call {
			RuntimeCall::EVM(module_evm::Call::eth_call {
				action,
				input,
				value,
				gas_limit,
				storage_limit,
				access_list,
				valid_until,
			}) => {
				if System::block_number() > valid_until {
					if cfg!(feature = "tracing") {
						// skip check when enable tracing feature
					} else {
						return Err(InvalidTransaction::Stale);
					}
				}

				let (_, _, _, _, mortality, check_nonce, _, _, _, charge) = extra.clone();

				if mortality != frame_system::CheckEra::from(sp_runtime::generic::Era::Immortal) {
					// require immortal
					return Err(InvalidTransaction::BadProof);
				}

				let nonce = check_nonce.nonce;
				let tip = charge.0;

				extra.5.mark_as_ethereum_tx(valid_until);

				Ok((
					EthereumTransactionMessage {
						chain_id: EVM::chain_id(),
						genesis: System::block_hash(0),
						nonce,
						tip,
						gas_price: Default::default(),
						gas_limit,
						storage_limit,
						action,
						value,
						input,
						valid_until,
						access_list,
					},
					extra,
				))
			}
			RuntimeCall::EVM(module_evm::Call::eth_call_v2 {
				action,
				input,
				value,
				gas_price,
				gas_limit,
				access_list,
			}) => {
				let (tip, valid_until) =
					decode_gas_price(gas_price, gas_limit, TxFeePerGasV2::get()).ok_or(InvalidTransaction::Stale)?;

				if System::block_number() > valid_until {
					if cfg!(feature = "tracing") {
						// skip check when enable tracing feature
					} else {
						return Err(InvalidTransaction::Stale);
					}
				}

				let (_, _, _, _, mortality, check_nonce, _, _, _, charge) = extra.clone();

				if mortality != frame_system::CheckEra::from(sp_runtime::generic::Era::Immortal) {
					// require immortal
					return Err(InvalidTransaction::BadProof);
				}

				let nonce = check_nonce.nonce;
				if tip != charge.0 {
					// The tip decoded from gas-price is different from the extra
					return Err(InvalidTransaction::BadProof);
				}

				extra.5.mark_as_ethereum_tx(valid_until);

				let storage_limit = decode_gas_limit(gas_limit).1;

				Ok((
					EthereumTransactionMessage {
						chain_id: EVM::chain_id(),
						genesis: System::block_hash(0),
						nonce,
						tip,
						gas_price,
						gas_limit,
						storage_limit,
						action,
						value,
						input,
						valid_until,
						access_list,
					},
					extra,
				))
			}
			_ => Err(InvalidTransaction::BadProof),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{dispatch::DispatchClass, traits::WhitelistedStorageKeys};
	use frame_system::offchain::CreateSignedTransaction;
	use sp_core::hexdisplay::HexDisplay;
	use sp_runtime::traits::Convert;
	use std::collections::HashSet;

	fn run_with_system_weight<F>(w: Weight, mut assertions: F)
	where
		F: FnMut(),
	{
		let mut t: sp_io::TestExternalities = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap()
			.into();
		t.execute_with(|| {
			System::set_block_consumed_resources(w, 0);
			assertions()
		});
	}

	#[test]
	fn check_whitelist() {
		let whitelist: HashSet<String> = AllPalletsWithSystem::whitelisted_storage_keys()
			.iter()
			.map(|e| HexDisplay::from(&e.key).to_string())
			.collect();

		// Block Number
		assert!(whitelist.contains("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac"));
		// Total Issuance
		assert!(whitelist.contains("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80"));
		// Execution Phase
		assert!(whitelist.contains("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a"));
		// Event Count
		assert!(whitelist.contains("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850"));
		// System Events
		assert!(whitelist.contains("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7"));
		// System BlockWeight
		assert!(whitelist.contains("26aa394eea5630e07c48ae0c9558cef734abf5cb34d6244378cddbf18e849d96"));
	}

	#[test]
	fn validate_transaction_submitter_bounds() {
		fn is_submit_signed_transaction<T>()
		where
			T: CreateSignedTransaction<RuntimeCall>,
		{
		}

		is_submit_signed_transaction::<Runtime>();
	}

	#[test]
	fn multiplier_can_grow_from_zero() {
		let minimum_multiplier = MinimumMultiplier::get();
		let target =
			TargetBlockFullness::get() * RuntimeBlockWeights::get().get(DispatchClass::Normal).max_total.unwrap();
		// if the min is too small, then this will not change, and we are doomed forever.
		// the weight is 1/100th bigger than target.
		run_with_system_weight(target * 101 / 100, || {
			let next = SlowAdjustingFeeUpdate::<Runtime>::convert(minimum_multiplier);
			assert!(next > minimum_multiplier, "{:?} !>= {:?}", next, minimum_multiplier);
		})
	}

	#[test]
	fn ensure_can_create_contract() {
		// Ensure that the `ExistentialDeposit` for creating the contract >= account `ExistentialDeposit`.
		// Otherwise, the creation of the contract account will fail because it is less than
		// ExistentialDeposit.
		assert!(
			Balance::from(NewContractExtraBytes::get()).saturating_mul(
				<StorageDepositPerByte as frame_support::traits::Get<Balance>>::get() / 10u128.saturating_pow(6)
			) >= NativeTokenExistentialDeposit::get()
		);
	}

	#[test]
	fn ensure_can_kick_collator() {
		// Ensure that `required_point` > 0, collator can be kicked out normally.
		assert!(
			CollatorKickThreshold::get().mul_floor(
				(SessionDuration::get() * module_collator_selection::POINT_PER_BLOCK)
					.checked_div(<Runtime as module_collator_selection::Config>::MaxCandidates::get())
					.unwrap()
			) > 0
		);
	}

	#[test]
	fn check_call_size() {
		assert!(
			core::mem::size_of::<RuntimeCall>() <= 280,
			"size of RuntimeCall is more than 260 bytes: some calls have too big arguments, use Box to \
			reduce the size of RuntimeCall.
			If the limit is too strong, maybe consider increasing the limit",
		);
	}

	#[test]
	fn check_on_initialize_with_bump_era_weight() {
		use module_homa::WeightInfo;
		let weight = weights::module_homa::WeightInfo::<Runtime>::on_initialize_with_bump_era(
			<Runtime as module_homa::Config>::ProcessRedeemRequestsLimit::get(),
		);
		let block_weight = RuntimeBlockWeights::get().max_block.div(3).mul(2);
		assert!(weight.all_lt(block_weight));
	}
}
