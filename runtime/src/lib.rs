//! The Acala runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
// The `large_enum_variant` warning originates from `construct_runtime` macro.
#![allow(clippy::large_enum_variant)]
#![allow(clippy::unnecessary_mut_passed)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::Encode;
use hex_literal::hex;
use sp_api::impl_runtime_apis;
use sp_core::{
	crypto::KeyTypeId,
	u32_trait::{_1, _2, _3, _4},
	OpaqueMetadata,
};
use sp_runtime::traits::{
	BlakeTwo256, Block as BlockT, Convert, NumberFor, OpaqueKeys, SaturatedConversion, Saturating, StaticLookup,
};
use sp_runtime::{
	create_runtime_str,
	curve::PiecewiseLinear,
	generic, impl_opaque_keys,
	traits::AccountIdConversion,
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, FixedPointNumber, ModuleId,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;
use static_assertions::const_assert;

use frame_system::{EnsureOneOf, EnsureRoot};
use orml_currencies::{BasicCurrencyAdapter, Currency};
use pallet_grandpa::fg_primitives;
use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_session::historical as pallet_session_historical;
use pallet_transaction_payment::{Multiplier, TargetedFeeAdjustment};

pub use frame_support::{
	construct_runtime, debug, parameter_types,
	traits::{Contains, ContainsLengthBound, Filter, Get, KeyOwnerProofSystem, Randomness},
	weights::{
		constants::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight, WEIGHT_PER_SECOND},
		IdentityFee, Weight,
	},
	StorageValue,
};
pub use orml_oracle::AuthorityId as OracleId;
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Percent, Permill, Perquintill};

pub use constants::{currency::*, time::*};
pub use module_cdp_treasury::SurplusDebitAuctionConfig;
pub use types::*;

mod benchmarking;
mod constants;
mod types;

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("acala"),
	impl_name: create_runtime_str!("acala"),
	authoring_version: 1,
	spec_version: 504,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
};

/// The version infromation used to identify this runtime when compiled
/// natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't
/// need to know the specifics of the runtime. They can then be made to be
/// agnostic over specific formats of data like extrinsics, allowing for them to
/// continue syncing the network through upgrades to even the core
/// datastructures.
pub mod opaque {
	use super::*;
	pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;

	/// Opaque block header type.
	pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// Opaque block type.
	pub type Block = generic::Block<Header, UncheckedExtrinsic>;
	/// Opaque block identifier type.
	pub type BlockId = generic::BlockId<Block>;

	impl_opaque_keys! {
		pub struct SessionKeys {
			pub grandpa: Grandpa,
			pub babe: Babe,
		}
	}
}

// Module accounts of runtime
parameter_types! {
	pub const PalletTreasuryModuleId: ModuleId = ModuleId(*b"aca/trsy");
	pub const LoansModuleId: ModuleId = ModuleId(*b"aca/loan");
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
	pub const CDPTreasuryModuleId: ModuleId = ModuleId(*b"aca/cdpt");
	pub const StakingPoolModuleId: ModuleId = ModuleId(*b"aca/stkp");
	pub const HomaTreasuryModuleId: ModuleId = ModuleId(*b"aca/hmtr");
}

pub fn get_all_module_accounts() -> Vec<AccountId> {
	vec![
		PalletTreasuryModuleId::get().into_account(),
		LoansModuleId::get().into_account(),
		DEXModuleId::get().into_account(),
		CDPTreasuryModuleId::get().into_account(),
		StakingPoolModuleId::get().into_account(),
		HomaTreasuryModuleId::get().into_account(),
	]
}

parameter_types! {
	pub const BlockHashCount: BlockNumber = 900; // mortal tx can be valid up to 1 hour after signing
	/// We allow for 2 seconds of compute with a 4 second average block time.
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	/// Assume 10% of weight for average on_initialize calls.
	pub MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
		.saturating_sub(Perbill::from_percent(10)) * MaximumBlockWeight::get();
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}

impl frame_system::Trait for Runtime {
	type AccountId = AccountId;
	type Call = Call;
	type Lookup = Indices;
	type Index = Index;
	type BlockNumber = BlockNumber;
	type Hash = Hash;
	type Hashing = BlakeTwo256;
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	type Event = Event;
	type Origin = Origin;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = Version;
	type ModuleToIndex = ModuleToIndex;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = module_accounts::Module<Runtime>;
	type DbWeight = RocksDbWeight;
	type BlockExecutionWeight = BlockExecutionWeight;
	type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
	type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}

impl pallet_babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::Proof;

	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, pallet_babe::AuthorityId)>>::IdentificationTuple;

	type HandleEquivocation = pallet_babe::EquivocationHandler<Self::KeyOwnerIdentification, ()>; // Offences
}

impl pallet_grandpa::Trait for Runtime {
	type Event = Event;
	type Call = Call;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::IdentificationTuple;

	type HandleEquivocation = pallet_grandpa::EquivocationHandler<Self::KeyOwnerIdentification, ()>; // Offences
}

