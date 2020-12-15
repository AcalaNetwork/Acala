//! Mocks for the airdrop module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const CHARLIE: AccountId = 2;
pub const ACA: AirDropCurrencyId = AirDropCurrencyId::ACA;
pub const KAR: AirDropCurrencyId = AirDropCurrencyId::KAR;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod airdrop {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		airdrop<T>,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
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
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

impl Config for Runtime {
	type Event = TestEvent;
}
pub type Airdrop = Module<Runtime>;

pub struct ExtBuilder();

impl Default for ExtBuilder {
	fn default() -> Self {
		Self()
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		GenesisConfig::<Runtime> {
			airdrop_accounts: vec![(CHARLIE, KAR, 100), (CHARLIE, KAR, 50), (CHARLIE, ACA, 80)],
		}
		.assimilate_storage(&mut t)
		.unwrap();
		t.into()
	}
}
