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

use cumulus_pallet_parachain_system::RelaychainDataProvider;
use frame_support::{
	construct_runtime,
	pallet_prelude::InvalidTransaction,
	parameter_types,
	traits::{
		fungible::HoldConsideration,
		tokens::{PayFromAccount, UnityAssetBalanceConversion},
		ConstBool, ConstU128, ConstU32, ConstU64, Contains, ContainsLengthBound, Currency as PalletCurrency,
		EnsureOrigin, EqualPrivilegeOnly, Get, Imbalance, InstanceFilter, LinearStoragePrice, LockIdentifier,
		OnUnbalanced, SortedMembers,
	},
	transactional,
	weights::{constants::RocksDbWeight, ConstantMultiplier, Weight},
	PalletId,
};
use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use module_asset_registry::{AssetIdMaps, EvmErc20InfoMapping};
use module_cdp_engine::CollateralCurrencyIds;
use module_currencies::BasicCurrencyAdapter;
use module_evm::{runner::RunnerExtended, CallInfo, CreateInfo, EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_relaychain::RelayChainCallBuilder;
use module_support::{AddressMapping, AssetIdMapping, DispatchableTask, ExchangeRateProvider, FractionalRate, PoolId};
use module_transaction_payment::TargetedFeeAdjustment;
use parity_scale_codec::{Decode, DecodeLimit, Encode};
use scale_info::TypeInfo;

use orml_tokens::CurrencyAdapter;
use orml_traits::{
	create_median_value_data_provider, define_aggregrated_parameters, parameter_type_with_key,
	parameters::ParameterStoreAdapter, DataFeeder, DataProviderExtended, GetByKey, MultiCurrency,
};
use pallet_transaction_payment::{FeeDetails, RuntimeDispatchInfo};
use primitives::{
	define_combined_task,
	evm::{decode_gas_limit, decode_gas_price, AccessListItem, EthereumTransactionMessage},
	task::TaskResult,
	unchecked_extrinsic::AcalaUncheckedExtrinsic,
};
use sp_api::impl_runtime_apis;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, BadOrigin, BlakeTwo256, Block as BlockT, Bounded, Convert, IdentityLookup,
		SaturatedConversion, StaticLookup,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ArithmeticError, DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Percent, Permill, Perquintill};

pub use authority::AuthorityConfigImpl;
pub use constants::{fee::*, time::*};
pub use primitives::{
	currency::AssetIds,
	evm::{BlockLimits, EstimateResourcesRequest},
	AccountId, AccountIndex, Address, Amount, AuctionId, AuthoritysOriginId, Balance, BlockNumber, CurrencyId,
	DataProviderId, EraIndex, Hash, Lease, Moment, Multiplier, Nonce, ReserveIdentifier, Share, Signature, TokenSymbol,
	TradingPair,
};
use runtime_common::precompile::AcalaPrecompiles;
use runtime_common::{
	cent, dollar, millicent, AllPrecompiles, CheckRelayNumber, ConsensusHook, CurrencyHooks,
	EnsureRootOrAllGeneralCouncil, EnsureRootOrAllTechnicalCommittee, EnsureRootOrHalfFinancialCouncil,
	EnsureRootOrHalfGeneralCouncil, EnsureRootOrHalfHomaCouncil, EnsureRootOrOneGeneralCouncil,
	EnsureRootOrOneThirdsTechnicalCommittee, EnsureRootOrThreeFourthsGeneralCouncil,
	EnsureRootOrTwoThirdsGeneralCouncil, EnsureRootOrTwoThirdsTechnicalCommittee, ExchangeRate,
	ExistentialDepositsTimesOneHundred, FinancialCouncilInstance, FinancialCouncilMembershipInstance, GasToWeight,
	GeneralCouncilInstance, GeneralCouncilMembershipInstance, HomaCouncilInstance, HomaCouncilMembershipInstance,
	MaxTipsOfPriority, OperationalFeeMultiplier, OperatorMembershipInstanceAcala, Price, ProxyType, RandomnessSource,
	Rate, Ratio, RuntimeBlockLength, RuntimeBlockWeights, TechnicalCommitteeInstance,
	TechnicalCommitteeMembershipInstance, TimeStampedPrice, TipPerWeightStep, ACA, AUSD, DOT, KSM, LCDOT, LDOT,
};
use xcm::prelude::*;

/// Import the stable_asset pallet.
pub use nutsfinance_stable_asset;

mod authority;
mod benchmarking;
pub mod constants;
/// Weights for pallets used in the runtime.
mod weights;
pub mod xcm_config;

/// This runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("mandala"),
	impl_name: create_runtime_str!("mandala"),
	authoring_version: 1,
	spec_version: 2260,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 3,
	state_version: 1,
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
	// Treasury reserve
	pub const TreasuryReservePalletId: PalletId = PalletId(*b"aca/reve");
	pub const PhragmenElectionPalletId: LockIdentifier = *b"aca/phre";
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
	pub const NomineesElectionId: LockIdentifier = *b"aca/nome";
	pub UnreleasedNativeVaultAccountId: AccountId = PalletId(*b"aca/urls").into_account_truncating();
	// This Pallet is only used to payment fee pool, it's not added to whitelist by design.
	// because transaction payment pallet will ensure the accounts always have enough ED.
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	pub const LiquidCrowdloanPalletId: PalletId = PalletId(*b"aca/lqcl");
	// Ecosystem modules
	pub const StableAssetPalletId: PalletId = PalletId(*b"nuts/sta");
	// lock identifier for earning module
	pub const EarningLockIdentifier: LockIdentifier = *b"aca/earn";
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![
		CDPEnginePalletId::get().into_account_truncating(),
		TreasuryPalletId::get().into_account_truncating(),
		LoansPalletId::get().into_account_truncating(),
		DEXPalletId::get().into_account_truncating(),
		CDPTreasuryPalletId::get().into_account_truncating(),
		HonzonTreasuryPalletId::get().into_account_truncating(),
		HomaTreasuryPalletId::get().into_account_truncating(),
		IncentivesPalletId::get().into_account_truncating(),
		TreasuryReservePalletId::get().into_account_truncating(),
		CollatorPotId::get().into_account_truncating(),
		UnreleasedNativeVaultAccountId::get(),
		StableAssetPalletId::get().into_account_truncating(),
	]
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = HOURS; // mortal tx can be valid up to 1 hour after signing
	pub const Version: RuntimeVersion = VERSION;
	pub const SS58Prefix: u8 = 42; // Ss58AddressFormat::SubstrateAccount
}

pub struct BaseCallFilter;
impl Contains<RuntimeCall> for BaseCallFilter {
	fn contains(call: &RuntimeCall) -> bool {
		!module_transaction_pause::PausedTransactionFilter::<Runtime>::contains(call)
			&& !matches!(call, RuntimeCall::Democracy(pallet_democracy::Call::propose { .. }),)
	}
}

impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type RuntimeCall = RuntimeCall;
	type Lookup = (Indices, EvmAccounts);
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
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(33);
	pub const SessionDuration: BlockNumber = DAYS; // used in SessionManagerConfig of genesis
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
}

parameter_types! {
	pub const CollatorKickThreshold: Permill = Permill::from_percent(50);
	// Ensure that can create the author(`ExistentialDeposit`) with dev mode.
	pub MinRewardDistributeAmount: Balance = NativeTokenExistentialDeposit::get();
}

impl module_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ValidatorSet = Session;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type PotId = CollatorPotId;
	type MinCandidates = ConstU32<5>;
	type MaxCandidates = ConstU32<200>;
	type MaxInvulnerables = ConstU32<50>;
	type KickPenaltySessionLength = ConstU32<8>;
	type CollatorKickThreshold = CollatorKickThreshold;
	type MinRewardDistributeAmount = MinRewardDistributeAmount;
	type WeightInfo = weights::module_collator_selection::WeightInfo<Runtime>;
}

parameter_types! {
	pub IndexDeposit: Balance = dollar(ACA);
}

impl pallet_indices::Config for Runtime {
	type AccountIndex = AccountIndex;
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = ();
	// type OnTimestampSet = Babe;
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
	pub const MaxReserves: u32 = ReserveIdentifier::Count as u32;
	pub NativeTokenExistentialDeposit: Balance = 10 * cent(ACA);
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
}

parameter_types! {
	pub TransactionByteFee: Balance = millicent(ACA);
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
	pub MaximumMultiplier: Multiplier = Bounded::max_value();
}

impl pallet_sudo::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type WeightInfo = ();
}

parameter_types! {
	pub const GeneralCouncilMotionDuration: BlockNumber = 7 * DAYS;
	pub const CouncilDefaultMaxProposals: u32 = 100;
	pub const CouncilDefaultMaxMembers: u32 = 100;
	pub MaxProposalWeight: Weight = Perbill::from_percent(50) * RuntimeBlockWeights::get().max_block;
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
	pub const FinancialCouncilMotionDuration: BlockNumber = 7 * DAYS;
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
	pub const HomaCouncilMotionDuration: BlockNumber = 7 * DAYS;
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
	pub const TechnicalCommitteeMotionDuration: BlockNumber = 7 * DAYS;
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
	pub MultisigDepositBase: Balance = 500 * millicent(ACA);
	pub MultisigDepositFactor: Balance = 100 * millicent(ACA);
}

impl pallet_multisig::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type DepositBase = MultisigDepositBase;
	type DepositFactor = MultisigDepositFactor;
	type MaxSignatories = ConstU32<100>;
	type WeightInfo = ();
}

pub struct GeneralCouncilProvider;
impl SortedMembers<AccountId> for GeneralCouncilProvider {
	fn sorted_members() -> Vec<AccountId> {
		pallet_collective::Members::<Runtime, pallet_collective::Instance1>::get() // GeneralCouncil::members()
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn add(_: &AccountId) {
		todo!()
	}
}

impl ContainsLengthBound for GeneralCouncilProvider {
	fn max_len() -> usize {
		100
	}
	fn min_len() -> usize {
		0
	}
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub ProposalBondMinimum: Balance = dollar(ACA);
	pub ProposalBondMaximum: Balance = 5 * dollar(ACA);
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TipCountdown: BlockNumber = DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(10);
	pub TipReportDepositBase: Balance = dollar(ACA);
	pub const SevenDays: BlockNumber = 7 * DAYS;
	pub const ZeroDay: BlockNumber = 0;
	pub const OneDay: BlockNumber = DAYS;
	pub BountyDepositBase: Balance = dollar(ACA);
	pub const BountyDepositPayoutDelay: BlockNumber = DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 14 * DAYS;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub CuratorDepositMin: Balance = dollar(ACA);
	pub CuratorDepositMax: Balance = 100 * dollar(ACA);
	pub BountyValueMinimum: Balance = 5 * dollar(ACA);
	pub DataDepositPerByte: Balance = cent(ACA);
	pub const MaximumReasonLength: u32 = 16384;
	pub const MaxApprovals: u32 = 100;
	pub const PayoutSpendPeriod: BlockNumber = 30 * DAYS;
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
	type MaxApprovals = MaxApprovals;
	type AssetKind = ();
	type Beneficiary = AccountId;
	type BeneficiaryLookup = IdentityLookup<Self::Beneficiary>;
	type Paymaster = PayFromAccount<Balances, TreasuryAccount>;
	type BalanceConverter = UnityAssetBalanceConversion;
	type PayoutPeriod = PayoutSpendPeriod;
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
	type OnSlash = ();
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
	type OnSlash = ();
}

parameter_types! {
	pub ConfigDepositBase: Balance = 10 * cent(ACA);
	pub FriendDepositFactor: Balance = cent(ACA);
	pub RecoveryDeposit: Balance = 10 * cent(ACA);
}

impl pallet_recovery::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ConfigDepositBase = ConfigDepositBase;
	type FriendDepositFactor = FriendDepositFactor;
	type MaxFriends = ConstU32<9>;
	type RecoveryDeposit = RecoveryDeposit;
	type WeightInfo = ();
}

parameter_types! {
	pub const LaunchPeriod: BlockNumber = 2 * HOURS;
	pub const VotingPeriod: BlockNumber = HOURS;
	pub const FastTrackVotingPeriod: BlockNumber = HOURS;
	pub MinimumDeposit: Balance = 100 * cent(ACA);
	pub const EnactmentPeriod: BlockNumber = MINUTES;
	pub const CooloffPeriod: BlockNumber = MINUTES;
}

impl pallet_democracy::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type EnactmentPeriod = EnactmentPeriod;
	type LaunchPeriod = LaunchPeriod;
	type VotingPeriod = VotingPeriod;
	type VoteLockingPeriod = EnactmentPeriod; // Same as EnactmentPeriod
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
	//TODO: might need to weight for Mandala
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = CouncilDefaultMaxProposals;
	type Preimages = Preimage;
	type MaxDeposits = ConstU32<100>;
	type MaxBlacklisted = ConstU32<100>;
	type SubmitOrigin = EnsureSigned<AccountId>;
}

impl orml_auction::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManager;
	type WeightInfo = weights::orml_auction::WeightInfo<Runtime>;
}

impl orml_authority::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type Scheduler = Scheduler;
	type AsOriginId = AuthoritysOriginId;
	type AuthorityConfig = AuthorityConfigImpl;
	type WeightInfo = weights::orml_authority::WeightInfo<Runtime>;
}

