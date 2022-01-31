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

use crate::{AllPrecompiles, Ratio, RuntimeBlockWeights, Weight};
use acala_service::chain_spec::mandala::evm_genesis;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	assert_ok, ord_parameter_types, parameter_types,
	traits::{
		EqualPrivilegeOnly, Everything, GenesisBuild, InstanceFilter, Nothing, OnFinalize, OnInitialize, SortedMembers,
	},
	weights::IdentityFee,
	PalletId, RuntimeDebug,
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_evm::EvmTask;
use module_support::DispatchableTask;
use module_support::{
	mocks::MockAddressMapping, AddressMapping as AddressMappingT, DEXIncentives, ExchangeRate, ExchangeRateProvider,
	Rate,
};
use orml_traits::{parameter_type_with_key, MultiReservableCurrency};
pub use primitives::{
	convert_decimals_to_evm, define_combined_task, evm::EvmAddress, task::TaskResult, Amount, BlockNumber, CurrencyId,
	DexShare, Header, Lease, Nonce, ReserveIdentifier, TokenSymbol, TradingPair,
};
use scale_info::TypeInfo;
use sp_core::{crypto::AccountId32, H160, H256};
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, BlockNumberProvider, Convert, IdentityLookup, One as OneT, Zero},
	DispatchResult, FixedPointNumber, FixedU128, Perbill,
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
	type BaseCallFilter = Everything;
	type BlockWeights = RuntimeBlockWeights;
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u32;
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
	pub static OracleMembers: Vec<AccountId> = vec![ALICE, BOB, EVA];
	pub const MaxHasDispatchedSize: u32 = 40;
}

pub struct Members;

impl SortedMembers<AccountId> for Members {
	fn sorted_members() -> Vec<AccountId> {
		OracleMembers::get()
	}
}

impl orml_oracle::Config for Test {
	type Event = Event;
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Self, MinimumCount, ExpiresIn>;
	type Time = Timestamp;
	type OracleKey = Key;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
	type Members = Members;
	type MaxHasDispatchedSize = MaxHasDispatchedSize;
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
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
}

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const RENBTC: CurrencyId = CurrencyId::Token(TokenSymbol::RENBTC);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const LP_ACA_AUSD: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Token(TokenSymbol::AUSD));

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
	type EVMBridge = module_evm_bridge::EVMBridge<Test>;
	type SweepOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type OnDust = ();
}

impl module_evm_bridge::Config for Test {
	type EVM = EVMModule;
}

impl module_asset_registry::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type StakingCurrencyId = GetStakingCurrencyId;
	type EVMBridge = module_evm_bridge::EVMBridge<Test>;
	type RegisterOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type WeightInfo = ();
}

define_combined_task! {
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	pub enum ScheduledTasks {
		EvmTask(EvmTask<Test>),
	}
}

parameter_types!(
	pub MinimumWeightRemainInBlock: Weight = u64::MIN;
);

impl module_idle_scheduler::Config for Test {
	type Event = Event;
	type WeightInfo = ();
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
}

parameter_types! {
	pub const CreateClassDeposit: Balance = 200;
	pub const CreateTokenDeposit: Balance = 100;
	pub const DataDepositPerByte: Balance = 10;
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
	pub MaxAttributesBytes: u32 = 2048;
}
impl module_nft::Config for Test {
	type Event = Event;
	type Currency = Balances;
	type CreateClassDeposit = CreateClassDeposit;
	type CreateTokenDeposit = CreateTokenDeposit;
	type DataDepositPerByte = DataDepositPerByte;
	type PalletId = NftPalletId;
	type MaxAttributesBytes = MaxAttributesBytes;
	type WeightInfo = ();
}

parameter_types! {
	pub MaxClassMetadata: u32 = 1024;
	pub MaxTokenMetadata: u32 = 1024;
}

impl orml_nft::Config for Test {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = module_nft::ClassData<Balance>;
	type TokenData = module_nft::TokenData<Balance>;
	type MaxClassMetadata = MaxClassMetadata;
	type MaxTokenMetadata = MaxTokenMetadata;
}

parameter_types! {
	pub const TransactionByteFee: Balance = 10;
	pub const GetStableCurrencyId: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
	pub DefaultFeeSwapPathList: Vec<Vec<CurrencyId>> = vec![vec![CurrencyId::Token(TokenSymbol::AUSD), CurrencyId::Token(TokenSymbol::ACA)]];
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::one();
	pub OperationalFeeMultiplier: u64 = 5;
	pub TipPerWeightStep: Balance = 1;
	pub MaxTipsOfPriority: Balance = 1000;
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	pub KaruraTreasuryAccount: AccountId = TreasuryPalletId::get().into_account();
}

impl module_transaction_payment::Config for Test {
	type Event = Event;
	type NativeCurrencyId = GetNativeCurrencyId;
	type DefaultFeeSwapPathList = DefaultFeeSwapPathList;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = ();
	type AlternativeFeeSwapDeposit = ExistentialDeposit;
	type TransactionByteFee = TransactionByteFee;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type TipPerWeightStep = TipPerWeightStep;
	type MaxTipsOfPriority = MaxTipsOfPriority;
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
	type DEX = ();
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = module_prices::RealTimePriceProvider<Test>;
	type WeightInfo = ();
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = KaruraTreasuryAccount;
	type UpdateOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo)]
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
			ProxyType::JustTransfer => matches!(c, Call::Balances(pallet_balances::Call::transfer { .. })),
			ProxyType::JustUtility => matches!(c, Call::Utility { .. }),
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
	type PalletsOrigin = OriginCaller;
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
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
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
	pub const TradingPathLimit: u32 = 4;
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
}

