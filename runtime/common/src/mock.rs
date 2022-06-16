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

use codec::{Decode, Encode};
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, FindAuthor, Nothing},
	weights::Weight,
	ConsensusEngineId, RuntimeDebug,
};
use module_evm::{EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_support::{mocks::MockAddressMapping, DispatchableTask};
use orml_traits::parameter_type_with_key;
use primitives::{
	define_combined_task, evm::convert_decimals_to_evm, task::TaskResult, Amount, BlockNumber, CurrencyId,
	ReserveIdentifier, TokenSymbol,
};
use scale_info::TypeInfo;
use sp_core::{H160, H256};
use sp_runtime::traits::Convert;
pub use sp_runtime::AccountId32;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup, Zero},
};
use std::str::FromStr;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<TestRuntime>;
type Block = frame_system::mocking::MockBlock<TestRuntime>;

type Balance = u128;

impl frame_system::Config for TestRuntime {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = primitives::Nonce;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<10>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = (
		module_evm::CallKillAccount<TestRuntime>,
		module_evm_accounts::CallKillAccount<TestRuntime>,
	);
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for TestRuntime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
}

impl pallet_timestamp::Config for TestRuntime {
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

impl orml_tokens::Config for TestRuntime {
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

impl orml_currencies::Config for TestRuntime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<TestRuntime, Balances, Amount, BlockNumber>;

define_combined_task! {
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<TestRuntime>),
	}
}

pub struct MockBlockNumberProvider;

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u32;

	fn current_block_number() -> Self::BlockNumber {
		Zero::zero()
	}
}

impl module_idle_scheduler::Config for TestRuntime {
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
		Some(AccountId32::from_str("1234500000000000000000000000000000000000").unwrap())
	}
}

parameter_types! {
	pub NetworkContractSource: H160 = H160::from_low_u64_be(1);
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: Balance = convert_decimals_to_evm(10);
}

impl module_evm_accounts::Config for TestRuntime {
	type Event = Event;
	type Currency = Balances;
	type AddressMapping = EvmAddressMapping<TestRuntime>;
	type TransferAll = Currencies;
	type ChainId = EvmChainId<TestRuntime>;
	type WeightInfo = ();
}

impl module_evm::Config for TestRuntime {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = ConstU32<100>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<20_000_000>;

	type Event = Event;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = ();

	type NetworkContractOrigin = frame_system::EnsureSignedBy<NetworkContractAccount, AccountId32>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = frame_system::EnsureSignedBy<CouncilAccount, AccountId32>;

	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = AuthorGiven;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

frame_support::construct_runtime!(
	pub enum TestRuntime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		EVM: module_evm,
		EvmAccounts: module_evm_accounts,
		Tokens: orml_tokens exclude_parts { Call },
		Balances: pallet_balances,
		Currencies: orml_currencies,
		IdleScheduler: module_idle_scheduler,
	}
);

pub fn new_test_ext() -> sp_io::TestExternalities {
	sp_io::TestExternalities::new_empty()
}
