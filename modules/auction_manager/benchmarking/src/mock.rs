//! Mocks for the auction manager benchmarking.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_dispatch, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::{EnsureRoot, EnsureSignedBy};
use orml_oracle::DefaultCombineData;
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{Convert, IdentityLookup},
	ModuleId,
};
use sp_std::vec;
use support::{ExchangeRate, ExchangeRateProvider, Price, Rate};

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		orml_oracle::ModuleOracle,
		auction_manager::AuctionManagerModule,
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime where system = frame_system {}
}

pub type AccountIndex = u32;
pub type AccountId = u128;
pub type AuctionId = u32;
pub type BlockNumber = u64;

pub const ACA: CurrencyId = CurrencyId::ACA;
pub const AUSD: CurrencyId = CurrencyId::AUSD;
pub const DOT: CurrencyId = CurrencyId::DOT;
pub const LDOT: CurrencyId = CurrencyId::LDOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

impl frame_system::Trait for Runtime {
	type Origin = Origin;
	type Index = AccountIndex;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = sp_core::H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = ();
	type MaximumBlockWeight = ();
	type MaximumBlockLength = ();
	type AvailableBlockRatio = ();
	type Version = ();
	type ModuleToIndex = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = (PalletBalances,);
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}

impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type WeightInfo = ();
}
pub type PalletBalances = pallet_balances::Module<Runtime>;

pub type AdaptedBasicCurrency =
	orml_currencies::BasicCurrencyAdapter<PalletBalances, Balance, Balance, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Trait for Runtime {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Runtime>;

impl orml_auction::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManagerModule;
	type WeightInfo = ();
}
pub type AuctionModule = orml_auction::Module<Runtime>;

parameter_types! {
	pub MinimumIncrementSize: Rate = Rate::saturating_from_rational(1, 20);
	pub const AuctionTimeToClose: u64 = 100;
	pub const AuctionDurationSoftCap: u64 = 2000;
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const UnsignedPriority: u64 = 1 << 20;
}

impl auction_manager::Trait for Runtime {
	type Event = ();
	type Currency = Currencies;
	type Auction = AuctionModule;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetStableCurrencyId = GetStableCurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type CDPTreasury = CDPTreasuryModule;
	type DEX = ();
	type PriceSource = prices::Module<Runtime>;
	type UnsignedPriority = UnsignedPriority;
	type EmergencyShutdown = EmergencyShutdownModule;
}
pub type AuctionManagerModule = auction_manager::Module<Runtime>;

/// An extrinsic type used for tests.
pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryModuleId: ModuleId = ModuleId(*b"aca/cdpt");
}

impl cdp_treasury::Trait for Runtime {
	type Event = ();
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManagerModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = CDPTreasuryModuleId;
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

parameter_types! {
	pub const MinimumPeriod: u64 = 5;
}

impl pallet_timestamp::Trait for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: u32 = 1000 * 60 * 30; // 30 mins
	pub const RootOperatorAccountId: AccountId = 1;
}

impl orml_oracle::Trait<orml_oracle::Instance1> for Runtime {
	type Event = ();
	type OnNewData = ();
	type CombineData = DefaultCombineData<Self, MinimumCount, ExpiresIn, orml_oracle::Instance1>;
	type Time = pallet_timestamp::Module<Self>;
	type OracleKey = CurrencyId;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
}
pub type ModuleOracle = orml_oracle::Module<Runtime, orml_oracle::Instance1>;

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::one()
	}
}

parameter_types! {
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub StableCurrencyFixedPrice: Price = Price::one();
}

impl prices::Trait for Runtime {
	type Event = ();
	type Source = orml_oracle::Module<Runtime, orml_oracle::Instance1>;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureSignedBy<One, AccountId>;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
}

pub struct MockConvert;
impl Convert<(CurrencyId, Balance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, Balance)) -> Balance {
		a.1.into()
	}
}

parameter_types! {
	pub const LoansModuleId: ModuleId = ModuleId(*b"aca/loan");
}

impl loans::Trait for Runtime {
	type Event = ();
	type Convert = MockConvert;
	type Currency = Tokens;
	type RiskManager = ();
	type CDPTreasury = CDPTreasuryModule;
	type ModuleId = LoansModuleId;
}

parameter_types! {
	pub CollateralCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::XBTC, CurrencyId::DOT];
}

impl emergency_shutdown::Trait for Runtime {
	type Event = ();
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type PriceSource = prices::Module<Runtime>;
	type CDPTreasury = CDPTreasuryModule;
	type AuctionManagerHandler = AuctionManagerModule;
	type ShutdownOrigin = EnsureRoot<AccountId>;
}
pub type EmergencyShutdownModule = emergency_shutdown::Module<Runtime>;

impl crate::Trait for Runtime {}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let _ = orml_oracle::GenesisConfig::<Runtime, orml_oracle::Instance1> {
		members: vec![1, 2, 3].into(),
		phantom: Default::default(),
	}
	.assimilate_storage(&mut storage);

	storage.into()
}
