//! The Acala runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]
// The `large_enum_variant` warning originates from `construct_runtime` macro.
#![allow(clippy::large_enum_variant)]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

use codec::Encode;
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
	transaction_validity::{TransactionPriority, TransactionSource, TransactionValidity},
	ApplyExtrinsicResult, ModuleId,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use frame_system::{self as system};
use orml_currencies::{BasicCurrencyAdapter, Currency};
use pallet_grandpa::fg_primitives;
use pallet_grandpa::{AuthorityId as GrandpaId, AuthorityList as GrandpaAuthorityList};
use pallet_session::historical as pallet_session_historical;

pub use frame_support::{
	construct_runtime, debug, parameter_types,
	traits::{Contains, ContainsLengthBound, KeyOwnerProofSystem, Randomness},
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
pub use sp_runtime::{Perbill, Percent, Permill};

pub use constants::{currency::*, time::*};
pub use types::*;

mod benchmarking;
mod constants;
mod types;

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("acala"),
	impl_name: create_runtime_str!("acala"),
	authoring_version: 1,
	spec_version: 404,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
	transaction_version: 1,
};

/// The version infromation used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
	NativeVersion {
		runtime_version: VERSION,
		can_author_with: Default::default(),
	}
}

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core datastructures.
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

parameter_types! {
	pub const BlockHashCount: BlockNumber = 900; // mortal tx can be valid up to 1 hour after signing
	/// We allow for 2 seconds of compute with a 4 second average block time.
	pub const MaximumBlockWeight: Weight = 2 * WEIGHT_PER_SECOND;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	/// Assume 10% of weight for average on_initialize calls.
	pub const MaximumExtrinsicWeight: Weight = AvailableBlockRatio::get()
		.saturating_sub(Perbill::from_percent(10)) * MaximumBlockWeight::get();
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}

impl system::Trait for Runtime {
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
	type OnKilledAccount = ();
	type DbWeight = RocksDbWeight;
	type BlockExecutionWeight = BlockExecutionWeight;
	type ExtrinsicBaseWeight = ExtrinsicBaseWeight;
	type MaximumExtrinsicWeight = MaximumExtrinsicWeight;
}

parameter_types! {
	pub const EpochDuration: u64 = EPOCH_DURATION_IN_SLOTS;
	pub const ExpectedBlockTime: Moment = MILLISECS_PER_BLOCK;
}

impl pallet_babe::Trait for Runtime {
	type EpochDuration = EpochDuration;
	type ExpectedBlockTime = ExpectedBlockTime;
	type EpochChangeTrigger = pallet_babe::ExternalTrigger;
}

impl pallet_grandpa::Trait for Runtime {
	type Event = Event;
	type Call = Call;

	type KeyOwnerProofSystem = Historical;

	type KeyOwnerProof = <Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::Proof;

	type KeyOwnerIdentification =
		<Self::KeyOwnerProofSystem as KeyOwnerProofSystem<(KeyTypeId, GrandpaId)>>::IdentificationTuple;

	type HandleEquivocation = pallet_grandpa::EquivocationHandler<
		Self::KeyOwnerIdentification,
		report::ReporterAppCrypto,
		Runtime,
		(), //Offences,
	>;
}

parameter_types! {
	pub const IndexDeposit: Balance = DOLLARS;
}

impl pallet_indices::Trait for Runtime {
	type AccountIndex = AccountIndex;
	type Event = Event;
	type Currency = Balances;
	type Deposit = IndexDeposit;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Trait for Runtime {
	/// A timestamp: milliseconds since the unix epoch.
	type Moment = Moment;
	type OnTimestampSet = Babe;
	type MinimumPeriod = MinimumPeriod;
}

parameter_types! {
	pub const AcaExistentialDeposit: Balance = 100 * MILLICENTS;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = AcaExistentialDeposit;
	type AccountStore = system::Module<Runtime>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = Balances;
	type OnTransactionPayment = ();
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
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
}

type GeneralCouncilMembershipInstance = pallet_membership::Instance1;
impl pallet_membership::Trait<GeneralCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type PrimeOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
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
}

type HonzonCouncilMembershipInstance = pallet_membership::Instance2;
impl pallet_membership::Trait<HonzonCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type PrimeOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
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
}

type HomaCouncilMembershipInstance = pallet_membership::Instance3;
impl pallet_membership::Trait<HomaCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type PrimeOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
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
}

type TechnicalCouncilMembershipInstance = pallet_membership::Instance4;
impl pallet_membership::Trait<TechnicalCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type PrimeOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type MembershipInitialized = TechnicalCouncil;
	type MembershipChanged = TechnicalCouncil;
}

type OperatorMembershipInstance = pallet_membership::Instance5;
impl pallet_membership::Trait<OperatorMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type PrimeOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type MembershipInitialized = Oracle;
	type MembershipChanged = Oracle;
}

