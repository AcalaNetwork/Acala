//! Mocks for the debit module.

#![cfg(test)]

use palette_support::{impl_outer_origin, parameter_types};
use sr_primitives::{testing::Header, traits::IdentityLookup, Fixed64, Perbill};
use su_primitives::H256;

use orml_traits::PriceProvider;
use support::RiskManager;

use super::*;

mod vaults {
	pub use crate::Event;
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
	pub const ExistentialDeposit: u64 = 0;
	pub const TransferFee: u64 = 0;
	pub const CreationFee: u64 = 2;
}

pub type AccountId = u32;
pub type BlockNumber = u64;
pub type Price = u64;
pub type Balance = u64;
pub type DebitBalance = u64;
pub type Amount = i64;
pub type DebitAmount = i64;
pub type CurrencyId = u32;
pub const ALICE: AccountId = 1;
pub const NATIVE_CURRENCY_ID: CurrencyId = 0;
pub const STABLE_COIN_ID: CurrencyId = 1;
pub const X_TOKEN_ID: CurrencyId = 2;
pub const Y_TOKEN_ID: CurrencyId = 3;

pub struct MockConvert;
impl Convert<(CurrencyId, DebitBalance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		(a.1 / DebitBalance::from(2u64)).into()
	}
}

impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
}

pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
}

pub type NativeCurrency = orml_currencies::NativeCurrencyOf<Runtime>;

impl orml_currencies::Trait for Runtime {
	type Event = ();
	type MultiCurrency = Tokens;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

pub type Currencies = orml_currencies::Module<Runtime>;

impl debits::Trait for Runtime {
	type Currency = NativeCurrency;
	type DebitBalance = DebitBalance;
	type CurrencyId = CurrencyId;
	type DebitAmount = DebitAmount;
	type Convert = MockConvert;
}

pub type DebitCurrency = debits::Module<Runtime>;

pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Amount, DebitAmount> for MockRiskManager {
	type Error = &'static str;
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
	fn required_collateral_ratio(currency_id: CurrencyId) -> Fixed64 {
		Fixed64::from_parts(1)
	}
	fn check_debit_cap(currency_id: CurrencyId, debits: DebitAmount) -> Result<(), Self::Error> {
		Ok(())
	}
}

pub struct MockPriceSource;
impl PriceProvider<CurrencyId, Price> for MockPriceSource {
	fn get_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		match (base, quote) {
			(1u32, 2u32) => Some(1u64),
			(STABLE_COIN_ID, Y_TOKEN_ID) => Some(2u64),
			_ => None,
		}
	}
}

impl system::Trait for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = ();
	type Hash = H256;
	type Hashing = ::sr_primitives::traits::BlakeTwo256;
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
pub type System = system::Module<Runtime>;

impl Trait for Runtime {
	type Event = ();
	type Convert = MockConvert;
	type Currency = Tokens;
	type DebitCurrency = DebitCurrency;
	type PriceSource = MockPriceSource;
	type RiskManager = MockRiskManager;

	type Price = Price;
}

pub type VaultsModule = Module<Runtime>;

pub struct ExtBuilder {
	currency_ids: Vec<CurrencyId>,
	endowed_accounts: Vec<AccountId>,
	initial_balance: Balance,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			currency_ids: vec![STABLE_COIN_ID, X_TOKEN_ID, Y_TOKEN_ID],
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
