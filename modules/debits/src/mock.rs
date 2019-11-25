//! Mocks for the debit module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use primitives::H256;
use sr_primitives::{testing::Header, traits::IdentityLookup, Perbill};

use super::*;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

mod debits {}

impl_outer_event! {
	pub enum TestEvent for Runtime {

	}
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

pub type AccountId = u64;
type BlockNumber = u64;

pub type Balance = u64;
pub type DebitBalance = u32;
pub type Amount = i64;
pub type CurrencyId = u32;

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
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
}
pub type System = system::Module<Runtime>;

impl Trait for Runtime {
	type CurrencyId = CurrencyId;
	type Currency = CurrencyHandler;
	type DebitBalance = DebitBalance;
	type Convert = ConvertHandler;
	type DebitAmount = Amount;
}

pub type DebitsModule = Module<Runtime>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const USD: CurrencyId = 1;

pub struct CurrencyHandler;

impl BasicCurrency<AccountId> for CurrencyHandler {
	type Balance = Balance;
	type Error = &'static str;

	fn total_issuance() -> Self::Balance {
		Self::Balance::default()
	}

	fn balance(_who: &AccountId) -> Self::Balance {
		Self::Balance::default()
	}

	fn transfer(_from: &AccountId, _to: &AccountId, _amount: Self::Balance) -> result::Result<(), Self::Error> {
		Ok(())
	}

	fn deposit(_who: &AccountId, _amount: Self::Balance) -> result::Result<(), Self::Error> {
		Ok(())
	}

	fn withdraw(_who: &AccountId, _amount: Self::Balance) -> result::Result<(), Self::Error> {
		Ok(())
	}

	fn slash(_who: &AccountId, _amount: Self::Balance) -> Self::Balance {
		Self::Balance::default()
	}
}

impl BasicCurrencyExtended<AccountId> for CurrencyHandler {
	type Amount = Amount;

	fn update_balance(_who: &AccountId, _by_amount: Self::Amount) -> result::Result<(), Self::Error> {
		Ok(())
	}
}

pub struct ConvertHandler;

impl Convert<(CurrencyId, Balance), DebitBalance> for ConvertHandler {
	fn convert(a: (CurrencyId, Balance)) -> DebitBalance {
		let balance: u64 = (a.1 * Balance::from(2u64)).into();
		let debit_balance = balance as u32;
		debit_balance
	}
}

impl Convert<(CurrencyId, DebitBalance), Balance> for ConvertHandler {
	fn convert(a: (CurrencyId, DebitBalance)) -> Balance {
		let debit_balance: u32 = (a.1 / DebitBalance::from(2u32)).into();
		let balance = debit_balance as u64;
		balance
	}
}

pub struct ExtBuilder;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		t.into()
	}
}
