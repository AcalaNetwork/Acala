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

use crate::{AllPrecompiles, Ratio, RuntimeBlockWeights, SystemContractsFilter, Weight};
use codec::{Decode, Encode};
use frame_support::{
	assert_ok, ord_parameter_types, parameter_types,
	traits::{GenesisBuild, InstanceFilter, OnFinalize, OnInitialize},
	weights::IdentityFee,
	RuntimeDebug,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_support::{DEXIncentives, ExchangeRate, ExchangeRateProvider};
use orml_traits::{parameter_type_with_key, MultiReservableCurrency};
pub use primitives::{
	evm::AddressMapping, mocks::MockAddressMapping, Amount, BlockNumber, CurrencyId, Header, Nonce, TokenSymbol,
	TradingPair,
};
use sp_core::{bytes::from_hex, crypto::AccountId32, Bytes, H160, H256};
use sp_runtime::{
	traits::{BlakeTwo256, Convert, IdentityLookup},
	DispatchResult, FixedPointNumber, FixedU128, ModuleId, Perbill,
};
use sp_std::{collections::btree_map::BTreeMap, str::FromStr};

pub type AccountId = AccountId32;
type Key = CurrencyId;
pub type Price = FixedU128;
type Balance = u128;

parameter_types! {
	pub const BlockHashCount: u32 = 250;
}
impl frame_system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
}

parameter_types! {
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: u32 = 600;
	pub const RootOperatorAccountId: AccountId = ALICE;
}

impl orml_oracle::Config for Test {
	type Event = Event;
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Self, MinimumCount, ExpiresIn>;
	type Time = Timestamp;
	type OracleKey = Key;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
	type WeightInfo = ();
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = ();
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
}

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const XBTC: CurrencyId = CurrencyId::Token(TokenSymbol::XBTC);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl module_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = EVMBridge;
}

impl module_evm_bridge::Config for Test {
	type EVM = ModuleEVM;
}

parameter_types! {
	pub const CreateClassDeposit: Balance = 200;
	pub const CreateTokenDeposit: Balance = 100;
	pub const NftModuleId: ModuleId = ModuleId(*b"aca/aNFT");
}
impl module_nft::Config for Test {
	type Event = Event;
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type ModuleId = NftModuleId;
	type WeightInfo = ();
}

impl orml_nft::Config for Test {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = module_nft::ClassData<Balance>;
	type TokenData = module_nft::TokenData<Balance>;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10;
	pub const GetStableCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
	pub AllNonNativeCurrencyIds: Vec<CurrencyId> = vec![CurrencyId::Token(TokenSymbol::AUSD)];
	pub MaxSlippageSwapWithDEX: Ratio = Ratio::one();
}

impl module_transaction_payment::Config for Test {
	type AllNonNativeCurrencyIds = AllNonNativeCurrencyIds;
	type NativeCurrencyId = GetNativeCurrencyId;
	type StableCurrencyId = GetStableCurrencyId;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = ();
	type TransactionByteFee = TransactionByteFee;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
	type DEX = ();
	type MaxSlippageSwapWithDEX = MaxSlippageSwapWithDEX;
	type WeightInfo = ();
}
pub type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Test>;

parameter_types! {
	pub const ProxyDepositBase: u64 = 1;
	pub const ProxyDepositFactor: u64 = 1;
	pub const MaxProxies: u16 = 4;
	pub const MaxPending: u32 = 2;
	pub const AnnouncementDepositBase: u64 = 1;
	pub const AnnouncementDepositFactor: u64 = 1;
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug)]
pub enum ProxyType {
	Any,
	JustTransfer,
	JustUtility,
}
impl Default for ProxyType {
	fn default() -> Self {
		Self::Any
	}
}
impl InstanceFilter<Call> for ProxyType {
	fn filter(&self, c: &Call) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer(..))),
			ProxyType::JustUtility => matches!(c, Call::Utility(..)),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		self == &ProxyType::Any || self == o
	}
}

