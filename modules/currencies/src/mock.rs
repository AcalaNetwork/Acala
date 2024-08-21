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

//! Mocks for the currencies module.

#![cfg(test)]

use super::*;
pub use crate as currencies;

use frame_support::{
	assert_ok, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Nothing, VariantCount},
	PalletId,
};
use frame_system::EnsureSignedBy;
use module_support::{
	mocks::{MockAddressMapping, TestRandomness},
	AddressMapping,
};
use orml_traits::{currency::MutationHooks, parameter_type_with_key};
use primitives::{evm::convert_decimals_to_evm, CurrencyId, ReserveIdentifier, TokenSymbol};
use sp_core::H256;
use sp_core::{H160, U256};
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, IdentityLookup},
	AccountId32, BuildStorage,
};
use sp_std::str::FromStr;

pub const CHARLIE: AccountId = AccountId32::new([6u8; 32]);
pub const DAVE: AccountId = AccountId32::new([7u8; 32]);
pub const EVE: AccountId = AccountId32::new([8u8; 32]);
pub const FERDIE: AccountId = AccountId32::new([9u8; 32]);

pub type AccountId = AccountId32;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
}

type Balance = u128;

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: CurrencyId| -> Balance {
		if *currency_id == DOT { return 2; }
		Default::default()
	};
}

parameter_types! {
	pub DustAccount: AccountId = PalletId(*b"orml/dst").into_account_truncating();
}

pub struct CurrencyHooks<T>(marker::PhantomData<T>);
impl<T: orml_tokens::Config> MutationHooks<T::AccountId, T::CurrencyId, T::Balance> for CurrencyHooks<T>
where
	T::AccountId: From<AccountId>,
{
	type OnDust = orml_tokens::TransferDust<T, DustAccount>;
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i64;
	type CurrencyId = CurrencyId;
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = CurrencyHooks<Runtime>;
	type WeightInfo = ();
	type MaxLocks = ConstU32<100>;
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

pub const NATIVE_CURRENCY_ID: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const X_TOKEN_ID: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = NATIVE_CURRENCY_ID;
}

#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, MaxEncodedLen, TypeInfo, RuntimeDebug)]
pub enum TestId {
	Foo,
}

impl VariantCount for TestId {
	const VARIANT_COUNT: u32 = 1;
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
	type RuntimeHoldReason = TestId;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

pub type PalletBalances = pallet_balances::Pallet<Runtime>;

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1000>;
	type WeightInfo = ();
}

parameter_types! {
	pub NetworkContractSource: H160 = alice_evm_addr();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
	pub const TxFeePerGas: u128 = 10;
	pub const DeveloperDeposit: u64 = 1000;
	pub const PublicationFee: u64 = 200;
}

pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> Weight {
		Weight::from_parts(a, 0)
	}
}

impl module_evm::Config for Runtime {
	type AddressMapping = MockAddressMapping;
	type Currency = PalletBalances;
	type TransferAll = ();
	type NewContractExtraBytes = ConstU32<1>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = TxFeePerGas;
	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_support::mocks::MockReservedTransactionPayment<Balances>;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;

	type DeveloperDeposit = DeveloperDeposit;
	type PublicationFee = PublicationFee;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId32>;

	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = ();
	type Randomness = TestRandomness<Self>;
	type Task = ();
	type IdleScheduler = ();
	type WeightInfo = ();
}

impl module_evm_bridge::Config for Runtime {
	type EVM = EVM;
}

parameter_types! {
	pub Erc20HoldingAccount: H160 = primitives::evm::ERC20_HOLDING_ACCOUNT;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type GasToWeight = GasToWeight;
	type SweepOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type OnDust = crate::TransferDust<Runtime, DustAccount>;
}

pub type NativeCurrency = Currency<Runtime, GetNativeCurrencyId>;
pub type AdaptedBasicCurrency = BasicCurrencyAdapter<Runtime, PalletBalances, i64, u64>;

pub type SignedExtra = module_evm::SetEvmOrigin<Runtime>;

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, RuntimeCall, u32, SignedExtra>;

frame_support::construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: currencies,
		EVM: module_evm,
		EVMBridge: module_evm_bridge,
	}
);

pub fn alice() -> AccountId {
	<Runtime as Config>::AddressMapping::get_account_id(&alice_evm_addr())
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> AccountId {
	<Runtime as Config>::AddressMapping::get_account_id(&bob_evm_addr())
}

pub fn bob_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn eva() -> AccountId {
	<Runtime as Config>::AddressMapping::get_account_id(&eva_evm_addr())
}

pub fn eva_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000005").unwrap()
}

pub const ID_1: LockIdentifier = *b"1       ";

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
}

pub fn erc20_address_not_exist() -> EvmAddress {
	EvmAddress::from_str("0x00ddfce53ee040d9eb21afbc0ae1bb4dbb0ba600").unwrap()
}

pub const ALICE_BALANCE: u128 = 100_000_000_000_000_000_000_000u128;

pub fn deploy_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();
	assert_ok!(EVM::create(
		RuntimeOrigin::signed(alice()),
		code,
		0,
		2_100_000,
		10_000,
		vec![]
	));

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
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
		used_gas: 1013342,
		used_storage: 4028,
	}));
}

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self { balances: vec![] }
	}
}

impl ExtBuilder {
	pub fn balances(mut self, balances: Vec<(AccountId, CurrencyId, Balance)>) -> Self {
		self.balances = balances;
		self
	}

	pub fn one_hundred_for_alice_n_bob(self) -> Self {
		self.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100),
			(bob(), NATIVE_CURRENCY_ID, 100),
			(alice(), X_TOKEN_ID, 100),
			(bob(), X_TOKEN_ID, 100),
		])
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.clone()
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id == NATIVE_CURRENCY_ID)
				.map(|(account_id, _, initial_balance)| (account_id, initial_balance))
				.collect::<Vec<_>>(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self
				.balances
				.into_iter()
				.filter(|(_, currency_id, _)| *currency_id != NATIVE_CURRENCY_ID)
				.collect::<Vec<_>>(),
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
