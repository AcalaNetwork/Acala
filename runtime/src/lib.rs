//! The Acala runtime. This can be compiled with `#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod constants;
mod types;

use sp_api::impl_runtime_apis;
use sp_core::u32_trait::{_1, _2, _3, _4};
use sp_core::OpaqueMetadata;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT, Convert, ConvertInto, OpaqueKeys, StaticLookup};
use sp_runtime::{
	create_runtime_str, curve::PiecewiseLinear, generic, impl_opaque_keys, transaction_validity::TransactionValidity,
	ApplyExtrinsicResult,
};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use version::NativeVersion;
use version::RuntimeVersion;

use orml_oracle::OperatorProvider;
use pallet_grandpa::fg_primitives;
use pallet_grandpa::AuthorityList as GrandpaAuthorityList;

// A few exports that help ease life for downstream crates.

pub use frame_support::{
	construct_runtime, parameter_types,
	traits::{Contains, Randomness},
	weights::Weight,
	StorageValue,
};
pub use pallet_staking::StakerStatus;
pub use pallet_timestamp::Call as TimestampCall;
#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;
pub use sp_runtime::{Perbill, Percent, Permill};

pub use module_primitives::CurrencyId;
pub use module_support::{ExchangeRate, Price, Rate, Ratio};
pub use orml_currencies::BasicCurrencyAdapter;

pub use constants::{currency::*, time::*};
pub use types::*;

/// This runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
	spec_name: create_runtime_str!("acala"),
	impl_name: create_runtime_str!("acala"),
	authoring_version: 1,
	spec_version: 26,
	impl_version: 0,
	apis: RUNTIME_API_VERSIONS,
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
	pub const BlockHashCount: BlockNumber = 250;
	pub const MaximumBlockWeight: Weight = 1_000_000_000;
	pub const AvailableBlockRatio: Perbill = Perbill::from_percent(75);
	pub const MaximumBlockLength: u32 = 5 * 1024 * 1024;
	pub const Version: RuntimeVersion = VERSION;
}

// FIXME: `pallet/frame-` prefix should be used for all pallet modules, but currently `frame_system`
// would cause compiling error in `construct_runtime!` https://github.com/paritytech/substrate/issues/3295
impl system::Trait for Runtime {
	/// The identifier used to distinguish between accounts.
	type AccountId = AccountId;
	/// The aggregated dispatch type that is available for extrinsics.
	type Call = Call;
	/// The lookup mechanism to get account ID from whatever is passed in dispatchers.
	type Lookup = Indices;
	/// The index type for storing how many extrinsics an account has signed.
	type Index = Index;
	/// The index type for blocks.
	type BlockNumber = BlockNumber;
	/// The type for hashing blocks and tries.
	type Hash = Hash;
	/// The hashing algorithm used.
	type Hashing = BlakeTwo256;
	/// The header type.
	type Header = generic::Header<BlockNumber, BlakeTwo256>;
	/// The ubiquitous event type.
	type Event = Event;
	/// The ubiquitous origin type.
	type Origin = Origin;
	/// Maximum number of block number to block hash mappings to keep (oldest pruned first).
	type BlockHashCount = BlockHashCount;
	/// Maximum weight of each block.
	type MaximumBlockWeight = MaximumBlockWeight;
	/// Maximum size of all encoded transactions (in bytes) that are allowed in one block.
	type MaximumBlockLength = MaximumBlockLength;
	/// Portion of the block weight that is available to all normal transactions.
	type AvailableBlockRatio = AvailableBlockRatio;
	/// Version of the runtime.
	type Version = Version;
	type ModuleToIndex = ModuleToIndex;
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
}

impl pallet_indices::Trait for Runtime {
	/// The type for recording indexing into the account enumeration. If this ever overflows, there
	/// will be problems!
	type AccountIndex = AccountIndex;
	/// Use the standard means of resolving an index hint from an id.
	type ResolveHint = pallet_indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
	/// Determine whether an account is dead.
	type IsDeadAccount = Balances;
	/// The ubiquitous event type.
	type Event = Event;
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
	pub const CreationFee: Balance = 50 * MILLICENTS;
}

