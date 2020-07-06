//! Mocks for nominees election module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_origin, parameter_types};
use primitives::{Amount, CurrencyId};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const ACA: CurrencyId = CurrencyId::ACA;
pub const LDOT: CurrencyId = CurrencyId::LDOT;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

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
	type Event = ();
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
pub type System = system::Module<Runtime>;

impl orml_tokens::Trait for Runtime {
	type Event = ();
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
}
pub type TokensModule = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
}
type PalletBalances = pallet_balances::Module<Runtime>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub const GetLDOTCurrencyId: CurrencyId = LDOT;
}

pub type NativeCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Balance>;
pub type LDOTCurrency = orml_currencies::Currency<Runtime, GetLDOTCurrencyId>;

impl orml_currencies::Trait for Runtime {
	type Event = ();
	type MultiCurrency = TokensModule;
	type NativeCurrency = NativeCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
}

parameter_types! {
	pub const MinBondThreshold: Balance = 5;
	pub const BondingDuration: EraIndex = 4;
	pub const NominateesCount: usize = 5;
	pub const MaxUnlockingChunks: usize = 3;
}

impl Trait for Runtime {
	type Currency = LDOTCurrency;
	type PolkadotAccountId = AccountId;
	type MinBondThreshold = MinBondThreshold;
	type BondingDuration = BondingDuration;
	type NominateesCount = NominateesCount;
	type MaxUnlockingChunks = MaxUnlockingChunks;
}
pub type NomineesElectionModule = Module<Runtime>;

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, LDOT, 1000), (BOB, LDOT, 1000)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = system::GenesisConfig::default().build_storage::<Runtime>().unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
