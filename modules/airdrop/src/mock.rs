//! Mocks for the airdrop module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use primitives::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};

mod airdrop {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		airdrop<T>,
	}
}

pub type AccountId = u64;
pub type BlockNumber = u64;
pub type Balance = u64;
pub type AirDropCurrencyId = u32;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const KAR: AirDropCurrencyId = 0;
pub const ACA: AirDropCurrencyId = 1;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
	pub const CreationFee: u64 = 2;
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

impl Trait for Runtime {
	type Event = TestEvent;
	type AirDropCurrencyId = AirDropCurrencyId;
	type Balance = Balance;
}
pub type Airdrop = Module<Runtime>;

pub struct ExtBuilder();

impl Default for ExtBuilder {
	fn default() -> Self {
		Self()
	}
}

impl ExtBuilder {
	pub fn build(self) -> runtime_io::TestExternalities {
		let t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
		t.into()
	}
}
