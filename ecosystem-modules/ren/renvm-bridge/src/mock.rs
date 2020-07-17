//! Mocks for the airdrop module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use orml_currencies::BasicCurrencyAdapter;
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};

pub type AccountId = H256;
pub type BlockNumber = u64;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod renvm {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		system<T>,
		pallet_balances<T>,
		renvm<T>,
	}
}

pub type RenvmBridgeCall = super::Call<Runtime>;

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

parameter_types! {
	pub const ExistentialDeposit: Balance = 0;
	pub const RenVmPublicKey: [u8; 20] = hex_literal::hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"];
	pub const RENBTCIdentifier: [u8; 32] = hex_literal::hex!["0000000000000000000000000a9add98c076448cbcfacf5e457da12ddbef4a8f"];
}

parameter_types! {
	pub const GetNativeCurrencyId: u8 = 0;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
}
pub type Balances = pallet_balances::Module<Runtime>;

parameter_types! {
	pub const UnsignedPriority: u64 = 1 << 20;
}

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = BasicCurrencyAdapter<Balances, Balance, Balance, i128, BlockNumber>;
	type PublicKey = RenVmPublicKey;
	type CurrencyIdentifier = RENBTCIdentifier;
	type UnsignedPriority = UnsignedPriority;
}
pub type RenVmBridge = Module<Runtime>;

pub struct ExtBuilder();

impl Default for ExtBuilder {
	fn default() -> Self {
		Self()
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();
		t.into()
	}
}