impl pallet_proxy::Config for Test {
	type Event = Event;
	type Call = Call;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ProxyDepositBase;
	type ProxyDepositFactor = ProxyDepositFactor;
	type MaxProxies = MaxProxies;
	type WeightInfo = ();
	type CallHasher = BlakeTwo256;
	type MaxPending = MaxPending;
	type AnnouncementDepositBase = AnnouncementDepositBase;
	type AnnouncementDepositFactor = AnnouncementDepositFactor;
}

impl pallet_utility::Config for Test {
	type Event = Event;
	type Call = Call;
	type WeightInfo = ();
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(10) * RuntimeBlockWeights::get().max_block;
	pub const MaxScheduledPerBlock: u32 = 50;
}

impl pallet_scheduler::Config for Test {
	type Event = Event;
	type Origin = Origin;
	type PalletsOrigin = OriginCaller;
	type Call = Call;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type MaxScheduledPerBlock = MaxScheduledPerBlock;
	type WeightInfo = ();
}

pub struct MockDEXIncentives;
impl DEXIncentives<AccountId, CurrencyId, Balance> for MockDEXIncentives {
	fn do_deposit_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		Tokens::reserve(lp_currency_id, who, amount)
	}

	fn do_withdraw_dex_share(who: &AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		let _ = Tokens::unreserve(lp_currency_id, who, amount);
		Ok(())
	}
}

ord_parameter_types! {
	pub const ListingOrigin: AccountId = ALICE;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const TradingPathLimit: u32 = 3;
	pub const DEXModuleId: ModuleId = ModuleId(*b"aca/dexm");
}

impl module_dex::Config for Test {
	type Event = Event;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type ModuleId = DEXModuleId;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub type MultiCurrencyPrecompile = crate::MultiCurrencyPrecompile<AccountId, MockAddressMapping, Currencies>;

pub type NFTPrecompile = crate::NFTPrecompile<AccountId, MockAddressMapping, NFTModule>;
pub type StateRentPrecompile = crate::StateRentPrecompile<AccountId, MockAddressMapping, ModuleEVM>;
pub type OraclePrecompile = crate::OraclePrecompile<AccountId, MockAddressMapping, Prices>;
pub type ScheduleCallPrecompile = crate::ScheduleCallPrecompile<
	AccountId,
	MockAddressMapping,
	Scheduler,
	ChargeTransactionPayment,
	Call,
	Origin,
	OriginCaller,
	Test,
>;
pub type DexPrecompile = crate::DexPrecompile<AccountId, MockAddressMapping, DexModule>;

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
	pub const MaxCodeSize: u32 = 60 * 1024;
	pub const ChainId: u64 = 1;
}

pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> u64 {
		a as Weight
	}
}

impl module_evm::Config for Test {
	type AddressMapping = MockAddressMapping;
	type Currency = Balances;
	type MergeAccount = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type MaxCodeSize = MaxCodeSize;
	type Event = Event;
	type Precompiles = AllPrecompiles<
		SystemContractsFilter,
		MultiCurrencyPrecompile,
		NFTPrecompile,
		StateRentPrecompile,
		OraclePrecompile,
		ScheduleCallPrecompile,
		DexPrecompile,
	>;
	type ChainId = ChainId;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = ChargeTransactionPayment;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type DeploymentFee = DeploymentFee;
	type TreasuryAccount = TreasuryAccount;
	type FreeDeploymentOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type WeightInfo = ();
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::saturating_from_rational(1, 2)
	}
}

parameter_types! {
	pub StableCurrencyFixedPrice: Price = Price::saturating_from_rational(1, 1);
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
}

ord_parameter_types! {
	pub const One: AccountId = AccountId::new([1u8; 32]);
}