parameter_types! {
	pub const IndexDeposit: Balance = DOLLARS;
}

impl pallet_indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type Event = Event;
	type Currency = Balances;
	type Deposit = IndexDeposit;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Trait for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const UncleGenerations: BlockNumber = 5;
}

impl pallet_authorship::Trait for Runtime {
	type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Babe>;
	type UncleGenerations = UncleGenerations;
	type FilterUncle = ();
	type EventHandler = (Staking, ()); // ImOnline
}

parameter_types! {
	pub const AcaExistentialDeposit: Balance = 0;
}

// `module_accounts` handles account opening/reaping, `ExistentialDeposit` in
// `pallet_balances` must be 0
const_assert!(AcaExistentialDeposit::get() == 0);

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = AcaExistentialDeposit;
	type AccountStore = module_accounts::Module<Runtime>;
	type WeightInfo = ();
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
	pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(25);
	pub AdjustmentVariable: Multiplier = Multiplier::saturating_from_rational(1, 100_000);
	pub MinimumMultiplier: Multiplier = Multiplier::saturating_from_rational(1, 1_000_000_000u128);
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = Balances;
	type OnTransactionPayment = PalletTreasury;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = fee::WeightToFee;
	type FeeMultiplierUpdate = TargetedFeeAdjustment<Self, TargetBlockFullness, AdjustmentVariable, MinimumMultiplier>;
}

impl pallet_sudo::Trait for Runtime {
	type Event = Event;
	type Call = Call;
}

parameter_types! {
	pub const GeneralCouncilMotionDuration: BlockNumber = 0;
	pub const GeneralCouncilMaxProposals: u32 = 100;
}

type GeneralCouncilInstance = pallet_collective::Instance1;
impl pallet_collective::Trait<GeneralCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = GeneralCouncilMotionDuration;
	type MaxProposals = GeneralCouncilMaxProposals;
	type WeightInfo = ();
}

type EnsureRootOrThreeFourthsGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>,
>;

type GeneralCouncilMembershipInstance = pallet_membership::Instance1;
impl pallet_membership::Trait<GeneralCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type RemoveOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type SwapOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type ResetOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type PrimeOrigin = EnsureRootOrThreeFourthsGeneralCouncil;
	type MembershipInitialized = GeneralCouncil;
	type MembershipChanged = GeneralCouncil;
}

parameter_types! {
	pub const HonzonCouncilMotionDuration: BlockNumber = 0;
	pub const HonzonCouncilMaxProposals: u32 = 100;
}

type HonzonCouncilInstance = pallet_collective::Instance2;
impl pallet_collective::Trait<HonzonCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = HonzonCouncilMotionDuration;
	type MaxProposals = HonzonCouncilMaxProposals;
	type WeightInfo = ();
}

type EnsureRootOrHalfGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>,
>;

type HonzonCouncilMembershipInstance = pallet_membership::Instance2;
impl pallet_membership::Trait<HonzonCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = EnsureRootOrHalfGeneralCouncil;
	type RemoveOrigin = EnsureRootOrHalfGeneralCouncil;
	type SwapOrigin = EnsureRootOrHalfGeneralCouncil;
	type ResetOrigin = EnsureRootOrHalfGeneralCouncil;
	type PrimeOrigin = EnsureRootOrHalfGeneralCouncil;
	type MembershipInitialized = HonzonCouncil;
	type MembershipChanged = HonzonCouncil;
}

parameter_types! {
	pub const HomaCouncilMotionDuration: BlockNumber = 0;
	pub const HomaCouncilMaxProposals: u32 = 100;
}

type HomaCouncilInstance = pallet_collective::Instance3;
impl pallet_collective::Trait<HomaCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = HomaCouncilMotionDuration;
	type MaxProposals = HomaCouncilMaxProposals;
	type WeightInfo = ();
}

type HomaCouncilMembershipInstance = pallet_membership::Instance3;
impl pallet_membership::Trait<HomaCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = EnsureRootOrHalfGeneralCouncil;
	type RemoveOrigin = EnsureRootOrHalfGeneralCouncil;
	type SwapOrigin = EnsureRootOrHalfGeneralCouncil;
	type ResetOrigin = EnsureRootOrHalfGeneralCouncil;
	type PrimeOrigin = EnsureRootOrHalfGeneralCouncil;
	type MembershipInitialized = HomaCouncil;
	type MembershipChanged = HomaCouncil;
}

parameter_types! {
	pub const TechnicalCouncilMotionDuration: BlockNumber = 0;
	pub const TechnicalCouncilMaxProposals: u32 = 100;
}

type TechnicalCouncilInstance = pallet_collective::Instance4;
impl pallet_collective::Trait<TechnicalCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
	type MotionDuration = TechnicalCouncilMotionDuration;
	type MaxProposals = TechnicalCouncilMaxProposals;
	type WeightInfo = ();
}