parameter_types! {
	pub const MultisigDepositBase: Balance = 500 * MILLICENTS;
	pub const MultisigDepositFactor: Balance = 100 * MILLICENTS;
	pub const MaxSignatories: u16 = 100;
}

impl pallet_utility::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type MultisigDepositBase = MultisigDepositBase;
	type MultisigDepositFactor = MultisigDepositFactor;
	type MaxSignatories = MaxSignatories;
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
	pub const TreasuryModuleId: ModuleId = ModuleId(*b"py/trsry");
}

impl pallet_treasury::Trait for Runtime {
	type Currency = Balances;
	type ApproveOrigin = pallet_collective::EnsureMembers<_4, AccountId, GeneralCouncilInstance>;
	type RejectOrigin = pallet_collective::EnsureMembers<_2, AccountId, GeneralCouncilInstance>;
	type Event = Event;
	type ProposalRejection = ();
	type ProposalBond = ProposalBond;
	type ProposalBondMinimum = ProposalBondMinimum;
	type SpendPeriod = SpendPeriod;
	type Burn = Burn;
	type Tippers = GeneralCouncilProvider;
	type TipCountdown = TipCountdown;
	type TipFindersFee = TipFindersFee;
	type TipReportDepositBase = TipReportDepositBase;
	type TipReportDepositPerByte = TipReportDepositPerByte;
	type ModuleId = TreasuryModuleId;
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl pallet_session::Trait for Runtime {
	type Event = Event;
	type ValidatorId = <Self as system::Trait>::AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	type ShouldEndSession = Babe;
	type SessionManager = pallet_session::historical::NoteHistoricalRoot<Self, Staking>;
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type Keys = opaque::SessionKeys;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
	type NextSessionRotation = Babe;
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

/// Struct that handles the conversion of Balance -> `u64`. This is used for staking's election
/// calculation.
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
	pub const ElectionLookahead: BlockNumber = 25;
	pub const StakingUnsignedPriority: TransactionPriority = TransactionPriority::max_value() / 2;
	pub const MaxIterations: u32 = 5;
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
	type SlashCancelOrigin = pallet_collective::EnsureProportionAtLeast<_3, _4, AccountId, GeneralCouncilInstance>;
	type SessionInterface = Self;
	type RewardCurve = RewardCurve;
	type NextNewSession = Session;
	type ElectionLookahead = ElectionLookahead;
	type Call = Call;
	type MaxIterations = MaxIterations;
	type MaxNominatorRewardedPerValidator = MaxNominatorRewardedPerValidator;
	type UnsignedPriority = StakingUnsignedPriority;
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
	type DustRemoval = ();
	type OnReceived = ();
}

parameter_types! {
	pub const StableCurrencyFixedPrice: Price = Price::from_rational(1, 1);
}

impl module_prices::Trait for Runtime {
	type Event = Event;
	type Source = Oracle;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
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
	type NativeCurrency = BasicCurrencyAdapter<Runtime, Balances, Balance>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

impl orml_vesting::Trait for Runtime {
	type Event = Event;
	type Currency = pallet_balances::Module<Runtime>;
}

parameter_types! {
	pub const MaxScheduleDispatchWeight: Weight = 100_000_000;
}

impl orml_schedule_update::Trait for Runtime {
	type Event = Event;
	type Call = Call;
	type MaxScheduleDispatchWeight = MaxScheduleDispatchWeight;
	type DispatchOrigin = system::EnsureRoot<AccountId>;
}

parameter_types! {
	pub const MinimumIncrementSize: Rate = Rate::from_rational(2, 100);
	pub const AuctionTimeToClose: BlockNumber = 15 * MINUTES;
	pub const AuctionDurationSoftCap: BlockNumber = 2 * HOURS;
	pub const GetAmountAdjustment: Rate = Rate::from_rational(20, 100);
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
}

impl module_loans::Trait for Runtime {
	type Event = Event;
	type Convert = module_cdp_engine::DebitExchangeRateConvertor<Runtime>;
	type Currency = Currencies;
	type RiskManager = CdpEngine;
	type DebitBalance = Balance;
	type DebitAmount = Amount;
	type CDPTreasury = CdpTreasury;
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
			system::CheckSpecVersion::<Runtime>::new(),
			system::CheckTxVersion::<Runtime>::new(),
			system::CheckGenesis::<Runtime>::new(),
			system::CheckEra::<Runtime>::from(generic::Era::mortal(period, current_block)),
			system::CheckNonce::<Runtime>::from(nonce),
			system::CheckWeight::<Runtime>::new(),
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
	pub const CollateralCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::XBTC, CurrencyId::LDOT];
	pub const DefaultLiquidationRatio: Ratio = Ratio::from_rational(110, 100);
	pub const DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::from_rational(1, 10);
	pub const DefaultLiquidationPenalty: Rate = Rate::from_rational(5, 100);
	pub const MinimumDebitValue: Balance = DOLLARS;
	pub const MaxSlippageSwapWithDEX: Ratio = Ratio::from_rational(5, 100);
	pub const CdpEngineUnsignedPriority: TransactionPriority = TransactionPriority::max_value();
}

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
	type UpdateOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, HonzonCouncilInstance>;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
	type DEX = Dex;
	type UnsignedPriority = CdpEngineUnsignedPriority;
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
	type OnShutdown = (CdpTreasury, CdpEngine, Honzon, Dex);
	type ShutdownOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
}

