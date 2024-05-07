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

use frame_support::{
	derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, FindAuthor, Nothing},
	weights::Weight,
	ConsensusEngineId,
};
use module_evm::{EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_support::{
	mocks::{MockAddressMapping, TestRandomness},
	DispatchableTask,
};
use orml_traits::parameter_type_with_key;
use parity_scale_codec::{Decode, Encode};
use primitives::{
	define_combined_task, evm::convert_decimals_to_evm, task::TaskResult, Amount, BlockNumber, CurrencyId, Nonce,
	ReserveIdentifier, TokenSymbol,
};
use scale_info::TypeInfo;
use sp_core::H160;
pub use sp_runtime::AccountId32;
use sp_runtime::{
	traits::{BlockNumberProvider, Convert, IdentityLookup, Zero},
	RuntimeDebug,
};
use std::str::FromStr;

type Block = frame_system::mocking::MockBlock<TestRuntime>;

type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for TestRuntime {
	type AccountId = AccountId32;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnKilledAccount = (
		module_evm::CallKillAccount<TestRuntime>,
		module_evm_accounts::CallKillAccount<TestRuntime>,
	);
}

impl pallet_balances::Config for TestRuntime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = module_support::SystemAccountStore<TestRuntime>;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
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

parameter_types! {
	pub MinimumWeightRemainInBlock: Weight = Weight::from_parts(0, 0);
}

impl module_idle_scheduler::Config for TestRuntime {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
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
	type RuntimeEvent = RuntimeEvent;
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

	type RuntimeEvent = RuntimeEvent;
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
	type Randomness = TestRandomness<Self>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

frame_support::construct_runtime!(
	pub enum TestRuntime {
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