pub struct PaymentsDisputeResolver;
impl orml_payments::DisputeResolver<AccountId> for PaymentsDisputeResolver {
	fn get_resolver_account() -> AccountId {
		TreasuryAccount::get()
	}
}

pub struct PaymentsFeeHandler;
impl orml_payments::FeeHandler<Runtime> for PaymentsFeeHandler {
	fn apply_fees(
		_from: &AccountId,
		_to: &AccountId,
		_detail: &orml_payments::PaymentDetail<Runtime>,
		_remark: Option<&[u8]>,
	) -> (AccountId, Percent) {
		// we do not charge any fee
		const MARKETPLACE_FEE_PERCENT: Percent = Percent::from_percent(0);
		let fee_receiver = TreasuryAccount::get();
		(fee_receiver, MARKETPLACE_FEE_PERCENT)
	}
}

parameter_types! {
	pub const IncentivePercentage: Percent = Percent::from_percent(5);
	pub const MaxRemarkLength: u32 = 10;
	// 1hr buffer period (60*60)/12
	pub const CancelBufferBlockLength: BlockNumber = 300;
	pub const MaxScheduledTaskListLength : u32 = 5;
}

impl orml_payments::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Asset = Currencies;
	type DisputeResolver = PaymentsDisputeResolver;
	type IncentivePercentage = IncentivePercentage;
	type FeeHandler = PaymentsFeeHandler;
	type MaxRemarkLength = MaxRemarkLength;
	type CancelBufferBlockLength = CancelBufferBlockLength;
	type MaxScheduledTaskListLength = MaxScheduledTaskListLength;
	type WeightInfo = orml_payments::weights::SubstrateWeight<Runtime>;
}

parameter_types! {
	pub CandidacyBond: Balance = 10 * dollar(LDOT);
	pub VotingBondBase: Balance = 2 * dollar(LDOT);
	pub VotingBondFactor: Balance = dollar(LDOT);
	pub const TermDuration: BlockNumber = 7 * DAYS;
}

impl pallet_elections_phragmen::Config for Runtime {
	type PalletId = PhragmenElectionPalletId;
	type RuntimeEvent = RuntimeEvent;
	type Currency = CurrencyAdapter<Runtime, GetLiquidCurrencyId>;
	type CurrencyToVote = sp_staking::currency_to_vote::U128CurrencyToVote;
	type ChangeMembers = HomaCouncil;
	type InitializeMembers = HomaCouncil;
	type CandidacyBond = CandidacyBond;
	type VotingBondBase = VotingBondBase;
	type VotingBondFactor = VotingBondFactor;
	type TermDuration = TermDuration;
	type DesiredMembers = ConstU32<13>;
	type DesiredRunnersUp = ConstU32<7>;
	type LoserCandidate = ();
	type KickedMember = ();
	type MaxVoters = ConstU32<100>;
	type MaxCandidates = ConstU32<20>;
	type MaxVotesPerVoter = ConstU32<5>;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: Moment = 1000 * 60 * 60; // 1 hours
	pub RootOperatorAccountId: AccountId = AccountId::from([0xffu8; 32]);
	pub const MaxFeedValues: u32 = 10; // max 10 values allowd to feed in one call.
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl orml_oracle::BenchmarkHelper<CurrencyId, Price, MaxFeedValues> for BenchmarkHelper {
	fn get_currency_id_value_pairs() -> sp_runtime::BoundedVec<(CurrencyId, Price), MaxFeedValues> {
		sp_runtime::BoundedVec::default()
	}
}

type AcalaDataProvider = orml_oracle::Instance1;
impl orml_oracle::Config<AcalaDataProvider> for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Runtime, MinimumCount, ExpiresIn, AcalaDataProvider>;
	type Time = Timestamp;
	type OracleKey = CurrencyId;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
	type Members = OperatorMembershipAcala;
	type MaxHasDispatchedSize = ConstU32<40>;
	type WeightInfo = weights::orml_oracle::WeightInfo<Runtime>;
	type MaxFeedValues = MaxFeedValues;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelper;
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

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a)
	}
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			CurrencyId::Token(symbol) => match symbol {
				TokenSymbol::AUSD => cent(*currency_id),
				TokenSymbol::DOT => 10 * millicent(*currency_id),
				TokenSymbol::LDOT => 50 * millicent(*currency_id),
				TokenSymbol::BNC => 800 * millicent(*currency_id), // 80BNC = 1KSM
				TokenSymbol::VSKSM => 10 * millicent(*currency_id), // 1VSKSM = 1KSM
				TokenSymbol::PHA => 4000 * millicent(*currency_id), // 400PHA = 1KSM
				TokenSymbol::KUSD |
				TokenSymbol::KSM |
				TokenSymbol::LKSM |
				TokenSymbol::KINT |
				TokenSymbol::KBTC |
				TokenSymbol::TAI => 10 * millicent(*currency_id),
				TokenSymbol::TAP => 10 * millicent(*currency_id),
				TokenSymbol::ACA |
				TokenSymbol::KAR => Balance::max_value() // unsupported
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
						map_or(Balance::max_value(), |metatata| metatata.minimal_balance)
				} else {
					Self::get(&currency_id_0)
				}
			},
			CurrencyId::Erc20(address) => AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::Erc20(*address)).map_or(Balance::max_value(), |metatata| metatata.minimal_balance),
			CurrencyId::StableAssetPoolToken(stable_asset_id) => {
				AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::StableAssetId(*stable_asset_id)).
					map_or(Balance::max_value(), |metatata| metatata.minimal_balance)
			},
			CurrencyId::LiquidCrowdloan(_) => ExistentialDeposits::get(&CurrencyId::Token(TokenSymbol::DOT)), // the same as DOT
			CurrencyId::ForeignAsset(foreign_asset_id) => {
				AssetIdMaps::<Runtime>::get_asset_metadata(AssetIds::ForeignAssetId(*foreign_asset_id)).
					map_or(Balance::max_value(), |metatata| metatata.minimal_balance)
			},
		}
	};
}

parameter_types! {
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = weights::orml_tokens::WeightInfo<Runtime>;
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime, TreasuryAccount>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = DustRemovalWhitelist;
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
	pub RewardRatePerRelaychainBlock: Rate = Rate::saturating_from_rational(2_492, 100_000_000_000u128);	// 14% annual staking reward rate of Polkadot
}

impl module_prices::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub Erc20HoldingAccount: H160 = primitives::evm::ERC20_HOLDING_ACCOUNT;
}

impl module_currencies::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = weights::module_currencies::WeightInfo<Runtime>;
	type AddressMapping = EvmAddressMapping<Runtime>;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type GasToWeight = GasToWeight;
	type SweepOrigin = EnsureRootOrOneGeneralCouncil;
	type OnDust = module_currencies::TransferDust<Runtime, TreasuryAccount>;
}