impl pallet_balances::Trait for Runtime {
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// What to do if a new account is created.
	type OnNewAccount = Indices;
	type OnReapAccount = (System, Recovery);
	/// The ubiquitous event type.
	type Event = Event;
	type DustRemoval = ();
	type TransferPayment = ();
	type ExistentialDeposit = AcaExistentialDeposit;
	type CreationFee = CreationFee;
}

parameter_types! {
	pub const TransactionBaseFee: Balance = 200 * MILLICENTS;
	pub const TransactionByteFee: Balance = 10 * MILLICENTS;
}

impl pallet_transaction_payment::Trait for Runtime {
	type Currency = pallet_balances::Module<Runtime>;
	type OnTransactionPayment = ();
	type TransactionBaseFee = TransactionBaseFee;
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = ConvertInto;
	type FeeMultiplierUpdate = ();
}

impl pallet_sudo::Trait for Runtime {
	type Event = Event;
	type Proposal = Call;
}

type GeneralCouncilInstance = pallet_collective::Instance1;
impl pallet_collective::Trait<GeneralCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

type GeneralCouncilMembershipInstance = pallet_membership::Instance1;
impl pallet_membership::Trait<GeneralCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_3, _4, AccountId, GeneralCouncilInstance>;
	type MembershipInitialized = GeneralCouncil;
	type MembershipChanged = GeneralCouncil;
}

type FinancialCouncilInstance = pallet_collective::Instance2;
impl pallet_collective::Trait<FinancialCouncilInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

type FinancialCouncilMembershipInstance = pallet_membership::Instance2;
impl pallet_membership::Trait<FinancialCouncilMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, GeneralCouncilInstance>;
	type MembershipInitialized = FinancialCouncil;
	type MembershipChanged = FinancialCouncil;
}

type OperatorCollectiveInstance = pallet_collective::Instance3;
impl pallet_collective::Trait<OperatorCollectiveInstance> for Runtime {
	type Origin = Origin;
	type Proposal = Call;
	type Event = Event;
}

type OperatorMembershipInstance = pallet_membership::Instance3;
impl pallet_membership::Trait<OperatorMembershipInstance> for Runtime {
	type Event = Event;
	type AddOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type RemoveOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type SwapOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type ResetOrigin = pallet_collective::EnsureProportionMoreThan<_1, _3, AccountId, GeneralCouncilInstance>;
	type MembershipInitialized = OperatorCollective;
	type MembershipChanged = OperatorCollective;
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
}

parameter_types! {
	pub const ProposalBond: Permill = Permill::from_percent(5);
	pub const ProposalBondMinimum: Balance = 1 * DOLLARS;
	pub const SpendPeriod: BlockNumber = 1 * DAYS;
	pub const Burn: Permill = Permill::from_percent(0);
	pub const TipCountdown: BlockNumber = 1 * DAYS;
	pub const TipFindersFee: Percent = Percent::from_percent(10);
	pub const TipReportDepositBase: Balance = 1 * DOLLARS;
	pub const TipReportDepositPerByte: Balance = 1 * CENTS;
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
}

parameter_types! {
	pub const DisabledValidatorsThreshold: Perbill = Perbill::from_percent(17);
}

impl pallet_session::Trait for Runtime {
	type SessionManager = Staking;
	type SessionHandler = <opaque::SessionKeys as OpaqueKeys>::KeyTypeIdProviders;
	type ShouldEndSession = Babe;
	type Event = Event;
	type Keys = opaque::SessionKeys;
	type ValidatorId = <Self as system::Trait>::AccountId;
	type ValidatorIdOf = pallet_staking::StashOf<Self>;
	type DisabledValidatorsThreshold = DisabledValidatorsThreshold;
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
	pub const SessionsPerEra: sp_staking::SessionIndex = 6; // 1 hour
	pub const BondingDuration: pallet_staking::EraIndex = 4; // 4 hours
	pub const SlashDeferDuration: pallet_staking::EraIndex = 4; // 4 hours
	pub const RewardCurve: &'static PiecewiseLinear<'static> = &REWARD_CURVE;
}