impl module_prices::Config for Test {
	type Event = Event;
	type Source = Oracle;
	type GetStableCurrencyId = GetStableCurrencyId;
	type StableCurrencyFixedPrice = StableCurrencyFixedPrice;
	type GetStakingCurrencyId = GetStakingCurrencyId;
	type GetLiquidCurrencyId = GetLiquidCurrencyId;
	type LockOrigin = EnsureSignedBy<One, AccountId>;
	type LiquidStakingExchangeRateProvider = MockLiquidStakingExchangeProvider;
	type DEX = DexModule;
	type Currency = Currencies;
	type WeightInfo = ();
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const EVA: AccountId = AccountId::new([5u8; 32]);

pub fn alice() -> H160 {
	H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
}

pub fn bob() -> H160 {
	H160([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2])
}

pub fn evm_genesis() -> BTreeMap<H160, module_evm::GenesisAccount<Balance, u64>> {
	let contracts_json = &include_bytes!("../../../../predeploy-contracts/resources/bytecodes.json")[..];
	let contracts: Vec<(String, String, String)> = serde_json::from_slice(contracts_json).unwrap();
	let mut accounts = BTreeMap::new();
	for (_, address, code_string) in contracts {
		let account = module_evm::GenesisAccount {
			nonce: 0,
			balance: 0u128,
			storage: Default::default(),
			code: Bytes::from_str(&code_string).unwrap().0,
		};

		let addr = H160::from_slice(
			from_hex(address.as_str())
				.expect("predeploy-contracts must specify address")
				.as_slice(),
		);
		accounts.insert(addr, account);
	}
	accounts
}

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000;
pub const ACA_ERC20_ADDRESS: &str = "0x0000000000000000000000000000000001000000";

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Oracle: orml_oracle::{Pallet, Storage, Call, Config<T>, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		EVMBridge: module_evm_bridge::{Pallet},
		NFTModule: module_nft::{Pallet, Call, Event<T>},
		TransactionPayment: module_transaction_payment::{Pallet, Call, Storage},
		Prices: module_prices::{Pallet, Storage, Call, Event<T>},
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
		Utility: pallet_utility::{Pallet, Call, Event},
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
		DexModule: module_dex::{Pallet, Storage, Call, Event<T>, Config<T>},
		ModuleEVM: module_evm::{Pallet, Config<T>, Call, Storage, Event<T>},
	}
);

// This function basically just builds a genesis storage key/value store
// according to our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	let _ = orml_oracle::GenesisConfig::<Test> {
		members: vec![ALICE, BOB, EVA].into(),
		phantom: Default::default(),
	}
	.assimilate_storage(&mut storage);

	let mut accounts = BTreeMap::new();
	let mut evm_genesis_accounts = evm_genesis();
	accounts.append(&mut evm_genesis_accounts);

	accounts.insert(
		alice(),
		module_evm::GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: Default::default(),
		},
	);
	accounts.insert(
		bob(),
		module_evm::GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			storage: Default::default(),
			code: Default::default(),
		},
	);

	pallet_balances::GenesisConfig::<Test>::default()
		.assimilate_storage(&mut storage)
		.unwrap();
	module_evm::GenesisConfig::<Test> { accounts }
		.assimilate_storage(&mut storage)
		.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		System::set_block_number(1);
		Timestamp::set_timestamp(1);

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			XBTC,
			1_000_000_000_000
		));
		assert_ok!(Currencies::update_balance(Origin::root(), ALICE, AUSD, 1_000_000_000));

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			MockAddressMapping::get_account_id(&alice()),
			XBTC,
			1_000
		));
	});
	ext
}

pub fn run_to_block(n: u32) {
	while System::block_number() < n {
		Scheduler::on_finalize(System::block_number());
		System::set_block_number(System::block_number() + 1);
		Scheduler::on_initialize(System::block_number());
	}
}
pub fn get_task_id(output: Vec<u8>) -> Vec<u8> {
	let mut num = [0u8; 4];
	num[..].copy_from_slice(&output[32 - 4..32]);
	let task_id_len: u32 = u32::from_be_bytes(num);
	return output[32..32 + task_id_len as usize].to_vec();
}
