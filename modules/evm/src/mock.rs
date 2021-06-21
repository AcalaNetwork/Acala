// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

#![cfg(test)]

use super::*;

use frame_support::{construct_runtime, ord_parameter_types, parameter_types};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{Amount, BlockNumber, CurrencyId, ReserveIdentifier, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	AccountId32,
};
use std::{collections::BTreeMap, str::FromStr};
use support::mocks::MockAddressMapping;

mod evm_mod {
	pub use super::super::*;
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u64>;
	type OnNewAccount = ();
	type OnKilledAccount = crate::CallKillAccount<Test>;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

parameter_types! {
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxReserves: u32 = 50;
}
impl pallet_balances::Config for Test {
	type Balance = u64;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinimumPeriod: u64 = 1000;
}
impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> u64 {
		Default::default()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = u64;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub struct GasToWeight;

impl Convert<u64, u64> for GasToWeight {
	fn convert(a: u64) -> u64 {
		a
	}
}

parameter_types! {
	pub NetworkContractSource: H160 = alice();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const NewContractExtraBytes: u32 = 100;
	pub const StorageDepositPerByte: u64 = 10;
	pub const DeveloperDeposit: u64 = 1000;
	pub const DeploymentFee: u64 = 200;
	pub const MaxCodeSize: u32 = 1000;
	pub const ChainId: u64 = 1;
}

impl Config for Test {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type MaxCodeSize = MaxCodeSize;

	type Event = Event;
	type Precompiles = ();
	type ChainId = ChainId;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = ();

	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type DeploymentFee = DeploymentFee;
	type TreasuryAccount = TreasuryAccount;
	type FreeDeploymentOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		EVM: evm_mod::{Pallet, Config<T>, Call, Storage, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Currencies: orml_currencies::{Pallet, Call, Event<T>},
	}
);

pub const INITIAL_BALANCE: u64 = 1_000_000_000_000;

pub fn contract_a() -> H160 {
	H160::from_str("2000000000000000000000000000000000000001").unwrap()
}

pub fn contract_b() -> H160 {
	H160::from_str("2000000000000000000000000000000000000002").unwrap()
}

pub fn alice() -> H160 {
	H160::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> H160 {
	H160::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn charlie() -> H160 {
	H160::from_str("1000000000000000000000000000000000000003").unwrap()
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	let mut accounts = BTreeMap::new();

	accounts.insert(
		contract_a(),
		GenesisAccount {
			nonce: 1,
			balance: Default::default(),
			storage: Default::default(),
			code: Default::default(),
		},
	);
	accounts.insert(
		contract_b(),
		GenesisAccount {
			nonce: 1,
			balance: Default::default(),
			storage: Default::default(),
			code: Default::default(),
		},
	);

	accounts.insert(
		alice(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: Default::default(),
		},
	);
	accounts.insert(
		bob(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: Default::default(),
		},
	);

	pallet_balances::GenesisConfig::<Test>::default()
		.assimilate_storage(&mut t)
		.unwrap();
	evm_mod::GenesisConfig::<Test> {
		accounts,
		treasury: Default::default(),
	}
	.assimilate_storage(&mut t)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| System::set_block_number(1));
	ext
}

pub fn balance(address: H160) -> u64 {
	let account_id = <Test as Config>::AddressMapping::get_account_id(&address);
	Balances::free_balance(account_id)
}

pub fn reserved_balance(address: H160) -> u64 {
	let account_id = <Test as Config>::AddressMapping::get_account_id(&address);
	Balances::reserved_balance(account_id)
}

#[cfg(not(feature = "with-ethereum-compatibility"))]
pub fn deploy_free(contract: H160) {
	let _ = EVM::deploy_free(Origin::signed(CouncilAccount::get()), contract);
}