impl pallet_staking::Trait for Runtime {
	type Currency = Balances;
	type Time = Timestamp;
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
}

parameter_types! {
	pub const ConfigDepositBase: Balance = 10 * CENTS;
	pub const FriendDepositFactor: Balance = 1 * CENTS;
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
	type Handler = module_auction_manager::Module<Runtime>;
}

pub struct OperatorCollectiveProvider;
impl OperatorProvider<AccountId> for OperatorCollectiveProvider {
	fn can_feed_data(who: &AccountId) -> bool {
		OperatorCollective::is_member(who)
	}

	fn operators() -> Vec<AccountId> {
		OperatorCollective::members()
	}
}

parameter_types! {
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: Moment = 1000 * 60 * 60 * 24 * 7; // 7 days
}

impl orml_oracle::Trait for Runtime {
	type Event = Event;
	type OnNewData = ();
	type OperatorProvider = OperatorCollectiveProvider;
	type CombineData = orml_oracle::DefaultCombineData<Runtime, MinimumCount, ExpiresIn>;
	type Time = Timestamp;
	type OracleKey = CurrencyId;
	type OracleValue = Price;
}

pub type TimeStampedPrice = orml_oracle::TimestampedValueOf<Runtime>;

parameter_types! {
	pub const TokenExistentialDeposit: Balance = 0;
}

impl orml_tokens::Trait for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = TokenExistentialDeposit;
	type DustRemoval = ();
}

parameter_types! {
	pub const StableCurrencyFixedPrice: Price = Price::from_rational(1, 1);
}

impl module_prices::Trait for Runtime {
	type CurrencyId = CurrencyId;
	type Source = orml_oracle::Module<Runtime>;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::ACA;
	pub const GetStableCurrencyId: CurrencyId = CurrencyId::AUSD;
}

impl orml_currencies::Trait for Runtime {
	type Event = Event;
	type MultiCurrency = orml_tokens::Module<Runtime>;
	type NativeCurrency = BasicCurrencyAdapter<Runtime, pallet_balances::Module<Runtime>, Balance>;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const MinimumIncrementSize: Rate = Rate::from_rational(2, 100);
	pub const AuctionTimeToClose: BlockNumber = 15 * MINUTES;
	pub const AuctionDurationSoftCap: BlockNumber = 2 * HOURS;
	pub const GetAmountAdjustment: Rate = Rate::from_rational(20, 100);
}

impl module_auction_manager::Trait for Runtime {
	type Event = Event;
	type Currency = orml_currencies::Module<Runtime>;
	type Auction = orml_auction::Module<Runtime>;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetStableCurrencyId = GetStableCurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Treasury = module_cdp_treasury::Module<Runtime>;
	type GetAmountAdjustment = GetAmountAdjustment;
	type PriceSource = module_prices::Module<Runtime>;
}

impl module_loans::Trait for Runtime {
	type Event = Event;
	type Convert = module_cdp_engine::DebitExchangeRateConvertor<Runtime>;
	type Currency = orml_currencies::Module<Runtime>;
	type RiskManager = module_cdp_engine::Module<Runtime>;
	type DebitBalance = Balance;
	type DebitAmount = Amount;
	type Treasury = module_cdp_treasury::Module<Runtime>;
}

parameter_types! {
	pub const CollateralCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::DOT, CurrencyId::XBTC];
	pub const GlobalStabilityFee: Rate = Rate::from_rational(14708, 100000000000u128); // 0.00000014708 per block, or 10% per month
	pub const DefaultLiquidationRatio: Ratio = Ratio::from_rational(150, 100);
	pub const DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::from_rational(1, 10);
	pub const DefaultLiquidationPenalty: Rate = Rate::from_rational(10, 100);
	pub const MinimumDebitValue: Balance = 1 * DOLLARS;
	pub const MaxSlippageSwapWithDex: Ratio = Ratio::from_rational(5, 100);
}