type TechnicalCouncilMembershipInstance = pallet_membership::Instance4;
impl pallet_membership::Trait<TechnicalCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = EnsureRootOrHalfGeneralCouncil;
	type RemoveOrigin = EnsureRootOrHalfGeneralCouncil;
	type SwapOrigin = EnsureRootOrHalfGeneralCouncil;
	type ResetOrigin = EnsureRootOrHalfGeneralCouncil;
	type PrimeOrigin = EnsureRootOrHalfGeneralCouncil;
	type MembershipInitialized = TechnicalCouncil;
	type MembershipChanged = TechnicalCouncil;
}

type EnsureRootOrOneThirdGeneralCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>,
>;

type OperatorMembershipInstance = pallet_membership::Instance5;
impl pallet_membership::Trait<OperatorMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = EnsureRootOrOneThirdGeneralCouncil;
	type RemoveOrigin = EnsureRootOrOneThirdGeneralCouncil;
	type SwapOrigin = EnsureRootOrOneThirdGeneralCouncil;
	type ResetOrigin = EnsureRootOrOneThirdGeneralCouncil;
	type PrimeOrigin = EnsureRootOrOneThirdGeneralCouncil;
	type MembershipInitialized = Oracle;
	type MembershipChanged = Oracle;
}

impl pallet_utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}

parameter_types! {
	pub const MultisigDepositBase: Balance = 500 * MILLICENTS;
	pub const MultisigDepositFactor: Balance = 100 * MILLICENTS;
	pub const MaxSignatories: u16 = 100;
}

impl pallet_multisig::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type DepositBase = MultisigDepositBase;
	type DepositFactor = MultisigDepositFactor;
	type MaxSignatories = MaxSignatories;
	type WeightInfo = ();
}

pub struct GeneralCouncilProvider;
impl Contains<AccountId> for GeneralCouncilProvider {
	fn contains(who: &AccountId) -> bool {
		GeneralCouncil::is_member(who)
	}

	fn sorted_members() -> Vec<AccountId> {
		GeneralCouncil::members()
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
	pub const ProposalBondMinimum: Balance = DOLLARS;
	pub const SpendPeriod: BlockNumber = DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TipCountdown: BlockNumber = DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(10);
	pub const TipReportDepositBase: Balance = DOLLARS;
	pub const TipReportDepositPerByte: Balance = CENTS;
}

impl pallet_treasury::Trait for Runtime {
	type ModuleId = PalletTreasuryModuleId;
	type Currency = Balances;
	type ApproveOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureMembers<_4, AccountId, GeneralCouncilInstance>,
	>;
	type RejectOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureMembers<_2, AccountId, GeneralCouncilInstance>,
	>;
	type Tippers = GeneralCouncilProvider;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type TipReportDepositPerByte = TipReportDepositPerByte;
	type Event = Event;
	type ProposalRejection = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type BurnDestination = ();
	type WeightInfo = ();
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl pallet_session::Trait for Runtime {
	type Event = Event;
	type ValidatorId = <Self as frame_system::Trait>::AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type NextSessionRotation = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type WeightInfo = ();
}

impl pallet_session::historical::Trait for Runtime {
	type FullIdentification = pallet_staking::Exposure<AccountId, Balance>;
	type FullIdentificationOf = pallet_staking::ExposureOf<Runtime>;
}

pallet_staking_reward_curve::build! {
	const REWARD_CURVE: PiecewiseLinear<'static> = curve!(
		min_inflation: 0_025_000,
		max_inflation: 0_100_000,
		ideal_stake: 0_500_000,
		falloff: 0_050_000,
		max_piece_count: 40,
		test_precision: 0_005_000,
	);
}

/// Struct that handles the conversion of Balance -> `u64`. This is used for
/// staking's election calculation.
pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
	fn factor() -> Balance {
		(Balances::total_issuance() / u64::max_value() as Balance).max(1)
	}
}

impl Convert<Balance, u64> for CurrencyToVoteHandler {
	fn convert(x: Balance) -> u64 {
		(x / Self::factor()) as u64
	}
}

impl Convert<u128, Balance> for CurrencyToVoteHandler {
	fn convert(x: u128) -> Balance {
		x * Self::factor()
	}
}

parameter_types! {
	pub const SessionsPerEra: sp_staking::SessionIndex = 3; // 3 hours
	pub const BondingDuration: pallet_staking::EraIndex = 4; // 12 hours
	pub const SlashDeferDuration: pallet_staking::EraIndex = 2; // 6 hours
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
	pub const MaxNominatorRewardedPerValidator: u32 = 64;
	pub const ElectionLookahead: BlockNumber = EPOCH_DURATION_IN_BLOCKS / 4;
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
	pub const MaxIterations: u32 = 5;
	// 0.05%. The higher the value, the more strict solution acceptance becomes.
	pub MinSolutionScoreBump: Perbill = Perbill::from_rational_approximation(5u32, 10_000);
}

