//! Mocks for the auction manager module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use primitives::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use system::EnsureSignedBy;

use super::*;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

mod auction_manager {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		auction_manager<T>,
		orml_tokens<T>,
		orml_auction<T>,
		cdp_treasury<T>,
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
	pub const ExistentialDeposit: u64 = 0;
	pub const CreationFee: u64 = 2;
	pub const MinimumIncrementSize: Rate = Rate::from_rational(1, 20);
	pub const AuctionTimeToClose: u64 = 100;
	pub const AuctionDurationSoftCap: u64 = 2000;
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetAmountAdjustment: Rate = Rate::from_rational(1, 2);
}

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type AuctionId = u64;
pub type CurrencyId = u32;
pub type Balance = u64;
pub type Amount = i64;

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

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type ExistentialDeposit = ExistentialDeposit;
	type DustRemoval = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

impl orml_auction::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManagerModule;
}
pub type Auction = orml_auction::Module<Runtime>;

ord_parameter_types! {
	pub const One: AccountId = 1;
}

impl cdp_treasury::Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManagerModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type Dex = ();
}
pub type CdpTreasury = cdp_treasury::Module<Runtime>;

pub struct MockPriceSource;
impl PriceProvider<CurrencyId, Price> for MockPriceSource {
	#[allow(unused_variables)]
	fn get_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		Some(Price::from_natural(1))
	}

	#[allow(unused_variables)]
	fn lock_price(currency_id: CurrencyId) {}

	#[allow(unused_variables)]
	fn unlock_price(currency_id: CurrencyId) {}
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Tokens;
	type Auction = Auction;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = AuctionTimeToClose;
	type AuctionDurationSoftCap = AuctionDurationSoftCap;
	type GetStableCurrencyId = GetStableCurrencyId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Treasury = CdpTreasury;
	type GetAmountAdjustment = GetAmountAdjustment;
	type PriceSource = MockPriceSource;
}
pub type AuctionManagerModule = Module<Runtime>;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const ACA: CurrencyId = 0;
pub const AUSD: CurrencyId = 1;
pub const BTC: CurrencyId = 2;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, AUSD, 1000),
				(BOB, AUSD, 1000),
				(CAROL, AUSD, 1000),
				(ALICE, BTC, 1000),
				(BOB, BTC, 1000),
				(CAROL, BTC, 1000),
				(ALICE, ACA, 1000),
				(BOB, ACA, 1000),
				(CAROL, ACA, 1000),
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