pub struct EnsureRootOrTreasury;
impl EnsureOrigin<RuntimeOrigin> for EnsureRootOrTreasury {
	type Success = AccountId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o).and_then(|o| match o {
			RawOrigin::Root => Ok(TreasuryPalletId::get().into_account_truncating()),
			RawOrigin::Signed(caller) => {
				if caller == TreasuryPalletId::get().into_account_truncating() {
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
	type RuntimeEvent = RuntimeEvent;
	type Currency = pallet_balances::Pallet<Runtime>;
	type MinVestedTransfer = ConstU128<0>;
	type VestedTransferOrigin = EnsureRootOrTreasury;
	type WeightInfo = weights::orml_vesting::WeightInfo<Runtime>;
	type MaxVestingSchedules = ConstU32<100>;
	type BlockNumberProvider = RelaychainDataProvider<Runtime>;
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
	type MaxScheduledPerBlock = ConstU32<50>;
	type WeightInfo = ();
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type Preimages = Preimage;
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
	type RuntimeEvent = RuntimeEvent;
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
}

impl module_loans::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: RuntimeCall,
		public: <Signature as sp_runtime::traits::Verify>::Signer,
		account: AccountId,
		nonce: Nonce,
	) -> Option<(
		RuntimeCall,
		<UncheckedExtrinsic as sp_runtime::traits::Extrinsic>::SignaturePayload,
	)> {
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
		let extra: SignedExtra = (
			frame_system::CheckNonZeroSender::<Runtime>::new(),
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			runtime_common::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			module_evm::SetEvmOrigin::<Runtime>::new(),
			module_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				log::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let address = Indices::unlookup(account);
		let (call, extra, _) = raw_payload.deconstruct();
		Some((call, (address, signature, extra)))
	}
}

impl frame_system::offchain::SigningTypes for Runtime {
	type Public = <Signature as sp_runtime::traits::Verify>::Signer;
	type Signature = Signature;
}

impl<C> frame_system::offchain::SendTransactionTypes<C> for Runtime
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(110, 100);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: FractionalRate = FractionalRate::try_from(Rate::saturating_from_rational(5, 100))
	.expect("Rate is in range; qed");
	pub MinimumDebitValue: Balance = dollar(AUSD);
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::saturating_from_rational(10, 100);
	pub MaxLiquidationContractSlippage: Ratio = Ratio::saturating_from_rational(15, 100);
	pub SettleErc20EvmOrigin: AccountId = AccountId::from(hex_literal::hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")); // `5HrN7fHLXWcFiXPwwtq2EkSGns9eMt5P7SpeTPewumZy6ftb`
}

impl module_cdp_engine::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

parameter_types! {
	pub DepositPerAuthorization: Balance = dollar(ACA);
}

impl module_honzon::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type DepositPerAuthorization = DepositPerAuthorization;
	type CollateralCurrencyIds = CollateralCurrencyIds<Runtime>;
	type WeightInfo = weights::module_honzon::WeightInfo<Runtime>;
}

impl module_emergency_shutdown::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type CollateralCurrencyIds = CollateralCurrencyIds<Runtime>;
	type PriceSource = Prices;
	type CDPTreasury = CdpTreasury;
	type AuctionManagerHandler = AuctionManager;
	type ShutdownOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_emergency_shutdown::WeightInfo<Runtime>;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 1000);	// 0.1%
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, ACA).unwrap(),
		TradingPair::from_currency_ids(AUSD, DOT).unwrap(),
		TradingPair::from_currency_ids(DOT, LDOT).unwrap(),
		TradingPair::from_currency_ids(DOT, ACA).unwrap(),
	];
	pub const ExtendedProvisioningBlocks: BlockNumber = 2 * DAYS;
	pub const TradingPathLimit: u32 = 4;
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![GetStakingCurrencyId::get()],
		vec![GetStableCurrencyId::get()],
		vec![GetLiquidCurrencyId::get()],
	];
}

impl module_dex::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = EvmErc20InfoMapping<Runtime>;
	type DEXIncentives = Incentives;
	type WeightInfo = weights::module_dex::WeightInfo<Runtime>;
	type ListingOrigin = EnsureRootOrHalfGeneralCouncil;
	type ExtendedProvisioningBlocks = ExtendedProvisioningBlocks;
	type OnLiquidityPoolUpdated = ();
}

impl module_aggregated_dex::Config for Runtime {
	type DEX = Dex;
	type StableAsset = RebasedStableAsset;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type DexSwapJointList = AlternativeSwapPathJointList;
	type SwapPathLimit = ConstU32<3>;
	type WeightInfo = weights::module_aggregated_dex::WeightInfo<Runtime>;
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
}

parameter_types! {
	pub HonzonTreasuryAccount: AccountId = HonzonTreasuryPalletId::get().into_account_truncating();
}

impl module_cdp_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

impl module_transaction_pause::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type WeightInfo = weights::module_transaction_pause::WeightInfo<Runtime>;
}

parameter_types! {
	pub DefaultFeeTokens: Vec<CurrencyId> = vec![AUSD, DOT, LDOT];
	pub const CustomFeeSurplus: Percent = Percent::from_percent(50);
	pub const AlternativeFeeSurplus: Percent = Percent::from_percent(25);
}

type NegativeImbalance = <Balances as PalletCurrency<AccountId>>::NegativeImbalance;
pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance>) {
		if let Some(mut fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut fees);
			}
			// for fees and tips, 80% to treasury, 20% to collator-selection pot.
			let split = fees.ration(80, 20);
			Treasury::on_unbalanced(split.0);

			Balances::resolve_creating(&CollatorSelection::account_id(), split.1);
			// Due to performance consideration remove the event.
			// let numeric_amount = split.1.peek();
			// let staking_pot = CollatorSelection::account_id();
			// System::deposit_event(pallet_balances::Event::Deposit(staking_pot, numeric_amount));
		}
	}
}

impl module_transaction_payment::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
	type FeeMultiplierUpdate =
		TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier, MaximumMultiplier>;
	type Swap = AcalaSwap;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = module_prices::RealTimePriceProvider<Runtime>;
	type WeightInfo = weights::module_transaction_payment::WeightInfo<Runtime>;
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = TreasuryAccount;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type CustomFeeSurplus = CustomFeeSurplus;
	type AlternativeFeeSurplus = AlternativeFeeSurplus;
	type DefaultFeeTokens = DefaultFeeTokens;
}