parameter_types! {
	pub const GetExchangeFee: Rate = Rate::from_rational(1, 1000);
	pub const EnabledCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::XBTC, CurrencyId::LDOT, CurrencyId::ACA];
}

impl module_dex::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type Share = Share;
	type EnabledCurrencyIds = EnabledCurrencyIds;
	type GetBaseCurrencyId = GetStableCurrencyId;
	type GetExchangeFee = GetExchangeFee;
	type CDPTreasury = CdpTreasury;
	type UpdateOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, HonzonCouncilInstance>;
}

parameter_types! {
	pub const MaxAuctionsCount: u32 = 100;
}

impl module_cdp_treasury::Trait for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManager;
	type UpdateOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, HonzonCouncilInstance>;
	type DEX = Dex;
	type MaxAuctionsCount = MaxAuctionsCount;
}

parameter_types! {
	pub const FreeTransferCount: u8 = 3;
	pub const FreeTransferPeriod: BlockNumber = DAYS;
	pub const FreeTransferDeposit: Balance = DOLLARS;
}

impl module_accounts::Trait for Runtime {
	type FreeTransferCount = FreeTransferCount;
	type FreeTransferPeriod = FreeTransferPeriod;
	type FreeTransferDeposit = FreeTransferDeposit;
	type Time = Timestamp;
	type DepositCurrency = Balances;
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
	pub const MaxBondRatio: Ratio = Ratio::from_rational(95, 100);	// 95%
	pub const MinBondRatio: Ratio = Ratio::from_rational(80, 100);	// 80%
	pub const MaxClaimFee: Rate = Rate::from_rational(5, 100);	// 5%
	pub const DefaultExchangeRate: ExchangeRate = ExchangeRate::from_rational(10, 100);	// 1 : 10
	pub const ClaimFeeReturnRatio: Ratio = Ratio::from_rational(98, 100); // 98%
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
}

#[allow(clippy::large_enum_variant)]
construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		// srml modules
		System: system::{Module, Call, Storage, Config, Event<T>},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
		Indices: pallet_indices::{Module, Call, Storage, Config<T>, Event<T>},
		Balances: pallet_balances::{Module, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Module, Storage},
		Sudo: pallet_sudo::{Module, Call, Config<T>, Storage, Event<T>},
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
		Utility: pallet_utility::{Module, Call, Storage, Event<T>},
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

		// acala modules
		Currencies: orml_currencies::{Module, Call, Event<T>},
		Prices: module_prices::{Module, Storage, Call, Event},
		Tokens: orml_tokens::{Module, Storage, Event<T>, Config<T>},
		Vesting: orml_vesting::{Module, Storage, Call, Event<T>, Config<T>},
		ScheduleUpdate: orml_schedule_update::{Module, Storage, Call, Event<T>},
		Auction: orml_auction::{Module, Storage, Call, Event<T>},
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
	system::CheckSpecVersion<Runtime>,
	system::CheckTxVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	module_accounts::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
/// The payload being signed in transactions.
pub type SignedPayload = generic::SignedPayload<Call, SignedExtra>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Call, SignedExtra>;
/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Runtime, AllModules>;

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

		fn submit_report_equivocation_extrinsic(
			equivocation_proof: fg_primitives::EquivocationProof<
				<Block as BlockT>::Hash,
				NumberFor<Block>,
			>,
			key_owner_proof: fg_primitives::OpaqueKeyOwnershipProof,
		) -> Option<()> {
			let key_owner_proof = key_owner_proof.decode()?;

			Grandpa::submit_report_equivocation_extrinsic(
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

			let mut batches = Vec::<BenchmarkBatch>::new();
			let params = (&pallet, &benchmark, &lowest_range_values, &highest_range_values, &steps, repeat);

			add_benchmark!(params, batches, b"dex", Dex);
			add_benchmark!(params, batches, b"cdp-treasury", CdpTreasury);
			add_benchmark!(params, batches, b"honzon", HonzonBench::<Runtime>);
			add_benchmark!(params, batches, b"cdp-engine", CdpEngineBench::<Runtime>);
			add_benchmark!(params, batches, b"emergency-shutdown", EmergencyShutdownBench::<Runtime>);
			add_benchmark!(params, batches, b"auction-manager", AuctionManagerBench::<Runtime>);
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
