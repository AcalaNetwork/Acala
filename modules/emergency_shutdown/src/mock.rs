//! Mocks for the honzon module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use primitives::Amount;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{Convert, IdentityLookup},
	DispatchResult, ModuleId, Perbill,
};
use support::{AuctionManager, Price, PriceProvider};

pub type AccountId = u128;
pub type AuctionId = u64;
pub type BlockNumber = u64;
pub type DebitBalance = Balance;
pub type DebitAmount = Amount;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const AUSD: CurrencyId = CurrencyId::AUSD;
pub const BTC: CurrencyId = CurrencyId::XBTC;
pub const DOT: CurrencyId = CurrencyId::DOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod emergency_shutdown {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		emergency_shutdown<T>,
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
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
}
pub type System = system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
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

// mock convert
pub struct MockConvert;
impl Convert<(CurrencyId, DebitBalance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		a.1.into()
	}
}

parameter_types! {
	pub const LoansModuleId: ModuleId = ModuleId(*b"aca/loan");
}

impl loans::Trait for Runtime {
	type Event = TestEvent;
	type Convert = MockConvert;
	type Currency = Tokens;
	type RiskManager = ();
	type DebitBalance = DebitBalance;
	type DebitAmount = DebitAmount;
	type CDPTreasury = CDPTreasuryModule;
	type ModuleId = LoansModuleId;
}

pub struct MockPriceSource;
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(_base: CurrencyId, _quote: CurrencyId) -> Option<Price> {
		Some(Price::one())
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
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryModuleId: ModuleId = ModuleId(*b"aca/cdpt");
}

impl cdp_treasury::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
	type MaxAuctionsCount = MaxAuctionsCount;
	type ModuleId = CDPTreasuryModuleId;
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

ord_parameter_types! {
	pub const CollateralCurrencyIds: Vec<CurrencyId> = vec![BTC, DOT];
}

impl Trait for Runtime {
	type Event = TestEvent;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type PriceSource = MockPriceSource;
	type CDPTreasury = CDPTreasuryModule;
	type AuctionManagerHandler = MockAuctionManager;
	type OnShutdown = CDPTreasuryModule;
	type ShutdownOrigin = EnsureSignedBy<One, AccountId>;
}
pub type EmergencyShutdownModule = Module<Runtime>;

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