impl pallet_staking::Trait for Runtime {
	type Currency = Balances;
	type UnixTime = Timestamp;
	type CurrencyToVote = CurrencyToVoteHandler;
	type RewardRemainder = PalletTreasury;
	type Event = Event;
	type Slash = PalletTreasury; // send the slashed funds to the pallet treasury.
	type Reward = (); // rewards are minted from the void
	type SessionsPerEra = SessionsPerEra;
	type BondingDuration = BondingDuration;
	type SlashDeferDuration = SlashDeferDuration;
	/// A super-majority of the council can cancel the slash.
	type SlashCancelOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, GeneralCouncilInstance>,
	>;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
	type NextNewSession = Session;
	type ElectionLookahead = ElectionLookahead;
	type Call = Call;
	type MaxIterations = MaxIterations;
	type MinSolutionScoreBump = MinSolutionScoreBump;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type UnsignedPriority = StakingUnsignedPriority;
	type WeightInfo = ();
}

parameter_types! {
	pub const ConfigDepositBase: Balance = 10 * CENTS;
	pub const FriendDepositFactor: Balance = CENTS;
	pub const MaxFriends: u16 = 9;
	pub const RecoveryDeposit: Balance = 10 * CENTS;
}

impl pallet_recovery::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ConfigDepositBase = ConfigDepositBase;
	type FriendDepositFactor = FriendDepositFactor;
	type MaxFriends = MaxFriends;
	type RecoveryDeposit = RecoveryDeposit;
}

impl orml_auction::Trait for Runtime {
	type Event = Event;
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManager;
}

parameter_types! {
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: Moment = 1000 * 60 * 60; // 60 mins
	pub const OracleUnsignedPriority: TransactionPriority = TransactionPriority::max_value() - 10000;
}

impl orml_oracle::Trait for Runtime {
	type Event = Event;
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Runtime, MinimumCount, ExpiresIn>;
	type Time = Timestamp;
	type OracleKey = CurrencyId;
	type OracleValue = Price;
	type UnsignedPriority = OracleUnsignedPriority;
	type AuthorityId = orml_oracle::AuthorityId;
}

pub type TimeStampedPrice = orml_oracle::TimestampedValueOf<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = module_accounts::Module<Runtime>;
}

parameter_types! {
	pub StableCurrencyFixedPrice: Price = Price::saturating_from_rational(1, 1);
}

impl module_prices::Trait for Runtime {
	type Event = Event;
	type Source = Oracle;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureRootOrHalfGeneralCouncil;
	type LiquidStakingExchangeRateProvider = LiquidStakingExchangeRateProvider;
}

pub struct LiquidStakingExchangeRateProvider;
impl module_support::ExchangeRateProvider for LiquidStakingExchangeRateProvider {
	fn get_exchange_rate() -> ExchangeRate {
		StakingPool::liquid_exchange_rate()
	}
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::ACA;
	pub const GetStableCurrencyId: CurrencyId = CurrencyId::AUSD;
}

impl orml_currencies::Trait for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Balances, Balance, Balance, Amount, BlockNumber>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const MinVestedTransfer: Balance = 100 * DOLLARS;
}

impl orml_vesting::Trait for Runtime {
	type Event = Event;
	type Currency = pallet_balances::Module<Runtime>;
	type MinVestedTransfer = MinVestedTransfer;
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(10) * MaximumBlockWeight::get();
}

impl pallet_scheduler::Trait for Runtime {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const UpdateFrequency: BlockNumber = 10;
}

impl orml_gradually_update::Trait for Runtime {
	type Event = Event;
	type UpdateFrequency = UpdateFrequency;
	type DispatchOrigin = EnsureRoot<AccountId>;
}

parameter_types! {
	pub MinimumIncrementSize: Rate = Rate::saturating_from_rational(2, 100);
	pub const AuctionTimeToClose: BlockNumber = 15 * MINUTES;
	pub const AuctionDurationSoftCap: BlockNumber = 2 * HOURS;
	pub GetAmountAdjustment: Rate = Rate::saturating_from_rational(20, 100);
	pub const AuctionManagerUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

impl module_auction_manager::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type Auction = Auction;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetStableCurrencyId = GetStableCurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type CDPTreasury = CdpTreasury;
	type GetAmountAdjustment = GetAmountAdjustment;
	type DEX = Dex;
	type PriceSource = Prices;
	type UnsignedPriority = AuctionManagerUnsignedPriority;
	type EmergencyShutdown = EmergencyShutdown;
}

impl module_loans::Trait for Runtime {
	type Event = Event;
	type Convert = module_cdp_engine::DebitExchangeRateConvertor<Runtime>;
	type Currency = Currencies;
	type RiskManager = CdpEngine;
	type CDPTreasury = CdpTreasury;
	type ModuleId = LoansModuleId;
}

