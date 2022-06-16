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

#![cfg(test)]

use super::*;

use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, FindAuthor, Nothing},
	ConsensusEngineId,
};
use frame_system::EnsureSignedBy;
use module_support::mocks::MockAddressMapping;
use orml_traits::parameter_type_with_key;
use primitives::{define_combined_task, Amount, BlockNumber, CurrencyId, ReserveIdentifier, TokenSymbol};
use sp_core::{H160, H256};
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	AccountId32,
};
use std::{collections::BTreeMap, str::FromStr};

type Balance = u128;

mod evm_mod {
	pub use super::super::*;
}

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
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
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = crate::CallKillAccount<Runtime>;
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

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
}

impl orml_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

define_combined_task! {
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<Runtime>),
	}
}

pub struct MockBlockNumberProvider;

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u32;

	fn current_block_number() -> Self::BlockNumber {
		Zero::zero()
	}
}

impl module_idle_scheduler::Config for Runtime {
	type Event = Event;
	type WeightInfo = ();
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = ConstU64<0>;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
}

pub struct GasToWeight;

impl Convert<u64, u64> for GasToWeight {
	fn convert(a: u64) -> u64 {
		a
	}
}

pub struct AuthorGiven;
impl FindAuthor<AccountId32> for AuthorGiven {
	fn find_author<'a, I>(_digests: I) -> Option<AccountId32>
	where
		I: 'a + IntoIterator<Item = (ConsensusEngineId, &'a [u8])>,
	{
		Some(<Runtime as Config>::AddressMapping::get_account_id(
			&H160::from_str("1234500000000000000000000000000000000000").unwrap(),
		))
	}
}

parameter_types! {
	pub NetworkContractSource: H160 = alice();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: Balance = convert_decimals_to_evm(10);
}

pub const NEW_CONTRACT_EXTRA_BYTES: u32 = 100;
pub const DEVELOPER_DEPOSIT: u128 = 1000;
pub const PUBLICATION_FEE: u128 = 200;
impl Config for Runtime {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = ConstU32<NEW_CONTRACT_EXTRA_BYTES>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<20_000_000>;

	type Event = Event;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_support::mocks::MockReservedTransactionPayment<Balances>;

	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = ConstU128<DEVELOPER_DEPOSIT>;
	type PublicationFee = ConstU128<PUBLICATION_FEE>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

	type Runner = crate::runner::stack::Runner<Self>;
	type FindAuthor = AuthorGiven;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Timestamp: pallet_timestamp,
		EVM: evm_mod,
		Tokens: orml_tokens,
		Balances: pallet_balances,
		Currencies: orml_currencies,
		IdleScheduler: module_idle_scheduler,
	}
);

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000_000;

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
	let mut t = frame_system::GenesisConfig::default()
		.build_storage::<Runtime>()
		.unwrap();

	let mut accounts = BTreeMap::new();

	accounts.insert(
		contract_a(),
		GenesisAccount {
			nonce: 1,
			..Default::default()
		},
	);
	accounts.insert(
		contract_b(),
		GenesisAccount {
			nonce: 1,
			..Default::default()
		},
	);

	accounts.insert(
		alice(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			..Default::default()
		},
	);
	accounts.insert(
		bob(),
		GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			..Default::default()
		},
	);

	pallet_balances::GenesisConfig::<Runtime> {
		balances: vec![(TreasuryAccount::get(), INITIAL_BALANCE)],
	}
	.assimilate_storage(&mut t)
	.unwrap();
	evm_mod::GenesisConfig::<Runtime> { chain_id: 1, accounts }
		.assimilate_storage(&mut t)
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(t);
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext
}

pub fn balance(address: H160) -> Balance {
	let account_id = <Runtime as Config>::AddressMapping::get_account_id(&address);
	Balances::free_balance(account_id)
}

pub fn eth_balance(address: H160) -> U256 {
	EVM::account_basic(&address).balance
}

pub fn reserved_balance(address: H160) -> Balance {
	let account_id = <Runtime as Config>::AddressMapping::get_account_id(&address);
	Balances::reserved_balance(account_id)
}

#[cfg(not(feature = "with-ethereum-compatibility"))]
pub fn publish_free(contract: H160) {
	let _ = EVM::publish_free(Origin::signed(CouncilAccount::get()), contract);
}
