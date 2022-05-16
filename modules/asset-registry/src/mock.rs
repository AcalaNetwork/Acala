// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Mocks for asset registry module.

#![cfg(test)]

use crate as asset_registry;
use frame_support::{
	assert_ok, construct_runtime, ord_parameter_types,
	pallet_prelude::GenesisBuild,
	parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything},
};
use frame_system::EnsureSignedBy;
use module_support::{mocks::MockAddressMapping, AddressMapping};
use primitives::{
	evm::convert_decimals_to_evm, evm::EvmAddress, AccountId, Balance, CurrencyId, ReserveIdentifier, TokenSymbol,
};
use sp_core::{H160, H256, U256};
use std::str::FromStr;

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = u64;
	type Call = Call;
	type Hash = sp_runtime::testing::H256;
	type Hashing = sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = sp_runtime::traits::IdentityLookup<Self::AccountId>;
	type Header = sp_runtime::testing::Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
}

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1000>;
	type WeightInfo = ();
}

parameter_types! {
	pub NetworkContractSource: EvmAddress = alice_evm_addr();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId = AccountId::from([1u8; 32]);
	pub const TreasuryAccount: AccountId = AccountId::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId = AccountId::from([0u8; 32]);
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
}

impl module_evm::Config for Runtime {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type TransferAll = ();
	type NewContractExtraBytes = ConstU32<1>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<10>;
	type Event = Event;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = ();
	type ChargeTransactionPayment = module_support::mocks::MockReservedTransactionPayment<Balances>;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;

	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId>;

	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = ();
	type Task = ();
	type IdleScheduler = ();
	type WeightInfo = ();
}

impl module_evm_bridge::Config for Runtime {
	type EVM = EVM;
}

parameter_types! {
	pub const KSMCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);
}
impl asset_registry::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type StakingCurrencyId = KSMCurrencyId;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type RegisterOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		AssetRegistry: asset_registry::{Pallet, Call, Event<T>, Storage},
		EVM: module_evm::{Pallet, Config<T>, Call, Storage, Event<T>},
		EVMBridge: module_evm_bridge::{Pallet},
	}
);

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
}

pub fn erc20_address_same_prefix() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba644").unwrap()
}

pub fn erc20_address_not_exists() -> EvmAddress {
	EvmAddress::from_str("0000000000000000000100000000000002000001").unwrap()
}

pub fn alice() -> AccountId {
	<Runtime as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub const ALICE_BALANCE: u128 = 100_000_000_000_000_000_000_000u128;

pub fn deploy_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();
	assert_ok!(EVM::create(Origin::signed(alice()), code, 0, 2_100_000, 10000, vec![]));

	System::assert_last_event(Event::EVM(module_evm::Event::Created {
		from: alice_evm_addr(),
		contract: erc20_address(),
		logs: vec![module_evm::Log {
			address: H160::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap(),
			topics: vec![
				H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").unwrap(),
				H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
				H256::from_str("0x0000000000000000000000001000000000000000000000000000000000000001").unwrap(),
			],
			data: {
				let mut buf = [0u8; 32];
				U256::from(ALICE_BALANCE).to_big_endian(&mut buf);
				H256::from_slice(&buf).as_bytes().to_vec()
			},
		}],
		used_gas: 1306611,
		used_storage: 5462,
	}));

	assert_ok!(EVM::publish_free(
		Origin::signed(CouncilAccount::get()),
		erc20_address()
	));
}

// Specify contract address
pub fn deploy_contracts_same_prefix() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();
	assert_ok!(EVM::create_predeploy_contract(
		Origin::signed(NetworkContractAccount::get()),
		erc20_address_same_prefix(),
		code,
		0,
		2_100_000,
		10000,
		vec![]
	));

	System::assert_has_event(Event::EVM(module_evm::Event::Created {
		from: alice_evm_addr(),
		contract: erc20_address_same_prefix(),
		logs: vec![module_evm::Log {
			address: erc20_address_same_prefix(),
			topics: vec![
				H256::from_str("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").unwrap(),
				H256::from_str("0x0000000000000000000000000000000000000000000000000000000000000000").unwrap(),
				H256::from_str("0x0000000000000000000000001000000000000000000000000000000000000001").unwrap(),
			],
			data: {
				let mut buf = [0u8; 32];
				U256::from(ALICE_BALANCE).to_big_endian(&mut buf);
				H256::from_slice(&buf).as_bytes().to_vec()
			},
		}],
		used_gas: 1306611,
		used_storage: 5462,
	}));

	System::assert_last_event(Event::EVM(module_evm::Event::ContractPublished {
		contract: erc20_address_same_prefix(),
	}));
}

pub struct ExtBuilder {
	balances: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		asset_registry::GenesisConfig::<Runtime> {
			assets: vec![(CurrencyId::Token(TokenSymbol::ACA), 1)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.balances.into_iter().collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_evm::GenesisConfig::<Runtime>::default()
			.assimilate_storage(&mut t)
			.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
