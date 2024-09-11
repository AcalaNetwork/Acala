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

use parity_scale_codec::{Decode, DecodeLimit, Encode};
use scale_info::TypeInfo;
use sp_consensus_aura::sr25519::AuthorityId as AuraId;
use sp_core::{crypto::KeyTypeId, OpaqueMetadata, H160};
use sp_runtime::{
	create_runtime_str, generic, impl_opaque_keys,
	traits::{
		AccountIdConversion, AccountIdLookup, BadOrigin, BlakeTwo256, Block as BlockT, Bounded, Convert,
		IdentityLookup, SaturatedConversion, StaticLookup,
	},
	transaction_validity::{TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ArithmeticError, DispatchResult, FixedPointNumber, Perbill, Percent, Permill, Perquintill,
	RuntimeDebug,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_system::{EnsureRoot, EnsureSigned, RawOrigin};
use module_asset_registry::{AssetIdMaps, EvmErc20InfoMapping};
use module_cdp_engine::CollateralCurrencyIds;
use module_currencies::BasicCurrencyAdapter;
use module_evm::{runner::RunnerExtended, CallInfo, CreateInfo, EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_relaychain::RelayChainCallBuilder;
use module_support::{AddressMapping, AssetIdMapping, DispatchableTask, PoolId};
use module_transaction_payment::TargetedFeeAdjustment;

use cumulus_pallet_parachain_system::RelaychainDataProvider;
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
	PalletId,
};
use orml_traits::{
	create_median_value_data_provider, define_aggregrated_parameters, parameter_type_with_key,
	parameters::ParameterStoreAdapter, DataFeeder, DataProviderExtended, MultiCurrency,
};
use pallet_transaction_payment::RuntimeDispatchInfo;

pub use pallet_collective::MemberCount;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

pub use authority::AuthorityConfigImpl;
pub use constants::{fee::*, time::*};
use module_support::{ExchangeRateProvider, FractionalRate};
use primitives::currency::AssetIds;
pub use primitives::{
	define_combined_task,
	evm::{
		decode_gas_limit, decode_gas_price, AccessListItem, BlockLimits, EstimateResourcesRequest,
		EthereumTransactionMessage,
	},
	task::TaskResult,
	unchecked_extrinsic::AcalaUncheckedExtrinsic,
	AccountId, AccountIndex, Address, Amount, AuctionId, AuthoritysOriginId, Balance, BlockNumber, CurrencyId,
	DataProviderId, DexShare, EraIndex, Hash, Lease, Moment, Multiplier, Nonce, ReserveIdentifier, Share, Signature,
	TokenSymbol, TradingPair,
};
use runtime_common::{
	cent, dollar, millicent, precompile::AcalaPrecompiles, AllPrecompiles, CheckRelayNumber, ConsensusHook,
	CurrencyHooks, EnsureRootOrAllGeneralCouncil, EnsureRootOrAllTechnicalCommittee, EnsureRootOrHalfFinancialCouncil,
	EnsureRootOrHalfGeneralCouncil, EnsureRootOrHalfHomaCouncil, EnsureRootOrOneGeneralCouncil,
	EnsureRootOrOneThirdsTechnicalCommittee, EnsureRootOrThreeFourthsGeneralCouncil,
	EnsureRootOrTwoThirdsGeneralCouncil, EnsureRootOrTwoThirdsTechnicalCommittee, ExchangeRate,
	ExistentialDepositsTimesOneHundred, FinancialCouncilInstance, FinancialCouncilMembershipInstance, GasToWeight,
	GeneralCouncilInstance, GeneralCouncilMembershipInstance, HomaCouncilInstance, HomaCouncilMembershipInstance,
	MaxTipsOfPriority, OperationalFeeMultiplier, OperatorMembershipInstanceAcala, Price, ProxyType, RandomnessSource,
	Rate, Ratio, RuntimeBlockLength, RuntimeBlockWeights, TechnicalCommitteeInstance,
	TechnicalCommitteeMembershipInstance, TimeStampedPrice, TipPerWeightStep, ACA, AUSD, DOT, LCDOT, LDOT, TAP,
};
use xcm::v4::prelude::*;

mod authority;
mod benchmarking;
pub mod constants;
/// Weights for pallets used in the runtime.
mod weights;
pub mod xcm_config;

/// This runtime version.
#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("acala"),
	impl_name: create_runtime_str!("acala"),
	authoring_version: 1,
	spec_version: 2260,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 3,
	state_version: 0,
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
	pub const HomaPalletId: PalletId = PalletId(*b"aca/homa");
	pub const HonzonTreasuryPalletId: PalletId = PalletId(*b"aca/hztr");
	pub const HomaTreasuryPalletId: PalletId = PalletId(*b"aca/hmtr");
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
	pub const CollatorPotId: PalletId = PalletId(*b"aca/cpot");
	pub const NomineesElectionId: LockIdentifier = *b"aca/nome";
	// Treasury reserve
	pub const TreasuryReservePalletId: PalletId = PalletId(*b"aca/reve");
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
	// Vault all unrleased native token.
	pub UnreleasedNativeVaultAccountId: AccountId = PalletId(*b"aca/urls").into_account_truncating();
	// This Pallet is only used to payment fee pool, it's not added to whitelist by design.
	// because transaction payment pallet will ensure the accounts always have enough ED.
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	pub const LiquidCrowdloanPalletId: PalletId = PalletId(*b"aca/lqcl");
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
	]
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 1200; // mortal tx can be valid up to 4 hour after signing
	pub const Version: RuntimeVersion = VERSION;
	pub const SS58Prefix: u8 = 10; // Ss58AddressFormat::AcalaAccount
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
				| pallet_xcm::Call::limited_teleport_assets { .. }
				| pallet_xcm::Call::transfer_assets { .. }
				| pallet_xcm::Call::transfer_assets_using_type_and_then { .. } => {
					return false;
				}
				// user xcm calls
				pallet_xcm::Call::claim_assets { .. } => {
					return true;
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
	pub const SessionDuration: BlockNumber = 2 * HOURS; // used in SessionManagerConfig of genesis
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
	pub const CollatorKickThreshold: Permill = Permill::from_percent(75);
}

impl module_collator_selection::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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
	pub NativeTokenExistentialDeposit: Balance = 10 * cent(ACA);	// 0.1 ACA
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
}

