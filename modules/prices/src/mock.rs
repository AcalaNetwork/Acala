//! Mocks for the prices module.

#![cfg(test)]

use frame_support::{impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use primitives::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};
use support::ExchangeRate;
use system::EnsureSignedBy;

use super::*;

mod prices {
	pub use super::super::*;
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		prices<T>,
	}
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type CurrencyId = u32;

pub const AUSD: CurrencyId = 1;
pub const BTC: CurrencyId = 2;
pub const DOT: CurrencyId = 3;
pub const OTHER: CurrencyId = 4;
pub const ETH: CurrencyId = 5;
pub const LDOT: CurrencyId = 6;

// Workaround for https://github.com/rust-lang/rust/issues/26925 . Remove when sorted.
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
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
}
pub type System = system::Module<Runtime>;

pub struct MockDataProvider;
impl DataProvider<CurrencyId, Price> for MockDataProvider {
	fn get(currency: &CurrencyId) -> Option<Price> {
		match currency {
			&AUSD => Some(Price::from_rational(99, 100)),
			&BTC => Some(Price::from_natural(5000)),
			&DOT => Some(Price::from_natural(100)),
			&OTHER => Some(Price::from_natural(0)),
			_ => None,
		}
	}
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::from_rational(1, 2)
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub const StableCurrencyFixedPrice: Price = Price::from_natural(1);
}

impl Trait for Runtime {
	type Event = TestEvent;
	type CurrencyId = CurrencyId;
	type Source = MockDataProvider;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureSignedBy<One, AccountId>;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
}
pub type PricesModule = Module<Runtime>;

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
