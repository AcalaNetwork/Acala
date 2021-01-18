//! Mocks for example module.

#![cfg(test)]

use crate as example;
use frame_support::pallet_prelude::GenesisBuild;
use frame_support::{construct_runtime, parameter_types};
use sp_runtime::traits::Block as BlockT;

parameter_types!(
	pub const SomeConst: u64 = 10;
	pub const BlockHashCount: u32 = 250;
);

impl frame_system::Config for Runtime {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u32;
	type Call = Call;
	type Hash = sp_runtime::testing::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = u64;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
}

impl example::Config for Runtime {
	type Event = Event;
	type SomeConst = SomeConst;
	type Balance = u64;
}

pub type Header = sp_runtime::generic::Header<u32, sp_runtime::traits::BlakeTwo256>;
pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, Call, (), ()>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Module, Call, Event<T>},
		// NOTE: name Example here is needed in order to have same module prefix
		Example: example::{Module, Call, Event<T>, Config<T>, Storage},
	}
);

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();
	example::GenesisConfig::<Runtime> {
		bar: vec![(1, 100), (2, 200)],
		..Default::default()
	}
	.assimilate_storage(&mut t)
	.unwrap();
	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}
