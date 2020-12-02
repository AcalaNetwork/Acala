//! Mocks for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_event, impl_outer_origin, parameter_types};
use orml_traits::parameter_type_with_key;
use primitives::{Amount, Balance, CurrencyId, TokenSymbol};
use sp_core::H256;
use sp_io::hashing::keccak_256;
use sp_runtime::{testing::Header, traits::IdentityLookup, Perbill};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

parameter_types! {
	pub ALICE: AccountId = AccountId32::from([0u8; 32]);
	pub BOB: AccountId = AccountId32::from([1u8; 32]);
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

mod evm_accounts {
	pub use super::super::*;
}

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

impl_outer_event! {
	pub enum TestEvent for Runtime {
		frame_system<T>,
		pallet_balances<T>,
		evm_accounts<T>,
		orml_tokens<T>,
		orml_currencies<T>,
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: u32 = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Trait for Runtime {
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
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = ();
	type BaseCallFilter = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}
impl pallet_balances::Trait for Runtime {
	type Balance = Balance;
	type Event = TestEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Module<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
}
pub type Balances = pallet_balances::Module<Runtime>;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Trait for Runtime {
	type Event = TestEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}
pub type Tokens = orml_tokens::Module<Runtime>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Trait for Runtime {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Runtime>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

impl Trait for Runtime {
	type Event = TestEvent;
	type Currency = Balances;
	type AddressMapping = EvmAddressMapping<Runtime>;
	type MergeAccount = Currencies;
	type KillAccount = ();
	type WeightInfo = ();
}
pub type EvmAccountsModule = Module<Runtime>;

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

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(bob_account_id(), 100000)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

pub fn alice() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

pub fn bob() -> secp256k1::SecretKey {
	secp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

pub fn bob_account_id() -> AccountId {
	let address = EvmAccountsModule::eth_address(&bob());
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId32::from(Into::<[u8; 32]>::into(data))
}