impl<LocalCall> frame_system::offchain::CreateSignedTransaction<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	fn create_transaction<C: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>>(
		call: Call,
		public: <Signature as sp_runtime::traits::Verify>::Signer,
		account: AccountId,
		nonce: Index,
	) -> Option<(
		Call,
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
			frame_system::CheckSpecVersion::<Runtime>::new(),
			frame_system::CheckTxVersion::<Runtime>::new(),
			frame_system::CheckGenesis::<Runtime>::new(),
			frame_system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			frame_system::CheckNonce::<Runtime>::from(nonce),
			frame_system::CheckWeight::<Runtime>::new(),
			module_accounts::ChargeTransactionPayment::<Runtime>::from(tip),
		);
		let raw_payload = SignedPayload::new(call, extra)
			.map_err(|e| {
				debug::warn!("Unable to create signed payload: {:?}", e);
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
	Call: From<C>,
{
	type OverarchingCall = Call;
	type Extrinsic = UncheckedExtrinsic;
}

parameter_types! {
	pub CollateralCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::XBTC, CurrencyId::LDOT, CurrencyId::RENBTC];
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(110, 100);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: Rate = Rate::saturating_from_rational(5, 100);
	pub const MinimumDebitValue: Balance = DOLLARS;
	pub MaxSlippageSwapWithDEX: Ratio = Ratio::saturating_from_rational(5, 100);
	pub const CdpEngineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

type EnsureRootOrHalfHonzonCouncil = EnsureOneOf<
	AccountId,
	EnsureRoot<AccountId>,
	pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, HonzonCouncilInstance>,
>;

impl module_cdp_engine::Trait for Runtime {
	type Event = Event;
	type PriceSource = Prices;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = MinimumDebitValue;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CdpTreasury;
	type UpdateOrigin = EnsureRootOrHalfHonzonCouncil;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
	type DEX = Dex;
	type UnsignedPriority = CdpEngineUnsignedPriority;
	type EmergencyShutdown = EmergencyShutdown;
}

impl module_honzon::Trait for Runtime {
	type Event = Event;
}

impl module_emergency_shutdown::Trait for Runtime {
	type Event = Event;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type PriceSource = Prices;
	type CDPTreasury = CdpTreasury;
	type AuctionManagerHandler = AuctionManager;
	type ShutdownOrigin = EnsureOneOf<
		AccountId,
		EnsureRoot<AccountId>,
		pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>,
	>;
}

parameter_types! {
	pub GetExchangeFee: Rate = Rate::saturating_from_rational(1, 1000);
	pub EnabledCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::XBTC, CurrencyId::LDOT, CurrencyId::ACA, CurrencyId::RENBTC];
}

impl module_dex::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type Share = Share;
	type EnabledCurrencyIds = EnabledCurrencyIds;
	type GetBaseCurrencyId = GetStableCurrencyId;
	type GetExchangeFee = GetExchangeFee;
	type CDPTreasury = CdpTreasury;
	type UpdateOrigin = EnsureRootOrHalfHonzonCouncil;
	type ModuleId = DEXModuleId;
	type EmergencyShutdown = EmergencyShutdown;
}

parameter_types! {
	pub const MaxAuctionsCount: u32 = 100;
}

impl module_cdp_treasury::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManager;
	type UpdateOrigin = EnsureRootOrHalfHonzonCouncil;
	type DEX = Dex;
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = CDPTreasuryModuleId;
	type EmergencyShutdown = EmergencyShutdown;
}

parameter_types! {
	pub const FreeTransferCount: u8 = 3;
	pub const FreeTransferPeriod: BlockNumber = DAYS;
	pub const FreeTransferDeposit: Balance = DOLLARS;
	// All currency types except for native currency, Sort by fee charge order
	pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::AUSD, CurrencyId::LDOT, CurrencyId::DOT, CurrencyId::XBTC, CurrencyId::RENBTC];
	pub const NewAccountDeposit: Balance = 100 * MILLICENTS;
}

impl module_accounts::Trait for Runtime {
	type FreeTransferCount = FreeTransferCount;
	type FreeTransferPeriod = FreeTransferPeriod;
	type FreeTransferDeposit = FreeTransferDeposit;
	type Time = Timestamp;
	type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
	type NativeCurrencyId = GetNativeCurrencyId;
	type Currency = Currencies;
	type DEX = Dex;
	type OnCreatedAccount = frame_system::CallOnCreatedAccount<Runtime>;
	type KillAccount = frame_system::CallKillAccount<Runtime>;
	type NewAccountDeposit = NewAccountDeposit;
	type TreasuryModuleId = PalletTreasuryModuleId;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
}

impl module_airdrop::Trait for Runtime {
	type Event = Event;
}

parameter_types! {
	pub const PolkadotBondingDuration: EraIndex = 7;
	pub const EraLength: BlockNumber = DAYS;
}

impl module_polkadot_bridge::Trait for Runtime {
	type Event = Event;
	type DOTCurrency = Currency<Runtime, GetStakingCurrencyId>;
	type OnNewEra = (NomineesElection, StakingPool);
	type BondingDuration = PolkadotBondingDuration;
	type EraLength = EraLength;
	type PolkadotAccountId = AccountId;
}

