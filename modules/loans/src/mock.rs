//! Mocks for the loans module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use su_primitives::H256;
use support::{AuctionManager, RiskManager};
use system::EnsureSignedBy;

use super::*;

mod loans {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		loans<T>,
		orml_tokens<T>,
		pallet_balances<T>,
		orml_currencies<T>,
		cdp_treasury<T>,
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

pub type AccountId = u32;
pub type BlockNumber = u64;
pub type Balance = u64;
pub type DebitBalance = u64;
pub type Amount = i64;
pub type DebitAmount = i64;
pub type CurrencyId = u32;
pub type AuctionId = u64;
pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const NATIVE_CURRENCY_ID: CurrencyId = 0;
pub const AUSD: CurrencyId = 1;
pub const X_TOKEN_ID: CurrencyId = 2;
pub const Y_TOKEN_ID: CurrencyId = 3;

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
}
pub type System = system::Module<Runtime>;

// tokens module
impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type DustRemoval = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

// currencies module
parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
	pub const GetStableCurrencyId: CurrencyId = AUSD;
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const CreationFee: u64 = 2;
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

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type Currencies = orml_currencies::Module<Runtime>;

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
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

impl cdp_treasury::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = ();
}
pub type CDPTreasuryModule = cdp_treasury::Module<Runtime>;

// mock convert
pub struct MockConvert;
impl Convert<(CurrencyId, DebitBalance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		(a.1 / DebitBalance::from(2u64)).into()
	}
}

// mock risk manager
pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Balance, DebitBalance> for MockRiskManager {
	fn check_position_valid(
		currency_id: CurrencyId,
		_collateral_balance: Balance,
		_debit_balance: DebitBalance,
	) -> DispatchResult {
		match currency_id {
			X_TOKEN_ID => Err(sp_runtime::DispatchError::Other("mock error")),
			Y_TOKEN_ID => Ok(()),
			_ => Err(sp_runtime::DispatchError::Other("mock error")),
		}
	}

	fn check_debit_cap(currency_id: CurrencyId, total_debit_balance: DebitBalance) -> DispatchResult {
		match (currency_id, total_debit_balance) {
			(X_TOKEN_ID, 1000) => Err(sp_runtime::DispatchError::Other("mock error")),
			(Y_TOKEN_ID, 1000) => Err(sp_runtime::DispatchError::Other("mock error")),
			(_, _) => Ok(()),
		}
	}
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Convert = MockConvert;
	type Currency = Currencies;
	type RiskManager = MockRiskManager;
	type DebitBalance = DebitBalance;
	type DebitAmount = DebitAmount;
	type CDPTreasury = CDPTreasuryModule;
}
pub type LoansModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, X_TOKEN_ID, 1000),
				(ALICE, Y_TOKEN_ID, 1000),
				(BOB, X_TOKEN_ID, 1000),
				(BOB, Y_TOKEN_ID, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}
}
