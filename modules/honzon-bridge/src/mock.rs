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

//! Mocks for Honzon Bridge module.

#![cfg(test)]

pub use crate as module_honzon_bridge;

pub use frame_support::{
	assert_ok, construct_runtime, ord_parameter_types,
	pallet_prelude::GenesisBuild,
	parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
pub use frame_system::{EnsureRoot, EnsureSignedBy, RawOrigin};
pub use module_evm_accounts::EvmAddressMapping;
pub use module_support::{mocks::MockAddressMapping, AddressMapping};
pub use orml_traits::{parameter_type_with_key, MultiCurrency};
use sp_core::{H160, H256, U256};
use sp_runtime::traits::AccountIdConversion;
use std::str::FromStr;

pub use primitives::{
	convert_decimals_to_evm, evm::EvmAddress, AccountId, Amount, Balance, BlockNumber, CurrencyId, ReserveIdentifier,
	TokenSymbol,
};

/// For testing only. Does not check for overflow.
pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}
pub const INITIAL_BALANCE: Balance = 1_000_000;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const KUSD: CurrencyId = CurrencyId::Token(TokenSymbol::KUSD);

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
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<0>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
}

impl pallet_timestamp::Config for Runtime {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1000>;
	type WeightInfo = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, Balances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = EvmAddressMapping<Runtime>;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type GasToWeight = ();
	type SweepOrigin = EnsureRoot<AccountId>;
	type OnDust = ();
}

parameter_types! {
	pub NetworkContractSource: EvmAddress = EvmAddress::default();
}

ord_parameter_types! {
	pub const TreasuryAccount: AccountId = AccountId::from([2u8; 32]);
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
}

impl module_evm::Config for Runtime {
	type AddressMapping = EvmAddressMapping<Runtime>;
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
	type NetworkContractOrigin = EnsureRoot<AccountId>;
	type NetworkContractSource = NetworkContractSource;

	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureRoot<AccountId>;

	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = ();
	type Task = ();
	type IdleScheduler = ();
	type WeightInfo = ();
}

impl module_evm_bridge::Config for Runtime {
	type EVM = EVM;
}

impl module_evm_accounts::Config for Runtime {
	type Event = Event;
	type Currency = Balances;
	type ChainId = ();
	type AddressMapping = EvmAddressMapping<Runtime>;
	type TransferAll = Currencies;
	type WeightInfo = ();
}

parameter_types! {
	pub const StableCoinCurrencyId: CurrencyId = KUSD;
	pub const HonzonBridgePalletId: PalletId = PalletId(*b"aca/hzbg");
	pub HonzonBridgeAccount: AccountId = HonzonBridgePalletId::get().into_account_truncating();
}

impl module_honzon_bridge::Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type StableCoinCurrencyId = StableCoinCurrencyId;
	type HonzonBridgeAccount = HonzonBridgeAccount;
	type UpdateOrigin = EnsureRoot<AccountId>;
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
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		Currencies: module_currencies,
		EVM: module_evm,
		EvmAccountsModule: module_evm_accounts,
		EVMBridge: module_evm_bridge,
		HonzonBridge: module_honzon_bridge,
	}
);

pub fn alice() -> AccountId {
	MockAddressMapping::get_account_id(&alice_evm_addr())
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
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

	assert_ok!(EVM::publish_free(Origin::root(), erc20_address()));
}

pub struct ExtBuilder {
	tokens_balances: Vec<(AccountId, CurrencyId, Balance)>,
	native_balances: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		let initial = dollar(INITIAL_BALANCE);
		Self {
			tokens_balances: vec![(alice(), KUSD, initial), (HonzonBridgeAccount::get(), KUSD, initial)],
			native_balances: vec![(alice(), initial), (HonzonBridgeAccount::get(), initial)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.tokens_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