parameter_types! {
	pub const GetLiquidCurrencyId: CurrencyId = CurrencyId::LDOT;
	pub const GetStakingCurrencyId: CurrencyId = CurrencyId::DOT;
	pub MaxBondRatio: Ratio = Ratio::saturating_from_rational(95, 100); // 95%
	pub MinBondRatio: Ratio = Ratio::saturating_from_rational(80, 100); // 80%
	pub MaxClaimFee: Rate = Rate::saturating_from_rational(5, 100); // 5%
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 100);	// 1 : 10
	pub ClaimFeeReturnRatio: Ratio = Ratio::saturating_from_rational(98, 100); // 98%
}

impl module_staking_pool::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type StakingCurrencyId = GetStakingCurrencyId;
	type LiquidCurrencyId = GetLiquidCurrencyId;
	type Nominees = NomineesElection;
	type OnCommission = ();
	type Bridge = PolkadotBridge;
	type MaxBondRatio = MaxBondRatio;
	type MinBondRatio = MinBondRatio;
	type MaxClaimFee = MaxClaimFee;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ClaimFeeReturnRatio = ClaimFeeReturnRatio;
	type ModuleId = StakingPoolModuleId;
}

impl module_homa::Trait for Runtime {
	type Homa = StakingPool;
}

parameter_types! {
	pub const MinCouncilBondThreshold: Balance = DOLLARS;
	pub const NominateesCount: usize = 7;
	pub const MaxUnlockingChunks: usize = 7;
	pub const NomineesElectionBondingDuration: EraIndex = 7;
}

impl module_nominees_election::Trait for Runtime {
	type Currency = Currency<Runtime, GetLiquidCurrencyId>;
	type PolkadotAccountId = AccountId;
	type MinBondThreshold = MinCouncilBondThreshold;
	type BondingDuration = NomineesElectionBondingDuration;
	type NominateesCount = NominateesCount;
	type MaxUnlockingChunks = MaxUnlockingChunks;
}

impl module_homa_treasury::Trait for Runtime {
	type Currency = Currencies;
	type Homa = StakingPool;
	type StakingCurrencyId = GetStakingCurrencyId;
	type ModuleId = HomaTreasuryModuleId;
}

parameter_types! {
	pub const RENBTCCurrencyId: CurrencyId = CurrencyId::RENBTC;
	pub const RenVmPublickKey: [u8; 20] = hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"];
	pub const RENBTCIdentifier: [u8; 32] = hex!["0000000000000000000000000a9add98c076448cbcfacf5e457da12ddbef4a8f"];
	pub const RenvmBridgeUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 3;
}

impl ecosystem_renvm_bridge::Trait for Runtime {
	type Event = Event;
	type Currency = Currency<Runtime, RENBTCCurrencyId>;
	type PublicKey = RenVmPublickKey;
	type CurrencyIdentifier = RENBTCIdentifier;
	type UnsignedPriority = RenvmBridgeUnsignedPriority;
}

#[allow(clippy::large_enum_variant)]
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// srml modules
		System: frame_system::{Module, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Authorship: pallet_authorship::{Module, Call, Storage, Inherent},
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent, ValidateUnsigned},
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event, ValidateUnsigned},
		Indices: pallet_indices::{Module, Call, Storage, Config<T>, Event<T>},
		Balances: pallet_balances::{Module, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Module, Storage},
		Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
		Utility: pallet_utility::{Module, Call, Event},
		Multisig: pallet_multisig::{Module, Call, Storage, Event<T>},
		PalletTreasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
		Staking: pallet_staking::{Module, Call, Config<T>, Storage, Event<T>},
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Recovery: pallet_recovery::{Module, Call, Storage, Event<T>},
		Historical: pallet_session_historical::{Module},

		// governance
		GeneralCouncil: pallet_collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		GeneralCouncilMembership: pallet_membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>},
		HonzonCouncil: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		HonzonCouncilMembership: pallet_membership::<Instance2>::{Module, Call, Storage, Event<T>, Config<T>},
		HomaCouncil: pallet_collective::<Instance3>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		HomaCouncilMembership: pallet_membership::<Instance3>::{Module, Call, Storage, Event<T>, Config<T>},
		TechnicalCouncil: pallet_collective::<Instance4>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		TechnicalCouncilMembership: pallet_membership::<Instance4>::{Module, Call, Storage, Event<T>, Config<T>},
		// oracle
		Oracle: orml_oracle::{Module, Storage, Call, Config<T>, Event<T>, ValidateUnsigned},
		// OperatorMembership must be placed after Oracle or else will have race condition on initialization
		OperatorMembership: pallet_membership::<Instance5>::{Module, Call, Storage, Event<T>, Config<T>},

		Currencies: orml_currencies::{Module, Call, Event<T>},
		Tokens: orml_tokens::{Module, Storage, Event<T>, Config<T>},
		Vesting: orml_vesting::{Module, Storage, Call, Event<T>, Config<T>},
		Scheduler: pallet_scheduler::{Module, Call, Storage, Event<T>},
		GraduallyUpdate: orml_gradually_update::{Module, Storage, Call, Event<T>},
		Auction: orml_auction::{Module, Storage, Call, Event<T>},

		// acala modules
		Prices: module_prices::{Module, Storage, Call, Event},
		AuctionManager: module_auction_manager::{Module, Storage, Call, Event<T>, ValidateUnsigned},
		Loans: module_loans::{Module, Storage, Call, Event<T>},
		Honzon: module_honzon::{Module, Storage, Call, Event<T>},
		Dex: module_dex::{Module, Storage, Call, Config, Event<T>},
		CdpTreasury: module_cdp_treasury::{Module, Storage, Call, Config, Event},
		CdpEngine: module_cdp_engine::{Module, Storage, Call, Event<T>, Config, ValidateUnsigned},
		EmergencyShutdown: module_emergency_shutdown::{Module, Storage, Call, Event<T>},
		Accounts: module_accounts::{Module, Call, Storage},
		AirDrop: module_airdrop::{Module, Call, Storage, Event<T>, Config<T>},
		Homa: module_homa::{Module, Call},
		NomineesElection: module_nominees_election::{Module, Call, Storage},
		StakingPool: module_staking_pool::{Module, Call, Storage, Event<T>},
		PolkadotBridge: module_polkadot_bridge::{Module, Call, Storage, Event<T>, Config},
		HomaTreasury: module_homa_treasury::{Module},

		// ecosystem modules
		RenVmBridge: ecosystem_renvm_bridge::{Module, Call, Storage, Event<T>, ValidateUnsigned},
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
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
	frame_system::CheckSpecVersion<Runtime>,
	frame_system::CheckTxVersion<Runtime>,
	frame_system::CheckGenesis<Runtime>,
	frame_system::CheckEra<Runtime>,
	frame_system::CheckNonce<Runtime>,
	frame_system::CheckWeight<Runtime>,
	module_accounts::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive =
	frame_executive::Executive<Runtime, Block, frame_system::ChainContext<Runtime>, Runtime, AllModules>;