impl module_dex::Config for Test {
	type Event = Event;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = EvmErc20InfoMapping;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type OnLiquidityPoolUpdated = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub type EvmErc20InfoMapping = module_asset_registry::EvmErc20InfoMapping<Test>;

parameter_types! {
	pub NetworkContractSource: H160 = alice_evm_addr();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const NewContractExtraBytes: u32 = 100;
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
	pub const TxFeePerGas: u64 = 10;
	pub const DeveloperDeposit: u64 = 1000;
	pub const PublicationFee: u64 = 200;
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
	type TransferAll = Currencies;
	type NewContractExtraBytes = NewContractExtraBytes;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = TxFeePerGas;
	type Event = Event;
	type Precompiles = AllPrecompiles<Self>;
	type ChainId = ChainId;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = ChargeTransactionPayment;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = DeveloperDeposit;
	type PublicationFee = PublicationFee;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = ();
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

pub struct MockLiquidStakingExchangeProvider;
impl ExchangeRateProvider for MockLiquidStakingExchangeProvider {
	fn get_exchange_rate() -> ExchangeRate {
		ExchangeRate::saturating_from_rational(1, 2)
	}
}

impl BlockNumberProvider for MockRelayBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		Self::get()
	}
}

parameter_type_with_key! {
	pub LiquidCrowdloanLeaseBlockNumber: |_lease: Lease| -> Option<BlockNumber> {
		None
	};
}

parameter_types! {
	pub StableCurrencyFixedPrice: Price = Price::saturating_from_rational(1, 1);
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub static MockRelayBlockNumberProvider: BlockNumber = 0;
	pub RewardRatePerRelaychainBlock: Rate = Rate::zero();
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
	type Erc20InfoMapping = EvmErc20InfoMapping;
	type LiquidCrowdloanLeaseBlockNumber = LiquidCrowdloanLeaseBlockNumber;
	type RelayChainBlockNumber = MockRelayBlockNumberProvider;
	type RewardRatePerRelaychainBlock = RewardRatePerRelaychainBlock;
	type WeightInfo = ();
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const EVA: AccountId = AccountId::new([5u8; 32]);

pub fn alice() -> AccountId {
	<Test as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn bob() -> AccountId {
	<Test as module_evm::Config>::AddressMapping::get_account_id(&bob_evm_addr())
}

pub fn bob_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000002").unwrap()
}

pub fn aca_evm_address() -> EvmAddress {
	EvmAddress::try_from(ACA).unwrap()
}

pub fn ausd_evm_address() -> EvmAddress {
	EvmAddress::try_from(AUSD).unwrap()
}

pub fn renbtc_evm_address() -> EvmAddress {
	EvmAddress::try_from(RENBTC).unwrap()
}

pub fn lp_aca_ausd_evm_address() -> EvmAddress {
	EvmAddress::try_from(LP_ACA_AUSD).unwrap()
}

pub fn erc20_address_not_exists() -> EvmAddress {
	EvmAddress::from_str("0000000000000000000000000000000200000001").unwrap()
}

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		Oracle: orml_oracle::{Pallet, Storage, Call, Event<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		EVMBridge: module_evm_bridge::{Pallet},
		AssetRegistry: module_asset_registry::{Pallet, Call, Storage, Event<T>},
		NFTModule: module_nft::{Pallet, Call, Event<T>},
		TransactionPayment: module_transaction_payment::{Pallet, Call, Storage, Event<T>},
		Prices: module_prices::{Pallet, Storage, Call, Event<T>},
		Proxy: pallet_proxy::{Pallet, Call, Storage, Event<T>},
		Utility: pallet_utility::{Pallet, Call, Event},
		Scheduler: pallet_scheduler::{Pallet, Call, Storage, Event<T>},
		DexModule: module_dex::{Pallet, Storage, Call, Event<T>, Config<T>},
		EVMModule: module_evm::{Pallet, Config<T>, Call, Storage, Event<T>},
		IdleScheduler: module_idle_scheduler::{Pallet, Call, Storage, Event<T>},
	}
);

// This function basically just builds a genesis storage key/value store
// according to our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut storage = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

	let mut accounts = BTreeMap::new();
	let mut evm_genesis_accounts = evm_genesis(vec![]);
	accounts.append(&mut evm_genesis_accounts);

	accounts.insert(
		alice_evm_addr(),
		module_evm::GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			..Default::default()
		},
	);
	accounts.insert(
		bob_evm_addr(),
		module_evm::GenesisAccount {
			nonce: 1,
			balance: INITIAL_BALANCE,
			..Default::default()
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
			RENBTC,
			1_000_000_000_000
		));
		assert_ok!(Currencies::update_balance(Origin::root(), ALICE, AUSD, 1_000_000_000));

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			MockAddressMapping::get_account_id(&alice_evm_addr()),
			RENBTC,
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
	num[..].copy_from_slice(&output[64 - 4..64]);
	let task_id_len: u32 = u32::from_be_bytes(num);
	output[64..64 + task_id_len as usize].to_vec()
}
