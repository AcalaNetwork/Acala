//! Mocks for the debit module.

#![cfg(test)]

use frame_support::{impl_outer_origin, parameter_types};
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use su_primitives::H256;
use support::RiskManager;

use super::*;

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

// mock convert
pub struct MockConvert;
impl Convert<(CurrencyId, DebitBalance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		(a.1 / DebitBalance::from(2u64)).into()
	}
}

// tokens module
impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
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
	type TransferPayment = ();
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type TransferFee = TransferFee;
	type CreationFee = CreationFee;
}

pub type PalletBalances = pallet_balances::Module<Runtime>;
pub type AdaptedBasicCurrency =
	orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance, orml_tokens::Error>;

impl orml_currencies::Trait for Runtime {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type Currencies = orml_currencies::Module<Runtime>;

impl debits::Trait for Runtime {
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type DebitBalance = DebitBalance;
	type CurrencyId = CurrencyId;
	type DebitAmount = DebitAmount;
	type Convert = MockConvert;
}

// debit module
pub type DebitCurrency = debits::Module<Runtime>;

// mock risk manager
pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Amount, DebitAmount> for MockRiskManager {
	type Error = &'static str;
	#[allow(unused_variables)]
	fn check_position_adjustment(
		account_id: &AccountId,
		currency_id: CurrencyId,
		collaterals: Amount,
		debits: DebitAmount,
	) -> Result<(), Self::Error> {
		match currency_id {
			2u32 => Err("mock error"),
			3u32 => Ok(()),
			_ => Err("mock error"),
		}
	}
	#[allow(unused_variables)]
	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> Result<(), Self::Error> {
		match (currency_id, debits) {
			(2u32, 1000i64) => Err("mock error"),
			(3u32, 1000i64) => Err("mock error"),
			(_, _) => Ok(()),
		}
	}
}

impl Trait for Runtime {
	type Event = ();
	type Convert = MockConvert;
	type Currency = Currencies;
	type DebitCurrency = DebitCurrency;
	type RiskManager = MockRiskManager;
}

pub type VaultsModule = Module<Runtime>;

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
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
}

pub struct ExtBuilder {
	currency_ids: Vec<CurrencyId>,
	endowed_accounts: Vec<AccountId>,
	initial_balance: Balance,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			currency_ids: vec![X_TOKEN_ID, Y_TOKEN_ID],
			endowed_accounts: vec![ALICE],
			initial_balance: 1000,
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
		orml_tokens::GenesisConfig::<Runtime> {
			tokens: self.currency_ids,
			initial_balance: self.initial_balance,
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}
}