impl module_earning::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ParameterStore = ParameterStoreAdapter<Parameters, module_earning::Parameters>;
	type OnBonded = module_incentives::OnEarningBonded<Runtime>;
	type OnUnbonded = module_incentives::OnEarningUnbonded<Runtime>;
	type OnUnstakeFee = Treasury; // fee goes to treasury
	type MinBond = ConstU128<100>;
	type UnbondingPeriod = ConstU32<3>;
	type MaxUnbondingChunks = ConstU32<3>;
	type LockIdentifier = EarningLockIdentifier;
	type WeightInfo = weights::module_earning::WeightInfo<Runtime>;
}

impl module_evm_accounts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type AddressMapping = EvmAddressMapping<Runtime>;
	type TransferAll = Currencies;
	type ChainId = EvmChainId<Runtime>;
	type WeightInfo = weights::module_evm_accounts::WeightInfo<Runtime>;
}

impl module_asset_registry::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type StakingCurrencyId = GetStakingCurrencyId;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type RegisterOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_asset_registry::WeightInfo<Runtime>;
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
	type RuntimeEvent = RuntimeEvent;
	type RewardsSource = UnreleasedNativeVaultAccountId;
	type NativeCurrencyId = GetNativeCurrencyId;
	type AccumulatePeriod = AccumulatePeriod;
	type UpdateOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type Currency = Currencies;
	type EmergencyShutdown = EmergencyShutdown;
	type PalletId = IncentivesPalletId;
	type WeightInfo = weights::module_incentives::WeightInfo<Runtime>;
}

parameter_types! {
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 100);	// 1 : 10
}

pub fn create_x2_parachain_location(index: u16) -> Location {
	Location::new(
		1,
		AccountId32 {
			network: None,
			id: Utility::derivative_account_id(ParachainInfo::get().into_account_truncating(), index).into(),
		},
	)
}

parameter_types! {
	pub HomaTreasuryAccount: AccountId = HomaTreasuryPalletId::get().into_account_truncating();
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![
		0,  // 15sr8Dvq3AT3Z2Z1y8FnQ4VipekAHhmQnrkgzegUr1tNgbcn
	];
	pub MintThreshold: Balance = dollar(DOT);
	pub RedeemThreshold: Balance = 10 * dollar(LDOT);
	pub const BondingDuration: EraIndex = 28;
}

impl module_homa::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
	type ProcessRedeemRequestsLimit = ConstU32<2_000>;
}

parameter_types! {
	pub MinBondAmount: Balance = 1_000 * dollar(LDOT);
	pub ValidatorInsuranceThreshold: Balance = 10_000 * dollar(LDOT);
}

impl module_homa_validator_list::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RelayChainAccountId = AccountId;
	type LiquidTokenCurrency = module_currencies::Currency<Runtime, GetLiquidCurrencyId>;
	type MinBondAmount = MinBondAmount;
	type BondingDuration = BondingDuration;
	type ValidatorInsuranceThreshold = ValidatorInsuranceThreshold;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type LiquidStakingExchangeRateProvider = Homa;
	type CurrentEra = Homa;
	type WeightInfo = weights::module_homa_validator_list::WeightInfo<Runtime>;
}

parameter_types! {
	pub MinNomineesElectionBondThreshold: Balance = 10 * dollar(LDOT);
	pub const MaxNominateesCount: u32 = 16;
}

impl module_nominees_election::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

parameter_types! {
	pub ParachainAccount: AccountId = ParachainInfo::get().into_account_truncating();
}

pub struct SubAccountIndexLocationConvertor;
impl Convert<u16, Location> for SubAccountIndexLocationConvertor {
	fn convert(sub_account_index: u16) -> Location {
		create_x2_parachain_location(sub_account_index)
	}
}

impl module_xcm_interface::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type StakingCurrencyId = GetStakingCurrencyId;
	type ParachainAccount = ParachainAccount;
	type RelayChainUnbondingSlashingSpans = ConstU32<5>;
	type SovereignSubAccountLocationConvert = SubAccountIndexLocationConvertor;
	type RelayChainCallBuilder = RelayChainCallBuilder<ParachainInfo, module_relaychain::PolkadotRelayChainCall>;
	type XcmTransfer = XTokens;
	type SelfLocation = xcm_config::SelfLocation;
	type AccountIdToLocation = xcm_config::AccountIdToLocation;
}

parameter_types! {
	pub CreateClassDeposit: Balance = 20 * dollar(ACA);
	pub CreateTokenDeposit: Balance = 2 * dollar(ACA);
}

impl module_nft::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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

parameter_types! {
	// One storage item; key size 32, value size 8; .
	pub ProxyDepositBase: Balance = deposit(1, 8);
	// Additional storage item size of 33 bytes.
	pub ProxyDepositFactor: Balance = deposit(0, 33);
	pub AnnouncementDepositBase: Balance = deposit(1, 8);
	pub AnnouncementDepositFactor: Balance = deposit(0, 66);
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
						| RuntimeCall::PhragmenElection(..)
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
}

parameter_types! {
	pub NetworkContractSource: H160 = H160::from_low_u64_be(0);
	pub PrecompilesValue: AllPrecompiles<Runtime, module_transaction_pause::PausedPrecompileFilter<Runtime>, AcalaPrecompiles<Runtime>> = AllPrecompiles::<_, _, _>::mandala();
}

#[cfg(feature = "with-ethereum-compatibility")]
parameter_types! {
	pub const NewContractExtraBytes: u32 = 0;
	pub const DeveloperDeposit: Balance = 0;
	pub const PublicationFee: Balance = 0;
}