impl_runtime_apis! {
	impl sp_api::Core<Block> for Runtime {
		fn version() -> RuntimeVersion {
			VERSION
		}

		fn execute_block(block: Block) {
			Executive::execute_block(block)
		}

		fn initialize_block(header: &<Block as BlockT>::Header) {
			Executive::initialize_block(header)
		}
	}

	impl sp_api::Metadata<Block> for Runtime {
		fn metadata() -> OpaqueMetadata {
			Runtime::metadata().into()
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

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(
			source: TransactionSource,
			tx: <Block as BlockT>::Extrinsic,
		) -> TransactionValidity {
			Executive::validate_transaction(source, tx)
		}
	}

	impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeGenesisConfiguration {
			sp_consensus_babe::BabeGenesisConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				allowed_slots: sp_consensus_babe::AllowedSlots::PrimaryAndSecondaryPlainSlots,
			}
		}

		fn current_epoch_start() -> sp_consensus_babe::SlotNumber {
			Babe::current_epoch_start()
		}

		fn generate_key_ownership_proof(
			_slot_number: sp_consensus_babe::SlotNumber,
			authority_id: sp_consensus_babe::AuthorityId,
			) -> Option<sp_consensus_babe::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((sp_consensus_babe::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(sp_consensus_babe::OpaqueKeyOwnershipProof::new)
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: sp_consensus_babe::EquivocationProof<<Block as BlockT>::Header>,
			key_owner_proof: sp_consensus_babe::OpaqueKeyOwnershipProof,
			) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Babe::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
				)
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}

		fn submit_report_equivocation_unsigned_extrinsic(
			equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Grandpa::submit_unsigned_equivocation_report(
				equivocation_proof,
				key_owner_proof,
			)
		}

		fn generate_key_ownership_proof(
			_set_id: fg_primitives::SetId,
			authority_id: GrandpaId,
		) -> Option<fg_primitives::OpaqueKeyOwnershipProof> {
			use codec::Encode;

			Historical::prove((fg_primitives::KEY_TYPE, authority_id))
				.map(|p| p.encode())
				.map(fg_primitives::OpaqueKeyOwnershipProof::new)
		}
	}

	impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
		fn account_nonce(account: AccountId) -> Index {
			System::account_nonce(account)
		}
	}

	impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<
		Block,
		Balance,
		UncheckedExtrinsic,
	> for Runtime {
		fn query_info(uxt: UncheckedExtrinsic, len: u32) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
			TransactionPayment::query_info(uxt, len)
		}
	}

	impl orml_oracle_rpc_runtime_api::OracleApi<
		Block,
		CurrencyId,
		TimeStampedPrice,
	> for Runtime {
		fn get_value(key: CurrencyId) -> Option<TimeStampedPrice> {
			Oracle::get_no_op(&key)
		}

		fn get_all_values() -> Vec<(CurrencyId, Option<TimeStampedPrice>)> {
			Oracle::get_all_values()
		}
	}

	impl module_dex_rpc_runtime_api::DexApi<
		Block,
		CurrencyId,
		Balance,
	> for Runtime {
		fn get_supply_amount(
			supply_currency_id: CurrencyId,
			target_currency_id: CurrencyId,
			target_currency_amount: Balance,
		) -> module_dex_rpc_runtime_api::BalanceInfo<Balance> {
			module_dex_rpc_runtime_api::BalanceInfo{
				amount: Dex::get_supply_amount_needed(supply_currency_id, target_currency_id, target_currency_amount)
			}
		}

		fn get_target_amount(
			supply_currency_id: CurrencyId,
			target_currency_id: CurrencyId,
			supply_currency_amount: Balance,
		) -> module_dex_rpc_runtime_api::BalanceInfo<Balance> {
			module_dex_rpc_runtime_api::BalanceInfo{
				amount: Dex::get_target_amount_available(supply_currency_id, target_currency_id, supply_currency_amount)
			}
		}
	}

	impl module_staking_pool_rpc_runtime_api::StakingPoolApi<
		Block,
		AccountId,
		Balance,
	> for Runtime {
		fn get_available_unbonded(account: AccountId) -> module_staking_pool_rpc_runtime_api::BalanceInfo<Balance> {
			module_staking_pool_rpc_runtime_api::BalanceInfo {
				amount: StakingPool::get_available_unbonded(&account)
			}
		}

		fn get_liquid_staking_exchange_rate() -> ExchangeRate {
			StakingPool::liquid_exchange_rate()
		}
	}

	// benchmarks for acala modules
	#[cfg(feature = "runtime-benchmarks")]
	impl frame_benchmarking::Benchmark<Block> for Runtime {
		fn dispatch_benchmark(
			pallet: Vec<u8>,
			benchmark: Vec<u8>,
			lowest_range_values: Vec<u32>,
			highest_range_values: Vec<u32>,
			steps: Vec<u32>,
			repeat: u32,
		) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, sp_runtime::RuntimeString> {
			use frame_benchmarking::{Benchmarking, BenchmarkBatch, add_benchmark};
			use orml_benchmarking::{add_benchmark as orml_add_benchmark};

			use module_honzon_benchmarking::Module as HonzonBench;
			use module_cdp_engine_benchmarking::Module as CdpEngineBench;
			use module_emergency_shutdown_benchmarking::Module as EmergencyShutdownBench;
			use module_auction_manager_benchmarking::Module as AuctionManagerBench;

			impl module_honzon_benchmarking::Trait for Runtime {}
			impl module_cdp_engine_benchmarking::Trait for Runtime {}
			impl module_emergency_shutdown_benchmarking::Trait for Runtime {}
			impl module_auction_manager_benchmarking::Trait for Runtime {}

			let whitelist: Vec<Vec<u8>> = vec![
				// Block Number
				// frame_system::Number::<Runtime>::hashed_key().to_vec(),
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec(),
				// Total Issuance
				hex_literal::hex!("c2261276cc9d1f8598ea4b6a74b15c2f57c875e4cff74148e4628f264b974c80").to_vec(),
				// Execution Phase
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7ff553b5a9862a516939d82b3d3d8661a").to_vec(),
				// Event Count
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef70a98fdbe9ce6c55837576c60c7af3850").to_vec(),
				// System Events
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7").to_vec(),
				// Caller 0 Account
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da946c154ffd9992e395af90b5b13cc6f295c77033fce8a9045824a6690bbf99c6db269502f0a8d1d2a008542d5690a0749").to_vec(),
				// Treasury Account
				hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef7b99d880ec681799c0cf30e8886371da95ecffd7b6c0f78751baa9d281e0bfa3a6d6f646c70792f74727372790000000000000000000000000000000000000000").to_vec(),
			];
			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&pallet, &benchmark, &lowest_range_values, &highest_range_values, &steps, repeat, &whitelist);

			add_benchmark!(params, batches, auction_manager_dex, Dex);
			add_benchmark!(params, batches, cdp_treasury, CdpTreasury);
			add_benchmark!(params, batches, honzon, HonzonBench::<Runtime>);
			add_benchmark!(params, batches, cdp_engine, CdpEngineBench::<Runtime>);
			add_benchmark!(params, batches, emergency_shutdown, EmergencyShutdownBench::<Runtime>);
			add_benchmark!(params, batches, auction_manager, AuctionManagerBench::<Runtime>);
			orml_add_benchmark!(params, batches, b"tokens", benchmarking::tokens);
			orml_add_benchmark!(params, batches, b"vesting", benchmarking::vesting);
			orml_add_benchmark!(params, batches, b"auction", benchmarking::auction);
			orml_add_benchmark!(params, batches, b"currencies", benchmarking::currencies);

			if batches.is_empty() { return Err("Benchmark not found for this module.".into()) }
			Ok(batches)
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_system::offchain::CreateSignedTransaction;

	#[test]
	fn validate_transaction_submitter_bounds() {
		fn is_submit_signed_transaction<T>()
		where
			T: CreateSignedTransaction<Call>,
		{
		}

		is_submit_signed_transaction::<Runtime>();
	}
}
