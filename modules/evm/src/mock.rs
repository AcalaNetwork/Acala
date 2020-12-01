#![cfg(test)]

use super::*;

use frame_support::{impl_outer_dispatch, impl_outer_event, impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use primitives::{Amount, BlockNumber, CurrencyId, TokenSymbol};
use sp_core::{Blake2Hasher, Hasher, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32, Perbill,
};
use std::{collections::BTreeMap, str::FromStr};

/// Hashed address mapping.
pub struct HashedAddressMapping<H>(sp_std::marker::PhantomData<H>);

impl<H: Hasher<Out = H256>> AddressMapping<AccountId32> for HashedAddressMapping<H> {
	fn to_account(address: &H160) -> AccountId32 {
		let mut data = [0u8; 24];
		data[0..4].copy_from_slice(b"evm:");
		data[4..24].copy_from_slice(&address[..]);
		let hash = H::hash(&data);

		AccountId32::from(Into::<[u8; 32]>::into(hash))
	}

	fn to_evm_address(_account: &AccountId32) -> Option<H160> {
		Some(H160::default())
	}
}

impl_outer_origin! {
	pub enum Origin for Test where system = frame_system {}
}

impl_outer_dispatch! {
	pub enum OuterCall for Test where origin: Origin {
		self::EVM,
	}
}

mod evm_mod {
	pub use crate::Event;
}
impl_outer_event! {
	pub enum TestEvent for Test {
		frame_system<T>,
		pallet_balances<T>,
		orml_tokens<T>,
		orml_currencies<T>,
		evm_mod<T>,
	}
}

#[derive(Clone, Eq, PartialEq)]
pub struct Test;
parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = 1024;
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}
impl frame_system::Trait for Test {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = OuterCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = TestEvent;
	type BlockHashCount = BlockHashCount;
	type MaximumBlockWeight = MaximumBlockWeight;
	type DbWeight = ();
	type BlockExecutionWeight = ();
	type ExtrinsicBaseWeight = ();
	type MaximumExtrinsicWeight = MaximumBlockWeight;
	type MaximumBlockLength = MaximumBlockLength;
	type AvailableBlockRatio = AvailableBlockRatio;
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const ContractExistentialDeposit: u64 = 1;
}
impl pallet_balances::Trait for Test {
	type Balance = u64;
	type DustRemoval = ();
	type Event = TestEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}
impl pallet_timestamp::Trait for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

impl orml_tokens::Trait for Test {
	type Event = TestEvent;
	type Balance = u64;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type OnReceived = ();
	type WeightInfo = ();
}
pub type Tokens = orml_tokens::Module<Test>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Trait for Test {
	type Event = TestEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type Currencies = orml_currencies::Module<Test>;
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub struct GasToWeight;

impl Convert<u32, u64> for GasToWeight {
	fn convert(a: u32) -> u64 {
		a as u64
	}
}

parameter_types! {
	pub NetworkContractSource: H160 = alice();
}

ord_parameter_types! {
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
}

impl Trait for Test {
	type AddressMapping = HashedAddressMapping<Blake2Hasher>;
	type Currency = Balances;
	type MergeAccount = Currencies;
	type ContractExistentialDeposit = ContractExistentialDeposit;

	type Event = Event<Test>;
	type Precompiles = ();
	type ChainId = SystemChainId;
	type Runner = crate::runner::native::Runner<Self>;
	type GasToWeight = GasToWeight;

	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;
}

pub type System = frame_system::Module<Test>;
pub type Balances = pallet_balances::Module<Test>;
pub type EVM = Module<Test>;

pub const INITIAL_BALANCE: u64 = 1_000_000_000_000;

pub fn alice() -> H160 {
	H160::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> H160 {
	H160::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn charlie() -> H160 {
	H160::from_str("1000000000000000000000000000000000000003").unwrap()
}

pub const NETWORK_CONTRACT_INDEX: u64 = 2048;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	let mut accounts = BTreeMap::new();
	accounts.insert(
		alice(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: vec![
				0x00, // STOP
			],
		},
	);
	accounts.insert(
		bob(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: vec![
				0xff, // INVALID
			],
		},
	);

	pallet_balances::GenesisConfig::<Test>::default()
		.assimilate_storage(&mut t)
		.unwrap();
	GenesisConfig::<Test> {
		accounts,
		network_contract_index: NETWORK_CONTRACT_INDEX,
	}
	.assimilate_storage(&mut t)
	.unwrap();
	t.into()
}

pub fn balance(address: H160) -> u64 {
	let account_id = <Test as Trait>::AddressMapping::to_account(&address);
	Balances::free_balance(account_id)
}

pub fn reserved_balance(address: H160) -> u64 {
	let account_id = <Test as Trait>::AddressMapping::to_account(&address);
	Balances::reserved_balance(account_id)
}
