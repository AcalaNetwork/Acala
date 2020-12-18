//! Mocks for the evm-bridge module.

#![cfg(test)]

use super::*;
use frame_support::{impl_outer_origin, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use module_evm::GenesisAccount;
use primitives::mocks::MockAddressMapping;
use sp_core::{bytes::from_hex, crypto::AccountId32, H256};
use sp_runtime::{testing::Header, traits::IdentityLookup};
use sp_std::{collections::btree_map::BTreeMap, str::FromStr};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type Balance = u128;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Runtime;

impl_outer_origin! {
	pub enum Origin for Runtime {}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Runtime {
	type BaseCallFilter = ();
	type Origin = Origin;
	type Call = ();
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = ();
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = ();
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
}
pub type System = frame_system::Module<Runtime>;

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
}

pub type Balances = pallet_balances::Module<Runtime>;

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}
impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_types! {
	pub const ContractExistentialDeposit: u64 = 1;
	pub const TransferMaintainerDeposit: u64 = 1;
	pub NetworkContractSource: H160 = alice();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: u64 = 10;
	pub const StorageDefaultQuota: u32 = 0x6000;
	pub const DeveloperDeposit: u64 = 1000;
	pub const DeploymentFee: u64 = 200;
}

impl module_evm::Config for Runtime {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type MergeAccount = ();
	type ContractExistentialDeposit = ContractExistentialDeposit;
	type TransferMaintainerDeposit = TransferMaintainerDeposit;
	type StorageDepositPerByte = StorageDepositPerByte;
	type StorageDefaultQuota = StorageDefaultQuota;

	type Event = ();
	type Precompiles = ();
	type ChainId = ();
	type GasToWeight = ();
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;

	type DeveloperDeposit = DeveloperDeposit;
	type DeploymentFee = DeploymentFee;
	type TreasuryAccount = ();
	type FreeDeploymentOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

	type WeightInfo = ();
}

pub type EVM = module_evm::Module<Runtime>;

impl Config for Runtime {
	type EVM = EVM;
}
pub type EvmBridgeModule = Module<Runtime>;

pub struct ExtBuilder();

impl Default for ExtBuilder {
	fn default() -> Self {
		Self()
	}
}

pub fn erc20_address() -> H160 {
	H160::from_str("2000000000000000000000000000000000000001").unwrap()
}

pub fn alice() -> H160 {
	H160::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> H160 {
	H160::from_str("1000000000000000000000000000000000000002").unwrap()
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		let mut accounts = BTreeMap::new();
		let mut storage = BTreeMap::new();
		storage.insert(
			H256::from_str("0000000000000000000000000000000000000000000000000000000000000002").unwrap(),
			H256::from_str("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap(),
		);
		storage.insert(
			H256::from_str("e6f18b3f6d2cdeb50fb82c61f7a7a249abf7b534575880ddcfde84bba07ce81d").unwrap(),
			H256::from_str("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap(),
		);
		accounts.insert(
			erc20_address(),
			GenesisAccount {
				nonce: 1,
				balance: 0,
				storage,
				code: from_hex(include!("./erc20_demo_contract")).unwrap(),
			},
		);
		module_evm::GenesisConfig::<Runtime> {
			accounts,
			network_contract_index: 2048,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