#[cfg(not(feature = "with-ethereum-compatibility"))]
parameter_types! {
	pub const NewContractExtraBytes: u32 = 10_000;
	pub DeveloperDeposit: Balance = dollar(ACA);
	pub PublicationFee: Balance = dollar(ACA);
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct StorageDepositPerByte;
impl<I: From<Balance>> frame_support::traits::Get<I> for StorageDepositPerByte {
	fn get() -> I {
		#[cfg(not(feature = "with-ethereum-compatibility"))]
		// NOTE: ACA decimals is 12, convert to 18.
		// 10 * millicent(ACA) * 10^6
		return I::from(100_000_000_000_000);
		#[cfg(feature = "with-ethereum-compatibility")]
		return I::from(0);
	}
}

// TODO: remove
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct TxFeePerGas;
impl<I: From<Balance>> frame_support::traits::Get<I> for TxFeePerGas {
	fn get() -> I {
		// NOTE: 200 GWei
		// ensure suffix is 0x0000
		I::from(200u128.saturating_mul(10u128.saturating_pow(9)) & !0xffff)
	}
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct TxFeePerGasV2;
impl<I: From<Balance>> frame_support::traits::Get<I> for TxFeePerGasV2 {
	fn get() -> I {
		// NOTE: 100 GWei
		I::from(100_000_000_000u128)
	}
}

#[cfg(feature = "with-ethereum-compatibility")]
static SHANGHAI_CONFIG: module_evm_utility::evm::Config = module_evm_utility::evm::Config::shanghai();

impl module_evm::Config for Runtime {
	type AddressMapping = EvmAddressMapping<Runtime>;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = TxFeePerGas;
	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType =
		AllPrecompiles<Self, module_transaction_pause::PausedPrecompileFilter<Self>, AcalaPrecompiles<Runtime>>;
	type PrecompilesValue = PrecompilesValue;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;
	type NetworkContractOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type PublicationFee = PublicationFee;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureRootOrHalfGeneralCouncil;
	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
	type Randomness = RandomnessSource<Runtime>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = weights::module_evm::WeightInfo<Runtime>;

	#[cfg(feature = "with-ethereum-compatibility")]
	fn config() -> &'static module_evm_utility::evm::Config {
		&SHANGHAI_CONFIG
	}
}

impl module_evm_bridge::Config for Runtime {
	type EVM = EVM;
}

impl module_session_manager::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

impl parachain_info::Config for Runtime {}

impl orml_unknown_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = EnsureRootOrHalfGeneralCouncil;
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
			CurrencyId::Token(TokenSymbol::LDOT) => Homa::get_exchange_rate()
				.checked_mul_int(balance)
				.ok_or(ArithmeticError::Overflow)?,
			_ => balance,
		})
	}

	fn convert_balance_back(balance: Balance, asset_id: CurrencyId) -> Result<Balance, ArithmeticError> {
		Ok(match asset_id {
			CurrencyId::Token(TokenSymbol::LDOT) => Homa::get_exchange_rate()
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
		matches!(currency_id, CurrencyId::Token(TokenSymbol::LDOT))
	}
}

type RebaseTokens = orml_tokens::Combiner<
	AccountId,
	IsLiquidToken,
	orml_tokens::Mapper<AccountId, Currencies, ConvertBalanceHoma, Balance, GetStableAssetStakingCurrencyId>,
	Currencies,
>;

parameter_types! {
	pub const GetStableAssetStakingCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
}

impl nutsfinance_stable_asset::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
}

define_combined_task! {
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<Runtime>),
	}
}

parameter_types!(
	// At least 2% of max block weight should remain before idle tasks are dispatched.
	pub MinimumWeightRemainInBlock: Weight = RuntimeBlockWeights::get().max_block / 50;
);

impl module_idle_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = weights::module_idle_scheduler::WeightInfo<Runtime>;
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = RelaychainDataProvider<Runtime>;
	// Number of relay chain blocks produced with no parachain blocks finalized,
	// once this number is reached idle scheduler is disabled as block production is slow
	type DisableBlockThreshold = ConstU32<6>;
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types!(
	pub const LiquidCrowdloanCurrencyId: CurrencyId = LCDOT;
);

impl module_liquid_crowdloan::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LiquidCrowdloanCurrencyId = LiquidCrowdloanCurrencyId;
	type RelayChainCurrencyId = GetStakingCurrencyId;
	type PalletId = LiquidCrowdloanPalletId;
	type GovernanceOrigin = EnsureRootOrHalfGeneralCouncil;
	type WeightInfo = weights::module_liquid_crowdloan::WeightInfo<Runtime>;
}

define_aggregrated_parameters! {
	pub RuntimeParameters = {
		Earning: module_earning::Parameters = 0,
	}
}

impl orml_parameters::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type AggregratedKeyValue = RuntimeParameters;
	type AdminOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type WeightInfo = ();
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug)]
pub struct ConvertEthereumTx;