parameter_types! {
	pub TransactionByteFee: Balance = millicent(ACA);
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
	pub ProposalBondMinimum: Balance = 10 * dollar(ACA);
	pub ProposalBondMaximum: Balance = 50 * dollar(ACA);
	pub const SpendPeriod: BlockNumber = 30 * DAYS;
	pub const Burn: Permill = Permill::from_percent(1);

	pub const TipCountdown: BlockNumber = 2 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(5);
	pub TipReportDepositBase: Balance = deposit(1, 0);
	pub BountyDepositBase: Balance = deposit(1, 0);
	pub const BountyDepositPayoutDelay: BlockNumber = 6 * DAYS;
	pub const BountyUpdatePeriod: BlockNumber = 35 * DAYS;
	pub const CuratorDepositMultiplier: Permill = Permill::from_percent(50);
	pub CuratorDepositMin: Balance = dollar(ACA);
	pub CuratorDepositMax: Balance = 100 * dollar(ACA);
	pub BountyValueMinimum: Balance = 5 * dollar(ACA);
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
	type Paymaster = PayFromAccount<Balances, AcalaTreasuryAccount>;
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
	pub MinimumDeposit: Balance = 1000 * dollar(ACA);
	pub const EnactmentPeriod: BlockNumber = 2 * DAYS;
	pub const VoteLockingPeriod: BlockNumber = 14 * DAYS;
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
	type WeightInfo = pallet_democracy::weights::SubstrateWeight<Runtime>;
	type MaxProposals = ConstU32<100>;
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

parameter_types! {
	pub const MinimumCount: u32 = 5;
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
	type MaxHasDispatchedSize = ConstU32<20>;
	type WeightInfo = ();
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

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		match currency_id {
			CurrencyId::Token(symbol) => match symbol {
				TokenSymbol::AUSD => 10 * cent(*currency_id),
				TokenSymbol::DOT => cent(*currency_id),
				TokenSymbol::LDOT => 5 * cent(*currency_id),
				TokenSymbol::TAP => dollar(*currency_id),

				TokenSymbol::KAR |
				TokenSymbol::KUSD |
				TokenSymbol::KSM |
				TokenSymbol::LKSM |
				TokenSymbol::BNC |
				TokenSymbol::PHA |
				TokenSymbol::VSKSM |
				TokenSymbol::ACA |
				TokenSymbol::KBTC |
				TokenSymbol::KINT |
				TokenSymbol::TAI => Balance::max_value() // unsupported
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

pub struct DustRemovalWhitelist;
impl Contains<AccountId> for DustRemovalWhitelist {
	fn contains(a: &AccountId) -> bool {
		get_all_module_accounts().contains(a)
	}
}

parameter_types! {
	pub AcalaTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = weights::orml_tokens::WeightInfo<Runtime>;
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime, AcalaTreasuryAccount>;
	type MaxLocks = MaxLocks;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = DustRemovalWhitelist;
}

parameter_type_with_key! {
	pub LiquidCrowdloanLeaseBlockNumber: |lease: Lease| -> Option<BlockNumber> {
		match lease {
			13 => Some(17_856_000),
			_ => None
		}
	};
}

parameter_type_with_key! {
	pub PricingPegged: |currency_id: CurrencyId| -> Option<CurrencyId> {
		match currency_id {
			// taiKSM
			CurrencyId::StableAssetPoolToken(0) => Some(DOT),
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
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
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
	type OnDust = module_currencies::TransferDust<Runtime, AcalaTreasuryAccount>;
}

parameter_types! {
	pub AcalaFoundationAccounts: Vec<AccountId> = vec![
		hex_literal::hex!["5336f96b54fa1832d517549bbffdfba2cae8983b8dcf65caff82d616014f5951"].into(),	// 22khtd8Zu9CpCY7DR4EPmmX66Aqsc91ShRAhehSWKGL7XDpL
		hex_literal::hex!["26adf1c3a5b73f8640404d59ccb81de3ede79965b140addc7d8c0ff8736b5c53"].into(),	// 21kK5T9tvL8nVdAAWizjtBgRbGcAs466iU6ZxeNWb7mFgg5i
		hex_literal::hex!["7e32626ae20238b3f2c63299bdc1eb4729c7aadc995ce2abaa4e42130209f5d5"].into(),	// 23j4ay2zBSgaSs18xstipmHBNi39W2Su9n8G89kWrz8eCe8F
		TreasuryPalletId::get().into_account_truncating(),
		TreasuryReservePalletId::get().into_account_truncating(),
	];
}

pub struct EnsureAcalaFoundation;
impl EnsureOrigin<RuntimeOrigin> for EnsureAcalaFoundation {
	type Success = AccountId;

	fn try_origin(o: RuntimeOrigin) -> Result<Self::Success, RuntimeOrigin> {
		Into::<Result<RawOrigin<AccountId>, RuntimeOrigin>>::into(o).and_then(|o| match o {
			RawOrigin::Signed(caller) => {
				if AcalaFoundationAccounts::get().contains(&caller) {
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
	type VestedTransferOrigin = EnsureAcalaFoundation;
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
	type MaxScheduledPerBlock = ConstU32<10>;
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
	pub const AuctionDurationSoftCap: BlockNumber = 24 * HOURS;
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
			frame_metadata_hash_extension::CheckMetadataHash::new(true),
			module_evm::SetEvmOrigin::<Runtime>::new(),
			module_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				log::warn!("Unable to create signed payload: {:?}", e);
			})
			.ok()?;
		let signature = raw_payload.using_encoded(|payload| C::sign(payload, public))?;
		let address = AccountIdLookup::unlookup(account);
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
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(150, 100);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: FractionalRate = FractionalRate::try_from(Rate::saturating_from_rational(8, 100))
		.expect("Rate is in range; qed");
	pub MinimumDebitValue: Balance = 50 * dollar(AUSD);
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::saturating_from_rational(10, 100);
	pub MaxLiquidationContractSlippage: Ratio = Ratio::saturating_from_rational(15, 100);
	pub SettleErc20EvmOrigin: AccountId = AccountId::from(hex_literal::hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff")); // `26fFquxSECczieT6xrgG9uvg7LaEc1vj5M6SmX5K6QYN6TGZ`
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
	pub DepositPerAuthorization: Balance = deposit(1, 64);
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
	type ShutdownOrigin = EnsureRoot<AccountId>;
	type WeightInfo = weights::module_emergency_shutdown::WeightInfo<Runtime>;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (3, 1000);	// 0.3%
	pub const ExtendedProvisioningBlocks: BlockNumber = 2 * DAYS;
	pub const TradingPathLimit: u32 = 4;
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
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![LCDOT],
		vec![DOT],
		vec![LDOT],
		vec![AUSD],
	];
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
	type UpdateOrigin = EnsureRootOrTwoThirdsGeneralCouncil;
	type WeightInfo = weights::module_transaction_pause::WeightInfo<Runtime>;
}

parameter_types! {
	pub const CustomFeeSurplus: Percent = Percent::from_percent(50);
	pub const AlternativeFeeSurplus: Percent = Percent::from_percent(25);
	pub DefaultFeeTokens: Vec<CurrencyId> = vec![AUSD, LCDOT, DOT, LDOT];
}

type NegativeImbalance = <Balances as PalletCurrency<AccountId>>::NegativeImbalance;
pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance>) {
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
	type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
	type Swap = AcalaSwap;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = module_prices::RealTimePriceProvider<Runtime>;
	type WeightInfo = weights::module_transaction_payment::WeightInfo<Runtime>;
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = AcalaTreasuryAccount;
	type UpdateOrigin = EnsureRootOrHalfGeneralCouncil;
	type CustomFeeSurplus = CustomFeeSurplus;
	type AlternativeFeeSurplus = AlternativeFeeSurplus;
	type DefaultFeeTokens = DefaultFeeTokens;
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
	pub CreateClassDeposit: Balance = 50 * dollar(ACA);
	pub CreateTokenDeposit: Balance = 20 * cent(ACA);
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
			ProxyType::StableAssetLiquidity | ProxyType::StableAssetSwap => false,
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
}

parameter_types! {
	pub const NewContractExtraBytes: u32 = 10_000;
	pub NetworkContractSource: H160 = H160::from_low_u64_be(0);
	pub DeveloperDeposit: Balance = 50 * dollar(ACA);
	pub PublicationFee: Balance = 10 * dollar(ACA);
	pub PrecompilesValue: AllPrecompiles<
		Runtime, module_transaction_pause::PausedPrecompileFilter<Runtime>, AcalaPrecompiles<Runtime>
	> = AllPrecompiles::<_, _, _>::acala();
}

#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct StorageDepositPerByte;
impl<I: From<Balance>> frame_support::traits::Get<I> for StorageDepositPerByte {
	fn get() -> I {
		// NOTE: ACA decimals is 12, convert to 18.
		// 30 * millicent(ACA) * 10^6
		I::from(300_000_000_000_000)
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

impl module_evm::Config for Runtime {
	type AddressMapping = EvmAddressMapping<Runtime>;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = TxFeePerGas;
	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType =
		AllPrecompiles<Self, module_transaction_pause::PausedPrecompileFilter<Self>, AcalaPrecompiles<Self>>;
	type PrecompilesValue = PrecompilesValue;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Runtime>;
	type NetworkContractOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type PublicationFee = PublicationFee;
	type TreasuryAccount = AcalaTreasuryAccount;
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

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub HomaTreasuryAccount: AccountId = HomaTreasuryPalletId::get().into_account_truncating();
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![
		0,  // 15sr8Dvq3AT3Z2Z1y8FnQ4VipekAHhmQnrkgzegUr1tNgbcn
	];
	pub MintThreshold: Balance = dollar(DOT);
	pub RedeemThreshold: Balance = 5 * dollar(LDOT);
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

pub fn create_x2_parachain_location(index: u16) -> Location {
	Location::new(
		1,
		AccountId32 {
			network: None,
			id: Utility::derivative_account_id(ParachainInfo::get().into_account_truncating(), index).into(),
		},
	)
}

pub struct SubAccountIndexLocationConvertor;
impl Convert<u16, Location> for SubAccountIndexLocationConvertor {
	fn convert(sub_account_index: u16) -> Location {
		create_x2_parachain_location(sub_account_index)
	}
}

parameter_types! {
	pub ParachainAccount: AccountId = ParachainInfo::get().into_account_truncating();
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
	type AccountIdToLocation = runtime_common::xcm_config::AccountIdToLocation;
}

impl orml_unknown_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
}

pub struct GetTotalFrozenStableCurrency;
impl frame_support::traits::Get<Balance> for GetTotalFrozenStableCurrency {
	fn get() -> Balance {
		let stable_currency_id = GetStableCurrencyId::get();
		let mut total_frozen_stable_currency = Balance::default();

		for (who, currency_id, locks) in orml_tokens::Locks::<Runtime>::iter() {
			if currency_id == stable_currency_id && !locks.is_empty() {
				let orml_tokens::AccountData::<Balance> { free, frozen, .. } =
					orml_tokens::Accounts::<Runtime>::get(who, currency_id);
				total_frozen_stable_currency = total_frozen_stable_currency.saturating_add(free.min(frozen));
			}
		}

		total_frozen_stable_currency
	}
}

impl orml_xcm::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type SovereignOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
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
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = RelaychainDataProvider<Runtime>;
	// Number of relay chain blocks produced with no parachain blocks finalized,
	// once this number is reached idle scheduler is disabled as block production is slow
	type DisableBlockThreshold = ConstU32<6>;
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
	orml_tokens::Mapper<AccountId, Currencies, ConvertBalanceHoma, Balance, GetLiquidCurrencyId>,
	Currencies,
>;

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

parameter_types!(
	pub const LiquidCrowdloanCurrencyId: CurrencyId = LCDOT;
);

impl module_liquid_crowdloan::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LiquidCrowdloanCurrencyId = LiquidCrowdloanCurrencyId;
	type RelayChainCurrencyId = GetStakingCurrencyId;
	type PalletId = LiquidCrowdloanPalletId;
	type GovernanceOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type WeightInfo = weights::module_liquid_crowdloan::WeightInfo<Runtime>;
}

parameter_types! {
	pub MinBond: Balance = 100 * dollar(ACA);
	pub const UnbondingPeriod: BlockNumber = 14 * DAYS;
	pub const EarningLockIdentifier: LockIdentifier = *b"aca/earn";
}

impl module_earning::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
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

		// Acala Core
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

		// Homa
		Homa: module_homa = 116,
		XcmInterface: module_xcm_interface = 117,
		HomaValidatorList: module_homa_validator_list = 118,
		NomineesElection: module_nominees_election = 119,

		// Acala Other
		Incentives: module_incentives = 120,
		NFT: module_nft = 121,
		AssetRegistry: module_asset_registry = 122,
		LiquidCrowdloan: module_liquid_crowdloan = 123,

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
/// The SignedExtension to the basic transaction logic.
pub type SignedExtra = (
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
		[module_emergency_shutdown, benchmarking::emergency_shutdown]
		[module_evm, benchmarking::evm]
		[module_homa, benchmarking::homa]
		[module_homa_validator_list, benchmarking::homa_validator_list]
		[module_honzon, benchmarking::honzon]
		[module_cdp_treasury, benchmarking::cdp_treasury]
		[module_collator_selection, benchmarking::collator_selection]
		[module_transaction_pause, benchmarking::transaction_pause]
		[module_transaction_payment, benchmarking::transaction_payment]
		[module_incentives, benchmarking::incentives]
		[module_prices, benchmarking::prices]
		[module_evm_accounts, benchmarking::evm_accounts]
		[module_currencies, benchmarking::currencies]
		[module_session_manager, benchmarking::session_manager]
		[orml_tokens, benchmarking::tokens]
		[orml_vesting, benchmarking::vesting]
		[orml_auction, benchmarking::auction]
		[orml_authority, benchmarking::authority]
		[nutsfinance_stable_asset, benchmarking::nutsfinance_stable_asset]
		[module_idle_scheduler, benchmarking::idle_scheduler]
		[module_aggregated_dex, benchmarking::aggregated_dex]
		[module_liquid_crowdloan, benchmarking::liquid_crowdloan]
		[module_nominees_election, benchmarking::nominees_election]
	);
	// frame_benchmarking::define_benchmarks!(
	// 	// XCM
	// 	[pallet_xcm, PalletXcmExtrinsicsBenchmark::<Runtime>]
	// // TODO: add oracle
	// );
}

sp_api::impl_runtime_apis! {
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
		fn query_fee_details(uxt: <Block as BlockT>::Extrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::FeeDetails<Balance> {
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
			use orml_traits::GetByKey;

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

cumulus_pallet_parachain_system::register_validate_block!(
	Runtime = Runtime,
	BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
);

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
		println!("{:?}", core::mem::size_of::<RuntimeCall>());
		assert!(
			core::mem::size_of::<RuntimeCall>() <= 280,
			"size of RuntimeCall is more than 280 bytes: some calls have too big arguments, use Box to \
			reduce the size of RuntimeCall.
			If the limit is too strong, maybe consider increasing the limit",
		);
	}
}
