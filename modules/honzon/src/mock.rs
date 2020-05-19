//! Mocks for the honzon module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_dispatch, impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::{offchain::SendTransactionTypes, EnsureSignedBy};
use primitives::Balance;
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::IdentityLookup,
	Perbill,
};
use support::{AuctionManager, ExchangeRate, Price, PriceProvider, Rate, Ratio};

mod honzon {
	pub use super::super::*;
}

impl_outer_dispatch! {
	pub enum Call for Runtime where origin: Origin {
		cdp_engine::CDPEngineModule,
	}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		honzon<T>,
		cdp_engine<T>,
		orml_tokens<T>,
		loans<T>,
		pallet_balances<T>,
		orml_currencies<T>,
		cdp_treasury,
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type DebitBalance = Balance;
pub type DebitAmount = Amount;
pub type AuctionId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const AUSD: CurrencyId = CurrencyId::AUSD;
pub const BTC: CurrencyId = CurrencyId::XBTC;
pub const DOT: CurrencyId = CurrencyId::DOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl system::Trait for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type ModuleToIndex = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
}
pub type System = system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type DustRemoval = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = system::Module<Runtime>;
}
pub type PalletBalances = pallet_balances::Module<Runtime>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type Currencies = orml_currencies::Module<Runtime>;

impl loans::Trait for Runtime {
	type Event = TestEvent;
	type Convert = cdp_engine::DebitExchangeRateConvertor<Runtime>;
	type Currency = Tokens;
	type RiskManager = CDPEngineModule;
	type DebitBalance = DebitBalance;
	type DebitAmount = DebitAmount;
	type CDPTreasury = CDPTreasuryModule;
}
pub type LoansModule = loans::Module<Runtime>;

pub struct MockPriceSource;
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(_base: CurrencyId, _quote: CurrencyId) -> Option<Price> {
		Some(Price::from_natural(1))
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		Some(Price::from_natural(1))
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
		_who: &AccountId,
		_currency_id: Self::CurrencyId,
		_amount: Self::Balance,
		_target: Self::Balance,
	) {
	}

	fn new_debit_auction(_amount: Self::Balance, _fix: Self::Balance) {}

	fn new_surplus_auction(_amount: Self::Balance) {}

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

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
}

impl cdp_treasury::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

parameter_types! {
	pub const CollateralCurrencyIds: Vec<CurrencyId> = vec![BTC, DOT];
	pub const DefaultLiquidationRatio: Ratio = Ratio::from_rational(3, 2);
	pub const DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::from_natural(1);
	pub const DefaultLiquidationPenalty: Rate = Rate::from_rational(10, 100);
	pub const MinimumDebitValue: Balance = 2;
	pub const MaxSlippageSwapWithDEX: Ratio = Ratio::from_rational(50, 100);
	pub const UnsignedPriority: u64 = 1 << 20;
}

impl cdp_engine::Trait for Runtime {
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
	type DEX = ();
	type UnsignedPriority = UnsignedPriority;
}
pub type CDPEngineModule = cdp_engine::Module<Runtime>;

/// An extrinsic type used for tests.
pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

impl Trait for Runtime {
	type Event = TestEvent;
}
pub type HonzonModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, BTC, 1000),
				(BOB, BTC, 1000),
				(ALICE, DOT, 1000),
				(BOB, DOT, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
