//! Mocks for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_dispatch, impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{TokenSymbol, TradingPair};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::IdentityLookup,
	ModuleId,
};
use sp_std::cell::RefCell;
use support::{AuctionManager, EmergencyShutdown};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type AuctionId = u32;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod cdp_engine {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		cdp_engine<T>,
		orml_tokens<T>,
		loans<T>,
		pallet_balances<T>,
		orml_currencies<T>,
		module_dex<T>,
		cdp_treasury,
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		cdp_engine::CDPEngineModule,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}
pub type PalletBalances = pallet_balances::Module<Runtime>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Config for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Runtime>;

parameter_types! {
	pub const LoansModuleId: ModuleId = ModuleId(*b"aca/loan");
}

impl loans::Config for Runtime {
	type Event = TestEvent;
	type Convert = DebitExchangeRateConvertor<Runtime>;
	type Currency = Currencies;
	type RiskManager = CDPEngineModule;
	type CDPTreasury = CDPTreasuryModule;
	type ModuleId = LoansModuleId;
	type OnUpdateLoan = ();
}
pub type LoansModule = loans::Module<Runtime>;

thread_local! {
	static RELATIVE_PRICE: RefCell<Option<Price>> = RefCell::new(Some(Price::one()));
}

pub struct MockPriceSource;
impl MockPriceSource {
	pub fn set_relative_price(price: Option<Price>) {
		RELATIVE_PRICE.with(|v| *v.borrow_mut() = price);
	}
}
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		match (base, quote) {
			(AUSD, BTC) => RELATIVE_PRICE.with(|v| *v.borrow_mut()),
			(BTC, AUSD) => RELATIVE_PRICE.with(|v| *v.borrow_mut()),
			_ => None,
		}
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		Some(Price::one())
	}

	fn lock_price(_currency_id: CurrencyId) {}

	fn unlock_price(_currency_id: CurrencyId) {}
}

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		_refund_recipient: &AccountId,
		_currency_id: Self::CurrencyId,
		_amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn new_debit_auction(_amount: Self::Balance, _fix: Self::Balance) -> DispatchResult {
		Ok(())
	}

	fn new_surplus_auction(_amount: Self::Balance) -> DispatchResult {
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		Ok(())
	}

	fn get_total_debit_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}

	fn get_total_surplus_in_auction() -> Self::Balance {
		Default::default()
	}
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryModuleId: ModuleId = ModuleId(*b"aca/cdpt");
}

impl cdp_treasury::Config for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = DEXModule;
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = CDPTreasuryModuleId;
	type WeightInfo = ();
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

parameter_types! {
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub const TradingPathLimit: usize = 3;
	pub EnabledTradingPairs : Vec<TradingPair> = vec![TradingPair::new(AUSD, BTC), TradingPair::new(AUSD, DOT)];
}

impl module_dex::Config for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = DEXModuleId;
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<One, AccountId>;
}
pub type DEXModule = module_dex::Module<Runtime>;

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

pub fn mock_shutdown() {
	IS_SHUTDOWN.with(|v| *v.borrow_mut() = true)
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IS_SHUTDOWN.with(|v| *v.borrow_mut())
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(3, 2);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::one();
	pub DefaultLiquidationPenalty: Rate = Rate::saturating_from_rational(10, 100);
	pub const MinimumDebitValue: Balance = 2;
	pub MaxSlippageSwapWithDEX: Ratio = Ratio::saturating_from_rational(50, 100);
	pub const UnsignedPriority: u64 = 1 << 20;
	pub CollateralCurrencyIds: Vec<CurrencyId> = vec![BTC, DOT];
}

impl Config for Runtime {
	type Event = TestEvent;
	type PriceSource = MockPriceSource;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = MinimumDebitValue;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CDPTreasuryModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
	type DEX = DEXModule;
	type UnsignedPriority = UnsignedPriority;
	type EmergencyShutdown = MockEmergencyShutdown;
	type WeightInfo = ();
}
pub type CDPEngineModule = Module<Runtime>;

/// An extrinsic type used for tests.
pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, BTC, 1000),
				(BOB, BTC, 1000),
				(CAROL, BTC, 100),
				(ALICE, DOT, 1000),
				(BOB, DOT, 1000),
				(CAROL, AUSD, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_dex::GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
