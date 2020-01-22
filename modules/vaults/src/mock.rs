//! Mocks for the vaults module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use su_primitives::H256;
use support::{AuctionManager, RiskManager};
use system::EnsureSignedBy;

use super::*;

mod vaults {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		vaults<T>,
		orml_tokens<T>,
		pallet_balances<T>,
		orml_currencies<T>,
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
pub const ALICE: AccountId = 1;
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
	pub const ExistentialDeposit: u64 = 0;
	pub const TransferFee: u64 = 0;
	pub const CreationFee: u64 = 2;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type OnFreeBalanceZero = ();
	type OnNewAccount = ();
	type OnReapAccount = ();
	type TransferPayment = ();
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
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

	#[allow(unused_variables)]
	fn new_collateral_auction(
		who: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
		bad_debt: Self::Balance,
	) {
	}

	#[allow(unused_variables)]
	fn new_debit_auction(amount: Self::Balance, fix: Self::Balance) {}

	#[allow(unused_variables)]
	fn new_surplus_auction(amount: Self::Balance) {}

	fn get_total_debit_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Default::default()
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

impl cdp_treasury::Trait for Runtime {
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
}
pub type CdpTreasury = cdp_treasury::Module<Runtime>;

// mock convert
pub struct MockConvert;
impl Convert<(CurrencyId, DebitBalance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		(a.1 / DebitBalance::from(2u64)).into()
	}
}

// mock risk manager
pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Amount, DebitAmount> for MockRiskManager {
	#[allow(unused_variables)]
	fn check_position_adjustment(
		account_id: &AccountId,
		currency_id: CurrencyId,
		collaterals: Amount,
		debits: DebitAmount,
	) -> DispatchResult {
		match currency_id {
			X_TOKEN_ID => Err(sp_runtime::DispatchError::Other("mock error")),
			Y_TOKEN_ID => Ok(()),
			_ => Err(sp_runtime::DispatchError::Other("mock error")),
		}
	}
	#[allow(unused_variables)]
	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> DispatchResult {
		match (currency_id, debits) {
			(X_TOKEN_ID, 1000i64) => Err(sp_runtime::DispatchError::Other("mock error")),
			(Y_TOKEN_ID, 1000i64) => Err(sp_runtime::DispatchError::Other("mock error")),
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
	type Treasury = CdpTreasury;
}
pub type VaultsModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, X_TOKEN_ID, 1000), (ALICE, Y_TOKEN_ID, 1000)],
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