impl Convert<(RuntimeCall, SignedExtra), Result<(EthereumTransactionMessage, SignedExtra), InvalidTransaction>>
	for ConvertEthereumTx
{
	fn convert(
		(call, mut extra): (RuntimeCall, SignedExtra),
	) -> Result<(EthereumTransactionMessage, SignedExtra), InvalidTransaction> {
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

				let (_, _, _, _, mortality, check_nonce, _, _, charge) = extra.clone();

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

				let (_, _, _, _, mortality, check_nonce, _, _, charge) = extra.clone();

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

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
	frame_system::CheckNonZeroSender<Runtime>,
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	runtime_common::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	// `SetEvmOrigin` needs ahead of `ChargeTransactionPayment`, we set origin in `SetEvmOrigin::validate()`, then
	// `ChargeTransactionPayment::validate()` can process erc20 token transfer successfully in the case of using erc20
	// as fee token.
	module_evm::SetEvmOrigin<Runtime>,
	module_transaction_payment::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
	AcalaUncheckedExtrinsic<RuntimeCall, SignedExtra, ConvertEthereumTx, StorageDepositPerByte, TxFeePerGas>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<RuntimeCall, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, RuntimeCall, SignedExtra>;
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

construct_runtime!(
	pub enum Runtime {
		// Core
		System: frame_system = 0,
		Timestamp: pallet_timestamp = 1,
		Scheduler: pallet_scheduler = 2,
		TransactionPause: module_transaction_pause = 3,
		Preimage: pallet_preimage = 4,

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

		// Utility
		Utility: pallet_utility = 30,
		Multisig: pallet_multisig = 31,
		Recovery: pallet_recovery = 32,
		Proxy: pallet_proxy = 33,
		// NOTE: IdleScheduler must be put before ParachainSystem in order to read relaychain blocknumber
		IdleScheduler: module_idle_scheduler = 34,

		Indices: pallet_indices = 40,

		// Governance
		GeneralCouncil: pallet_collective::<Instance1> = 50,
		GeneralCouncilMembership: pallet_membership::<Instance1> = 51,
		FinancialCouncil: pallet_collective::<Instance2> = 52,
		FinancialCouncilMembership: pallet_membership::<Instance2> = 53,
		HomaCouncil: pallet_collective::<Instance3> = 54,
		HomaCouncilMembership: pallet_membership::<Instance3> = 55,
		TechnicalCommittee: pallet_collective::<Instance4> = 56,
		TechnicalCommitteeMembership: pallet_membership::<Instance4> = 57,

		Authority: orml_authority = 70,
		PhragmenElection: pallet_elections_phragmen = 71,
		Democracy: pallet_democracy = 72,

		// Oracle
		//
		// NOTE: OperatorMembership must be placed after Oracle or else will have race condition on initialization
		AcalaOracle: orml_oracle::<Instance1> = 80,
		OperatorMembershipAcala: pallet_membership::<Instance5> = 82,

		// ORML Core
		Auction: orml_auction = 100,
		Rewards: orml_rewards = 101,
		OrmlNFT: orml_nft exclude_parts { Call } = 102,
		Parameters: orml_parameters = 103,

		// Acala Core
		Prices: module_prices = 110,
		Dex: module_dex = 111,
		DexOracle: module_dex_oracle = 112,
		AggregatedDex: module_aggregated_dex = 113,

		// Honzon
		AuctionManager: module_auction_manager = 120,
		Loans: module_loans = 121,
		Honzon: module_honzon = 122,
		CdpTreasury: module_cdp_treasury = 123,
		CdpEngine: module_cdp_engine = 124,
		EmergencyShutdown: module_emergency_shutdown = 125,

		// Homa
		NomineesElection: module_nominees_election = 131,
		Homa: module_homa = 136,
		XcmInterface: module_xcm_interface = 137,
		HomaValidatorList: module_homa_validator_list = 138,

		// Acala Other
		Incentives: module_incentives = 140,
		NFT: module_nft = 141,
		AssetRegistry: module_asset_registry = 142,
		LiquidCrowdloan: module_liquid_crowdloan = 143,

		// Parachain
		ParachainInfo: parachain_info exclude_parts { Call } = 161,

		// XCM
		XcmpQueue: cumulus_pallet_xcmp_queue = 170,
		PolkadotXcm: pallet_xcm = 171,
		CumulusXcm: cumulus_pallet_xcm exclude_parts { Call } = 172,
		// DmpQueue is removed
		XTokens: orml_xtokens = 174,
		UnknownTokens: orml_unknown_tokens = 175,
		OrmlXcm: orml_xcm = 176,
		MessageQueue: pallet_message_queue = 177,

		// Smart contracts
		EVM: module_evm = 180,
		EVMBridge: module_evm_bridge exclude_parts { Call } = 181,
		EvmAccounts: module_evm_accounts = 182,

		// Collator support. the order of these 4 are important and shall not change.
		Authorship: pallet_authorship = 190,
		CollatorSelection: module_collator_selection = 191,
		Session: pallet_session = 192,
		Aura: pallet_aura = 193,
		AuraExt: cumulus_pallet_aura_ext = 194,
		SessionManager: module_session_manager = 195,

		// Stable asset
		StableAsset: nutsfinance_stable_asset = 200,
		Payments: orml_payments = 201,

		// Staking related pallets
		Earning: module_earning = 210,

		// Parachain System, always put it at the end
		ParachainSystem: cumulus_pallet_parachain_system = 160,

		// Dev
		Sudo: pallet_sudo = 255,
	}
);

#[cfg(feature = "runtime-benchmarks")]
#[macro_use]
extern crate orml_benchmarking;

#[cfg(feature = "runtime-benchmarks")]
mod benches {
	define_benchmarks!(
		[module_dex, benchmarking::dex]
		[module_dex_oracle, benchmarking::dex_oracle]
		[module_asset_registry, benchmarking::asset_registry]
		[module_auction_manager, benchmarking::auction_manager]
		[module_cdp_engine, benchmarking::cdp_engine]
		[module_earning, benchmarking::earning]
		[module_emergency_shutdown, benchmarking::emergency_shutdown]
		[module_evm, benchmarking::evm]
		[module_homa, benchmarking::homa]
		[module_homa_validator_list, benchmarking::homa_validator_list]
		[module_honzon, benchmarking::honzon]
		[module_cdp_treasury, benchmarking::cdp_treasury]
		[module_collator_selection, benchmarking::collator_selection]
		[module_nominees_election, benchmarking::nominees_election]
		[module_transaction_pause, benchmarking::transaction_pause]
		[module_transaction_payment, benchmarking::transaction_payment]
		[module_incentives, benchmarking::incentives]
		[module_prices, benchmarking::prices]
		[module_evm_accounts, benchmarking::evm_accounts]
		[module_currencies, benchmarking::currencies]
		[module_session_manager, benchmarking::session_manager]
		[module_liquid_crowdloan, benchmarking::liquid_crowdloan]
		[orml_tokens, benchmarking::tokens]
		[orml_vesting, benchmarking::vesting]
		[orml_auction, benchmarking::auction]
		[orml_authority, benchmarking::authority]
		[nutsfinance_stable_asset, benchmarking::nutsfinance_stable_asset]
		[module_idle_scheduler, benchmarking::idle_scheduler]
		[module_aggregated_dex, benchmarking::aggregated_dex]
	);
	// frame_benchmarking::define_benchmarks!(
	// 	// XCM
	// 	[pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
	// );
}

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
			TransactionPayment::query_info(uxt, len)
		}
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> FeeDetails<Balance> {
			TransactionPayment::query_fee_details(uxt, len)
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
		fn get_value(provider_id: DataProviderId ,key: CurrencyId) -> Option<TimeStampedPrice> {
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
				<Runtime as module_evm::Config>::config(),
			)
		}

		fn get_estimate_resources_request(extrinsic: Vec<u8>) -> Result<EstimateResourcesRequest, sp_runtime::DispatchError> {
			let utx = UncheckedExtrinsic::decode_all_with_depth_limit(sp_api::MAX_EXTRINSIC_DEPTH, &mut &*extrinsic)
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
			use frame_benchmarking::{list_benchmark as frame_list_benchmark, Benchmarking, BenchmarkList};
			use frame_support::traits::StorageInfoTrait;

			use module_nft::benchmarking::Pallet as NftBench;
			// use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;

			let mut list = Vec::<BenchmarkList>::new();

			frame_list_benchmark!(list, extra, module_nft, NftBench::<Runtime>);
			list_benchmarks!(list, extra);

			let storage_info = AllPalletsWithSystem::storage_info();

			return (list, storage_info)
		}

		fn dispatch_benchmark(
			config: frame_benchmarking::BenchmarkConfig
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, BenchmarkError, add_benchmark as frame_add_benchmark};
			use module_nft::benchmarking::Pallet as NftBench;
			use frame_support::traits::{WhitelistedStorageKeys, TrackedStorageKey};

			// const UNITS: Balance = 1_000_000_000_000;
			// const CENTS: Balance = UNITS / 100;

			// parameter_types! {
			// 	pub FeeAssetId: AssetId = AssetId(Location::parent());
			// 	pub const BaseDeliveryFee: u128 = CENTS.saturating_mul(3);
			// }
			// pub type PriceForParentDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
			// 	FeeAssetId,
			// 	BaseDeliveryFee,
			// 	TransactionByteFee,
			// 	ParachainSystem,
			// >;

			// use pallet_xcm::benchmarking::Pallet as PalletXcmExtrinsicsBenchmark;
			// impl pallet_xcm::benchmarking::Config for Runtime {
			// 	type DeliveryHelper = cumulus_primitives_utility::ToParentDeliveryHelper<
			// 		xcm_config::XcmConfig,
			// 		ExistentialDepositAsset,
			// 		PriceForParentDelivery,
			// 	>;
			// 	fn reachable_dest() -> Option<Location> {
			// 		Some(Parent.into())
			// 	}

			// 	fn teleportable_asset_and_dest() -> Option<(Asset, Location)> {
			// 		Some((
			// 			Asset {
			// 				fun: Fungible(NativeTokenExistentialDeposit::get()),
			// 				id: AssetId(Parent.into())
			// 			},
			// 			Parent.into(),
			// 		))
			// 	}

			// 	fn reserve_transferable_asset_and_dest() -> Option<(Asset, Location)> {
			// 		None
			// 	}

			// 	fn get_asset() -> Asset {
			// 		Asset {
			// 			id: AssetId(Location::parent()),
			// 			fun: Fungible(UNITS),
			// 		}
			// 	}
			// }

			// parameter_types! {
			// 	pub ExistentialDepositAsset: Option<Asset> = Some((
			// 		Location::parent(),
			// 		NativeTokenExistentialDeposit::get()
			// 	).into());
			// }

			// impl pallet_xcm_benchmarks::Config for Runtime {
			// 	type XcmConfig = xcm_config::XcmConfig;
			// 	type AccountIdConverter = xcm_config::LocationToAccountId;
			// 	type DeliveryHelper = cumulus_primitives_utility::ToParentDeliveryHelper<
			// 		xcm_config::XcmConfig,
			// 		ExistentialDepositAsset,
			// 		PriceForParentDelivery,
			// 	>;
			// 	fn valid_destination() -> Result<Location, BenchmarkError> {
			// 		Ok(Location::parent())
			// 	}
			// 	fn worst_case_holding(_depositable_count: u32) -> Assets {
			// 		// just concrete assets according to relay chain.
			// 		let assets: Vec<Asset> = vec![
			// 			Asset {
			// 				id: AssetId(Location::parent()),
			// 				fun: Fungible(1_000_000 * UNITS),
			// 			}
			// 		];
			// 		assets.into()
			// 	}
			// }

			let mut whitelist: Vec<TrackedStorageKey> = AllPalletsWithSystem::whitelisted_storage_keys();

			// Treasury Account
			// TODO: this is manual for now, someday we might be able to use a
			// macro for this particular key
			let treasury_key = frame_system::Account::<Runtime>::hashed_key_for(Treasury::account_id());
			whitelist.push(treasury_key.to_vec().into());

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&config, &whitelist);

			frame_add_benchmark!(params, batches, module_nft, NftBench::<Runtime>);
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
			frame_support::genesis_builder_helper::get_preset::<RuntimeGenesisConfig>(id, |_| None)
		}

		fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
			vec![]
		}
	}
}