impl module_cdp_engine::Trait for Runtime {
	type Event = Event;
	type PriceSource = module_prices::Module<Runtime>;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type GlobalStabilityFee = GlobalStabilityFee;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = MinimumDebitValue;
	type GetStableCurrencyId = GetStableCurrencyId;
	type Treasury = module_cdp_treasury::Module<Runtime>;
	type UpdateOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, FinancialCouncilInstance>;
	type MaxSlippageSwapWithDex = MaxSlippageSwapWithDex;
	type Currency = orml_currencies::Module<Runtime>;
	type Dex = module_dex::Module<Runtime>;
}

impl module_honzon::Trait for Runtime {
	type Event = Event;
}

impl module_emergency_shutdown::Trait for Runtime {
	type Event = Event;
	type PriceSource = module_prices::Module<Runtime>;
	type Treasury = module_cdp_treasury::Module<Runtime>;
	type AuctionManagerHandler = module_auction_manager::Module<Runtime>;
	type OnShutdown = (
		module_cdp_treasury::Module<Runtime>,
		module_cdp_engine::Module<Runtime>,
		module_honzon::Module<Runtime>,
	);
	type ShutdownOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, FinancialCouncilInstance>;
}

parameter_types! {
	pub const GetExchangeFee: Rate = Rate::from_rational(2, 1000);
}

impl module_dex::Trait for Runtime {
	type Event = Event;
	type Currency = orml_currencies::Module<Runtime>;
	type Share = Share;
	type GetBaseCurrencyId = GetStableCurrencyId;
	type GetExchangeFee = GetExchangeFee;
}

impl module_cdp_treasury::Trait for Runtime {
	type Currency = orml_currencies::Module<Runtime>;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = module_auction_manager::Module<Runtime>;
	type UpdateOrigin = pallet_collective::EnsureProportionMoreThan<_1, _2, AccountId, FinancialCouncilInstance>;
	type Dex = module_dex::Module<Runtime>;
}

parameter_types! {
	pub const FreeTransferCount: u8 = 3;
	pub const FreeTransferPeriod: BlockNumber = 1 * DAYS;
	pub const FreeTransferDeposit: Balance = 1 * DOLLARS;
}

impl module_accounts::Trait for Runtime {
	type FreeTransferCount = FreeTransferCount;
	type FreeTransferPeriod = FreeTransferPeriod;
	type FreeTransferDeposit = FreeTransferDeposit;
	type Time = Timestamp;
	type Currency = orml_currencies::Module<Runtime>;
	type Call = Call;
	type DepositCurrency = Balances;
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = opaque::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{Module, Call, Storage, Config, Event},
		Timestamp: pallet_timestamp::{Module, Call, Storage, Inherent},
		Babe: pallet_babe::{Module, Call, Storage, Config, Inherent(Timestamp)},
		Grandpa: pallet_grandpa::{Module, Call, Storage, Config, Event},
		Indices: pallet_indices,
		Balances: pallet_balances::{Module, Storage, Config<T>, Event<T>},
		TransactionPayment: pallet_transaction_payment::{Module, Storage},
		Sudo: pallet_sudo,
		RandomnessCollectiveFlip: pallet_randomness_collective_flip::{Module, Call, Storage},
		GeneralCouncil: pallet_collective::<Instance1>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		GeneralCouncilMembership: pallet_membership::<Instance1>::{Module, Call, Storage, Event<T>, Config<T>},
		FinancialCouncil: pallet_collective::<Instance2>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		FinancialCouncilMembership: pallet_membership::<Instance2>::{Module, Call, Storage, Event<T>, Config<T>},
		OperatorCollective: pallet_collective::<Instance3>::{Module, Call, Storage, Origin<T>, Event<T>, Config<T>},
		OperatorMembership: pallet_membership::<Instance3>::{Module, Call, Storage, Event<T>, Config<T>},
		Utility: pallet_utility::{Module, Call, Storage, Event<T>},
		PalletTreasury: pallet_treasury::{Module, Call, Storage, Config, Event<T>},
		Staking: pallet_staking,
		Session: pallet_session::{Module, Call, Storage, Event, Config<T>},
		Recovery: pallet_recovery::{Module, Call, Storage, Event<T>},

