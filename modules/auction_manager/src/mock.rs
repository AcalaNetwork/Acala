//! Mocks for the auction_manager module.

#![cfg(test)]

use primitives::H256;
use sr_primitives::{testing::Header, traits::IdentityLookup, Perbill, Permill};
use srml_support::{impl_outer_event, impl_outer_origin, parameter_types};

use super::*;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

mod auction_manager {
	pub use crate::Event;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		auction_manager<T>,
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
	pub const MinimumIncrementSize: Permill = Permill::from_percent(5);
	pub const AuctionTimeToClose: u64 = 100;
	pub const AuctionDurationSoftCap: u64 = 2000;
	pub const GetNativeCurrencyId: u32 = 1;
}

pub type AccountId = u64;
type BlockNumber = u64;
pub type AuctionId = u64;
pub type CurrencyId = u32;
pub type Balance = u64;

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

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
}
pub type Tokens = orml_tokens::Module<Runtime>;

impl orml_auction::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManagerModule;
}
pub type Auction = orml_auction::Module<Runtime>;

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type Auction = Auction;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}
pub type AuctionManagerModule = Module<Runtime>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const AUSD: CurrencyId = 1;
pub const BTC: CurrencyId = 2;

pub struct ExtBuilder {
	currency_id: Vec<CurrencyId>,
	endowed_accounts: Vec<AccountId>,
	initial_balance: Balance,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			currency_id: vec![AUSD, BTC],
			endowed_accounts: vec![0],
			initial_balance: 0,
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			tokens: vec![AUSD, BTC],
			initial_balance: 1000,
			endowed_accounts: vec![ALICE, BOB],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
