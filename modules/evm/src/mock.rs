// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, FindAuthor, Nothing},
	ConsensusEngineId,
};
use frame_system::EnsureSignedBy;
use module_support::mocks::{MockAddressMapping, TestRandomness};
use orml_traits::parameter_type_with_key;
use primitives::{define_combined_task, Amount, BlockNumber, CurrencyId, ReserveIdentifier, TokenSymbol};
use sp_core::{bytes::from_hex, H160};
use sp_runtime::{
	traits::{BlockNumberProvider, IdentityLookup},
	AccountId32, BuildStorage,
};
use std::{collections::BTreeMap, str::FromStr};

type Balance = u128;

pub mod evm_mod {
	pub use super::super::*;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<2>;
	type AccountStore = module_support::SystemAccountStore<Runtime>;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
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
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ReserveIdentifier;
	type DustRemovalWhitelist = Nothing;
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

parameter_types! {
	pub MinimumWeightRemainInBlock: Weight = Weight::zero();
}

impl module_idle_scheduler::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
}

impl pallet_utility::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

pub struct GasToWeight;

impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> Weight {
		Weight::from_parts(a, 0)
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

	type RuntimeEvent = RuntimeEvent;
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
	type Randomness = TestRandomness<Self>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Timestamp: pallet_timestamp,
		EVM: evm_mod,
		Tokens: orml_tokens,
		Balances: pallet_balances,
		Currencies: orml_currencies,
		IdleScheduler: module_idle_scheduler,
		Utility: pallet_utility,
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
	let mut t = frame_system::GenesisConfig::<Runtime>::default()
		.build_storage()
		.unwrap();

	let mut accounts = BTreeMap::new();

	// pragma solidity >=0.8.2 <0.9.0;
	// contract Test {}
	let contract = from_hex(
		"0x6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220199b6fd928fecd2e7ce866eb76c49927191c7a839fd75192acc84b773e5dbf1e64736f6c63430008120033"
	).unwrap();

	accounts.insert(
		contract_a(),
		GenesisAccount {
			nonce: 1,
			code: contract.clone(),
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