		Currencies: orml_currencies::{Module, Call, Event<T>},
		Oracle: orml_oracle::{Module, Storage, Call, Event<T>},
		Prices: module_prices::{Module, Storage},
		Tokens: orml_tokens::{Module, Storage, Event<T>, Config<T>},
		Auction: orml_auction::{Module, Storage, Call, Event<T>},
		AuctionManager: module_auction_manager::{Module, Storage, Call, Event<T>},
		Loans: module_loans::{Module, Storage, Call, Event<T>},
		CdpEngine: module_cdp_engine::{Module, Storage, Call, Event<T>, Config<T>},
		Honzon: module_honzon::{Module, Storage, Call, Event<T>},
		Dex: module_dex::{Module, Storage, Call, Event<T>},
		CdpTreasury: module_cdp_treasury::{Module, Storage, Call, Config<T>},
		EmergencyShutdown: module_emergency_shutdown::{Module, Storage, Call, Event<T>},
		Accounts: module_accounts::{Module, Call, Storage},
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
	system::CheckVersion<Runtime>,
	system::CheckGenesis<Runtime>,
	system::CheckEra<Runtime>,
	system::CheckNonce<Runtime>,
	system::CheckWeight<Runtime>,
	module_accounts::ChargeTransactionPayment<Runtime>,
);
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedExtrinsic<Address, Call, Signature, SignedExtra>;
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

	impl block_builder_api::BlockBuilder<Block> for Runtime {
		fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
			Executive::apply_extrinsic(extrinsic)
		}

		fn finalize_block() -> <Block as BlockT>::Header {
			Executive::finalize_block()
		}

		fn inherent_extrinsics(data: inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
			data.create_extrinsics()
		}

		fn check_inherents(
			block: Block,
			data: inherents::InherentData,
		) -> inherents::CheckInherentsResult {
			data.check_extrinsics(&block)
		}

		fn random_seed() -> <Block as BlockT>::Hash {
			RandomnessCollectiveFlip::random_seed()
		}
	}

	impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
		fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
			Executive::validate_transaction(tx)
		}
	}

	impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
		fn offchain_worker(header: &<Block as BlockT>::Header) {
			Executive::offchain_worker(header)
		}
	}

	impl sp_consensus_babe::BabeApi<Block> for Runtime {
		fn configuration() -> sp_consensus_babe::BabeConfiguration {
			// The choice of `c` parameter (where `1 - c` represents the
			// probability of a slot being empty), is done in accordance to the
			// slot duration and expected target block time, for safely
			// resisting network delays of maximum two seconds.
			// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>
			sp_consensus_babe::BabeConfiguration {
				slot_duration: Babe::slot_duration(),
				epoch_length: EpochDuration::get(),
				c: PRIMARY_PROBABILITY,
				genesis_authorities: Babe::authorities(),
				randomness: Babe::randomness(),
				secondary_slots: true,
			}
		}
	}

	impl sp_session::SessionKeys<Block> for Runtime {
		fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
			opaque::SessionKeys::generate(seed)
		}

		fn decode_session_keys(
			encoded: Vec<u8>,
		) -> Option<Vec<(Vec<u8>, sp_core::crypto::KeyTypeId)>> {
			opaque::SessionKeys::decode_into_raw_public_keys(&encoded)
		}
	}

	impl fg_primitives::GrandpaApi<Block> for Runtime {
		fn grandpa_authorities() -> GrandpaAuthorityList {
			Grandpa::grandpa_authorities()
		}
	}

	impl system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Index> for Runtime {
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
	}
}