#[cfg(not(feature = "standalone"))]
cumulus_pallet_parachain_system::register_validate_block!(
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
);

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{dispatch::DispatchInfo, traits::WhitelistedStorageKeys};
	use frame_system::offchain::CreateSignedTransaction;
	use sp_core::hexdisplay::HexDisplay;
	use sp_runtime::traits::SignedExtension;
	use std::collections::HashSet;

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
			"size of RuntimeCall is more than 280 bytes: some calls have too big arguments, use Box to \
			reduce the size of RuntimeCall.
			If the limit is too strong, maybe consider increasing the limit",
		);
	}

	#[test]
	fn convert_tx_check_evm_nonce() {
		sp_io::TestExternalities::new_empty().execute_with(|| {
			let alice: AccountId = sp_runtime::AccountId32::from([8; 32]);
			System::inc_account_nonce(&alice); // system::account.nonce = 1

			let address = EvmAddressMapping::<Runtime>::get_evm_address(&alice)
				.unwrap_or_else(|| EvmAddressMapping::<Runtime>::get_default_evm_address(&alice));

			// set evm nonce to 3
			module_evm::Accounts::<Runtime>::insert(
				&address,
				module_evm::AccountInfo {
					nonce: 3,
					contract_info: None,
				},
			);

			let call = RuntimeCall::EVM(module_evm::Call::eth_call {
				action: module_evm::TransactionAction::Create,
				input: vec![0x01],
				value: 0,
				gas_limit: 21_000,
				storage_limit: 1_000,
				valid_until: 30,
				access_list: vec![],
			});

			let extra: SignedExtra = (
				frame_system::CheckNonZeroSender::<Runtime>::new(),
				frame_system::CheckSpecVersion::<Runtime>::new(),
				frame_system::CheckTxVersion::<Runtime>::new(),
				frame_system::CheckGenesis::<Runtime>::new(),
				frame_system::CheckEra::<Runtime>::from(generic::Era::Immortal),
				runtime_common::CheckNonce::<Runtime>::from(3),
				frame_system::CheckWeight::<Runtime>::new(),
				module_evm::SetEvmOrigin::<Runtime>::new(),
				module_transaction_payment::ChargeTransactionPayment::<Runtime>::from(0),
			);

			let mut expected_extra = extra.clone();
			expected_extra.5.mark_as_ethereum_tx(30);

			assert_eq!(
				ConvertEthereumTx::convert((call.clone(), extra.clone())).unwrap(),
				(
					EthereumTransactionMessage {
						nonce: 3, // evm::account.nonce
						tip: 0,
						gas_price: 0,
						gas_limit: 21_000,
						storage_limit: 1_000,
						action: module_evm::TransactionAction::Create,
						value: 0,
						input: vec![0x01],
						chain_id: 0,
						genesis: sp_core::H256::default(),
						valid_until: 30,
						access_list: vec![],
					},
					expected_extra.clone()
				)
			);

			let info = DispatchInfo::default();

			// valid tx in future
			assert_eq!(
				extra.5.validate(&alice, &call, &info, 0),
				Ok(sp_runtime::transaction_validity::ValidTransaction {
					priority: 0,
					requires: vec![Encode::encode(&(alice.clone(), 2u32))],
					provides: vec![Encode::encode(&(alice.clone(), 3u32))],
					longevity: sp_runtime::transaction_validity::TransactionLongevity::MAX,
					propagate: true,
				})
			);
			// valid evm tx
			assert_eq!(
				expected_extra.5.validate(&alice, &call, &info, 0),
				Ok(sp_runtime::transaction_validity::ValidTransaction {
					priority: 0,
					requires: vec![],
					provides: vec![Encode::encode(&(address, 3u32))],
					longevity: 30,
					propagate: true,
				})
			);

			// valid evm tx in future
			expected_extra.5.nonce = 4;
			assert_eq!(
				expected_extra.5.validate(&alice, &call, &info, 0),
				Ok(sp_runtime::transaction_validity::ValidTransaction {
					priority: 0,
					requires: vec![Encode::encode(&(address, 3u32))],
					provides: vec![Encode::encode(&(address, 4u32))],
					longevity: 30,
					propagate: true,
				})
			);
		});
	}
}
