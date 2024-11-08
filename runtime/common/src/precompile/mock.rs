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

#![cfg(any(test, feature = "wasm-bench"))]

use crate::{AllPrecompiles, Ratio, RuntimeBlockWeights, Weight};
use frame_support::{
	derive_impl, ord_parameter_types, parameter_types,
	traits::{
		ConstU128, ConstU32, ConstU64, EqualPrivilegeOnly, Everything, InstanceFilter, LockIdentifier, Nothing,
		OnFinalize, OnInitialize, SortedMembers,
	},
	weights::{ConstantMultiplier, IdentityFee},
	PalletId,
};
use frame_system::{offchain::SendTransactionTypes, EnsureRoot, EnsureSignedBy};
use module_cdp_engine::CollateralCurrencyIds;
use module_evm::{EvmChainId, EvmTask};
use module_evm_accounts::EvmAddressMapping;
use module_support::{
	mocks::{MockStableAsset, TestRandomness},
	AddressMapping as AddressMappingT, AuctionManager, DEXIncentives, DispatchableTask, EmergencyShutdown,
	ExchangeRate, ExchangeRateProvider, FractionalRate, HomaSubAccountXcm, PoolId, PriceProvider, Rate,
	SpecificJointsSwap,
};
use orml_traits::{location::AbsoluteReserveProvider, parameter_type_with_key, MultiCurrency, MultiReservableCurrency};
use parity_scale_codec::{Decode, Encode, MaxEncodedLen};
pub use primitives::{
	define_combined_task,
	evm::{convert_decimals_to_evm, EvmAddress},
	task::TaskResult,
	Address, Amount, AuctionId, BlockNumber, CurrencyId, DexShare, EraIndex, Header, Lease, Moment, Nonce,
	ReserveIdentifier, Signature, TokenSymbol, TradingPair,
};
use scale_info::TypeInfo;
use sp_core::H160;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, BlockNumberProvider, Convert, IdentityLookup, One as OneT, Zero},
	AccountId32, DispatchResult, FixedPointNumber, FixedU128, Perbill, Percent, RuntimeDebug,
};
use sp_std::prelude::*;
use xcm::{prelude::*, v4::Xcm};
use xcm_builder::FixedWeightBounds;

pub type AccountId = AccountId32;
type Key = CurrencyId;
pub type Price = FixedU128;
type Balance = u128;

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Test {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
	type BlockHashCount = ConstU32<250>;
}

parameter_types! {
	pub const ExistenceRequirement: u128 = 1;
	pub const MinimumCount: u32 = 1;
	pub const ExpiresIn: u32 = 600;
	pub const RootOperatorAccountId: AccountId = ALICE;
	pub OracleMembers: Vec<AccountId> = vec![ALICE, BOB, EVA];
}

pub struct Members;

impl SortedMembers<AccountId> for Members {
	fn sorted_members() -> Vec<AccountId> {
		OracleMembers::get()
	}
}

parameter_types! {
	pub const MaxFeedValues: u32 = 10; // max 10 values allowd to feed in one call.
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelper;
#[cfg(feature = "runtime-benchmarks")]
impl orml_oracle::BenchmarkHelper<Key, Price, MaxFeedValues> for BenchmarkHelper {
	fn get_currency_id_value_pairs() -> sp_runtime::BoundedVec<(CurrencyId, Price), MaxFeedValues> {
		sp_runtime::BoundedVec::default()
	}
}

impl orml_oracle::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnNewData = ();
	type CombineData = orml_oracle::DefaultCombineData<Self, MinimumCount, ExpiresIn>;
	type Time = Timestamp;
	type OracleKey = Key;
	type OracleValue = Price;
	type RootOperatorAccountId = RootOperatorAccountId;
	type Members = Members;
	type WeightInfo = ();
	type MaxHasDispatchedSize = ConstU32<40>;
	type MaxFeedValues = MaxFeedValues;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelper;
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
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistenceRequirement;
	type AccountStore = module_support::SystemAccountStore<Test>;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ReserveIdentifier;
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}

pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const LCDOT: CurrencyId = CurrencyId::LiquidCrowdloan(13);
pub const LP_ACA_AUSD: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Token(TokenSymbol::AUSD));

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
	pub Erc20HoldingAccount: H160 = H160::from_low_u64_be(1);
}

impl module_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20HoldingAccount = Erc20HoldingAccount;
	type WeightInfo = ();
	type AddressMapping = EvmAddressMapping<Test>;
	type EVMBridge = module_evm_bridge::EVMBridge<Test>;
	type GasToWeight = ();
	type SweepOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type OnDust = ();
}

impl module_evm_bridge::Config for Test {
	type EVM = EVMModule;
}

impl module_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
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

impl module_idle_scheduler::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type Index = Nonce;
	type Task = ScheduledTasks;
	type MinimumWeightRemainInBlock = MinimumWeightRemainInBlock;
	type RelayChainBlockNumberProvider = MockBlockNumberProvider;
	type DisableBlockThreshold = ConstU32<6>;
}

parameter_types! {
	pub const NftPalletId: PalletId = PalletId(*b"aca/aNFT");
}
impl module_nft::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type CreateClassDeposit = ConstU128<200>;
	type CreateTokenDeposit = ConstU128<100>;
	type DataDepositPerByte = ConstU128<10>;
	type PalletId = NftPalletId;
	type MaxAttributesBytes = ConstU32<2048>;
	type WeightInfo = ();
}

impl orml_nft::Config for Test {
	type ClassId = u32;
	type TokenId = u64;
	type ClassData = module_nft::ClassData<Balance>;
	type TokenData = module_nft::TokenData<Balance>;
	type MaxClassMetadata = ConstU32<1024>;
	type MaxTokenMetadata = ConstU32<1024>;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::one();
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub const TransactionPaymentPalletId: PalletId = PalletId(*b"aca/fees");
	pub KaruraTreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
	pub const CustomFeeSurplus: Percent = Percent::from_percent(50);
	pub const AlternativeFeeSurplus: Percent = Percent::from_percent(25);
	pub DefaultFeeTokens: Vec<CurrencyId> = vec![AUSD];
}

impl module_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type NativeCurrencyId = GetNativeCurrencyId;
	type Currency = Balances;
	type MultiCurrency = Currencies;
	type OnTransactionPayment = ();
	type OperationalFeeMultiplier = ConstU64<5>;
	type TipPerWeightStep = ConstU128<1>;
	type MaxTipsOfPriority = ConstU128<1000>;
	type AlternativeFeeSwapDeposit = ExistenceRequirement;
	type WeightToFee = IdentityFee<Balance>;
	type LengthToFee = ConstantMultiplier<Balance, ConstU128<10>>;
	type FeeMultiplierUpdate = ();
	type Swap = SpecificJointsSwap<DexModule, AlternativeSwapPathJointList>;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type TradingPathLimit = TradingPathLimit;
	type PriceSource = module_prices::RealTimePriceProvider<Test>;
	type WeightInfo = ();
	type PalletId = TransactionPaymentPalletId;
	type TreasuryAccount = KaruraTreasuryAccount;
	type UpdateOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type CustomFeeSurplus = CustomFeeSurplus;
	type AlternativeFeeSurplus = AlternativeFeeSurplus;
	type DefaultFeeTokens = DefaultFeeTokens;
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
impl InstanceFilter<RuntimeCall> for ProxyType {
	fn filter(&self, c: &RuntimeCall) -> bool {
		match self {
			ProxyType::Any => true,
			ProxyType::JustTransfer => matches!(
				c,
				RuntimeCall::Balances(pallet_balances::Call::transfer_allow_death { .. })
			),
			ProxyType::JustUtility => matches!(c, RuntimeCall::Utility { .. }),
		}
	}
	fn is_superset(&self, o: &Self) -> bool {
		self == &ProxyType::Any || self == o
	}
}

impl pallet_proxy::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type Currency = Balances;
	type ProxyType = ProxyType;
	type ProxyDepositBase = ConstU128<1>;
	type ProxyDepositFactor = ConstU128<1>;
	type MaxProxies = ConstU32<4>;
	type WeightInfo = ();
	type MaxPending = ConstU32<2>;
	type CallHasher = BlakeTwo256;
	type AnnouncementDepositBase = ConstU128<1>;
	type AnnouncementDepositFactor = ConstU128<1>;
}

impl pallet_utility::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type WeightInfo = ();
}

parameter_types! {
	pub MaximumSchedulerWeight: Weight = Perbill::from_percent(10) * RuntimeBlockWeights::get().max_block;
}

impl pallet_scheduler::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeOrigin = RuntimeOrigin;
	type PalletsOrigin = OriginCaller;
	type RuntimeCall = RuntimeCall;
	type MaximumWeight = MaximumSchedulerWeight;
	type ScheduleOrigin = EnsureRoot<AccountId>;
	type OriginPrivilegeCmp = EqualPrivilegeOnly;
	type MaxScheduledPerBlock = ConstU32<50>;
	type WeightInfo = ();
	type Preimages = ();
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
	type RuntimeEvent = RuntimeEvent;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = EvmErc20InfoMapping;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type ExtendedProvisioningBlocks = ConstU32<0>;
	type OnLiquidityPoolUpdated = ();
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"aca/loan");
}

impl module_loans::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Tokens;
	type RiskManager = CDPEngine;
	type CDPTreasury = CDPTreasury;
	type PalletId = LoansPalletId;
	type OnUpdateLoan = ();
}

pub struct MockPriceSource;
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(_base: CurrencyId, _quote: CurrencyId) -> Option<Price> {
		Some(Price::one())
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		Some(Price::one())
	}
}

parameter_type_with_key! {
	pub MinimumCollateralAmount: |_currency_id: CurrencyId| -> Balance {
		10
	};
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(3, 2);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::one();
	pub DefaultLiquidationPenalty: FractionalRate = FractionalRate::try_from(Rate::saturating_from_rational(10, 100)).unwrap();
	pub MaxLiquidationContractSlippage: Ratio = Ratio::saturating_from_rational(15, 100);
	pub CDPEnginePalletId: PalletId = PalletId(*b"aca/cdpe");
	pub SettleErc20EvmOrigin: AccountId = AccountId::from(hex_literal::hex!("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"));
}

impl module_cdp_engine::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type PriceSource = MockPriceSource;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = ConstU128<2>;
	type MinimumCollateralAmount = MinimumCollateralAmount;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CDPTreasury;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type UnsignedPriority = ConstU64<1048576>; // 1 << 20
	type EmergencyShutdown = MockEmergencyShutdown;
	type UnixTime = Timestamp;
	type Currency = Currencies;
	type DEX = DexModule;
	type LiquidationContractsUpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxLiquidationContractSlippage = MaxLiquidationContractSlippage;
	type MaxLiquidationContracts = ConstU32<10>;
	type LiquidationEvmBridge = module_evm_bridge::LiquidationEvmBridge<Test>;
	type PalletId = CDPEnginePalletId;
	type EvmAddressMapping = module_evm_accounts::EvmAddressMapping<Test>;
	type Swap = SpecificJointsSwap<DexModule, AlternativeSwapPathJointList>;
	type EVMBridge = module_evm_bridge::EVMBridge<Test>;
	type SettleErc20EvmOrigin = SettleErc20EvmOrigin;
	type WeightInfo = ();
}

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		_refund_recipient: &AccountId,
		_currency_id: Self::CurrencyId,
		_amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		Ok(())
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Default::default()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		Default::default()
	}
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		false
	}
}

parameter_types! {
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub CDPTreasuryAccount: AccountId = PalletId(*b"aca/hztr").into_account_truncating();
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![AUSD],
	];
}

impl module_cdp_treasury::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = DexModule;
	type MaxAuctionsCount = ConstU32<10_000>;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = CDPTreasuryAccount;
	type WeightInfo = ();
	type StableAsset = MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber>;
	type Swap = SpecificJointsSwap<DexModule, AlternativeSwapPathJointList>;
}

impl module_honzon::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type DepositPerAuthorization = ConstU128<100>;
	type CollateralCurrencyIds = CollateralCurrencyIds<Test>;
	type WeightInfo = ();
}

parameter_types! {
	pub const StableAssetPalletId: PalletId = PalletId(*b"nuts/sta");
}

pub struct EnsurePoolAssetId;
impl nutsfinance_stable_asset::traits::ValidateAssetId<CurrencyId> for EnsurePoolAssetId {
	fn validate(_currency_id: CurrencyId) -> bool {
		true
	}
}

impl nutsfinance_stable_asset::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = CurrencyId;
	type Balance = Balance;
	type Assets = Tokens;
	type PalletId = StableAssetPalletId;

	type AtLeast64BitUnsigned = u128;
	type FeePrecision = ConstU128<10_000_000_000>; // 10 decimals
	type APrecision = ConstU128<100>; // 2 decimals
	type PoolAssetLimit = ConstU32<5>;
	type SwapExactOverAmount = ConstU128<100>;
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type EnsurePoolAssetId = EnsurePoolAssetId;
}

impl module_transaction_pause::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type WeightInfo = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Test, Balances, Amount, BlockNumber>;

pub type EvmErc20InfoMapping = module_asset_registry::EvmErc20InfoMapping<Test>;

parameter_types! {
	pub NetworkContractSource: H160 = alice_evm_addr();
	pub PrecompilesValue: AllPrecompiles<Test, module_transaction_pause::PausedPrecompileFilter<Test>, ()> = AllPrecompiles::<_, _, _>::mandala();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId32 = AccountId32::from([1u8; 32]);
	pub const TreasuryAccount: AccountId32 = AccountId32::from([2u8; 32]);
	pub const NetworkContractAccount: AccountId32 = AccountId32::from([0u8; 32]);
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
}

pub struct GasToWeight;
impl Convert<u64, Weight> for GasToWeight {
	fn convert(a: u64) -> Weight {
		Weight::from_parts(a, 0)
	}
}

impl module_evm::Config for Test {
	type AddressMapping = EvmAddressMapping<Test>;
	type Currency = Balances;
	type TransferAll = Currencies;
	type NewContractExtraBytes = ConstU32<100>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<10>;
	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType = AllPrecompiles<Self, module_transaction_pause::PausedPrecompileFilter<Self>, ()>;
	type PrecompilesValue = PrecompilesValue;
	type GasToWeight = GasToWeight;
	type ChargeTransactionPayment = module_transaction_payment::ChargeTransactionPayment<Test>;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;
	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId>;
	type Runner = module_evm::runner::stack::Runner<Self>;
	type FindAuthor = ();
	type Randomness = TestRandomness<Self>;
	type Task = ScheduledTasks;
	type IdleScheduler = IdleScheduler;
	type WeightInfo = ();
}

impl module_evm_accounts::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type AddressMapping = EvmAddressMapping<Test>;
	type ChainId = EvmChainId<Test>;
	type TransferAll = ();
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

parameter_type_with_key! {
	pub PricingPegged: |_currency_id: CurrencyId| -> Option<CurrencyId> {
		None
	};
}

parameter_types! {
	pub StableCurrencyFixedPrice: Price = Price::saturating_from_rational(1, 1);
	pub const GetStakingCurrencyId: CurrencyId = DOT;
	pub const GetLiquidCurrencyId: CurrencyId = LDOT;
	pub MockRelayBlockNumberProvider: BlockNumber = 0;
	pub RewardRatePerRelaychainBlock: Rate = Rate::zero();
}

ord_parameter_types! {
	pub const One: AccountId = AccountId::new([1u8; 32]);
}

impl module_prices::Config for Test {
	type RuntimeEvent = RuntimeEvent;
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
	type PricingPegged = PricingPegged;
	type WeightInfo = ();
}

/// mock XCM transfer.
pub struct MockHomaSubAccountXcm;
impl HomaSubAccountXcm<AccountId, Balance> for MockHomaSubAccountXcm {
	type RelayChainAccountId = AccountId;
	fn transfer_staking_to_sub_account(sender: &AccountId, _: u16, amount: Balance) -> DispatchResult {
		Currencies::withdraw(StakingCurrencyId::get(), sender, amount)
	}

	fn withdraw_unbonded_from_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn bond_extra_on_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn unbond_on_sub_account(_: u16, _: Balance) -> DispatchResult {
		Ok(())
	}

	fn nominate_on_sub_account(_: u16, _: Vec<Self::RelayChainAccountId>) -> DispatchResult {
		Ok(())
	}

	fn get_xcm_transfer_fee() -> Balance {
		1_000_000
	}

	fn get_parachain_fee(_: Location) -> Balance {
		1_000_000
	}
}

ord_parameter_types! {
	pub const HomaAdmin: AccountId = ALICE;
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = DOT;
	pub const LiquidCurrencyId: CurrencyId = LDOT;
	pub const HomaPalletId: PalletId = PalletId(*b"aca/homa");
	pub const HomaTreasuryAccount: AccountId = HOMA_TREASURY;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub ActiveSubAccountsIndexList: Vec<u16> = vec![0, 1, 2];
	pub const BondingDuration: EraIndex = 28;
	pub const MintThreshold: Balance = 0;
	pub const RedeemThreshold: Balance = 0;
}

impl module_homa::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GovernanceOrigin = EnsureSignedBy<HomaAdmin, AccountId>;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type PalletId = HomaPalletId;
	type TreasuryAccount = HomaTreasuryAccount;
	type DefaultExchangeRate = DefaultExchangeRate;
	type ActiveSubAccountsIndexList = ActiveSubAccountsIndexList;
	type BondingDuration = BondingDuration;
	type MintThreshold = MintThreshold;
	type RedeemThreshold = RedeemThreshold;
	type RelayChainBlockNumber = MockRelayBlockNumberProvider;
	type XcmInterface = MockHomaSubAccountXcm;
	type WeightInfo = ();
	type NominationsProvider = ();
	type ProcessRedeemRequestsLimit = ConstU32<2_000>;
}

parameter_type_with_key! {
	pub MinimalShares: |_pool_id: PoolId| -> Balance {
		0
	};
}

impl orml_rewards::Config for Test {
	type Share = Balance;
	type Balance = Balance;
	type PoolId = PoolId;
	type CurrencyId = CurrencyId;
	type MinimalShares = MinimalShares;
	type Handler = Incentives;
}

parameter_types! {
	pub const IncentivesPalletId: PalletId = PalletId(*b"aca/inct");
}

ord_parameter_types! {
	pub const RewardsSource: AccountId = REWARDS_SOURCE;
}

impl module_incentives::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RewardsSource = RewardsSource;
	type AccumulatePeriod = ConstU32<10>;
	type NativeCurrencyId = GetNativeCurrencyId;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type Currency = Tokens;
	type EmergencyShutdown = MockEmergencyShutdown;
	type PalletId = IncentivesPalletId;
	type WeightInfo = ();
}

parameter_types! {
	pub UniversalLocation: InteriorLocation = Here;
}

pub struct CurrencyIdConvert;
impl Convert<CurrencyId, Option<Location>> for CurrencyIdConvert {
	fn convert(id: CurrencyId) -> Option<Location> {
		use primitives::TokenSymbol::*;
		use CurrencyId::Token;
		match id {
			Token(DOT) => Some(Location::parent()),
			_ => None,
		}
	}
}
impl Convert<Location, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: Location) -> Option<CurrencyId> {
		use primitives::TokenSymbol::*;
		use CurrencyId::Token;

		if location == Location::parent() {
			return Some(Token(DOT));
		}
		None
	}
}
impl Convert<Asset, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(asset: Asset) -> Option<CurrencyId> {
		let AssetId(location) = asset.id;
		Self::convert(location)
	}
}

parameter_types! {
	pub SelfLocation: Location = Location::new(1, Parachain(2000));
}

pub struct AccountIdToLocation;
impl Convert<AccountId, Location> for AccountIdToLocation {
	fn convert(account: AccountId) -> Location {
		Junction::AccountId32 {
			network: None,
			id: account.into(),
		}
		.into()
	}
}

parameter_type_with_key! {
	pub ParachainMinFee: |location: Location| -> Option<u128> {
		#[allow(clippy::match_ref_pats)] // false positive
		match (location.parents, location.first_interior()) {
			(1, Some(Parachain(3))) => Some(100),
			_ => None,
		}
	};
}

pub enum Weightless {}
impl PreparedMessage for Weightless {
	fn weight_of(&self) -> Weight {
		unreachable!()
	}
}

pub struct MockExec;
impl ExecuteXcm<RuntimeCall> for MockExec {
	type Prepared = Weightless;

	fn prepare(_message: Xcm<RuntimeCall>) -> Result<Self::Prepared, Xcm<RuntimeCall>> {
		unreachable!()
	}

	fn execute(_origin: impl Into<Location>, _pre: Weightless, _hash: &mut XcmHash, _weight_credit: Weight) -> Outcome {
		unreachable!()
	}

	fn prepare_and_execute(
		_origin: impl Into<Location>,
		message: Xcm<RuntimeCall>,
		_id: &mut XcmHash,
		weight_limit: Weight,
		_weight_credit: Weight,
	) -> Outcome {
		let o = match (message.0.len(), &message.0.first()) {
			(
				1,
				Some(Transact {
					require_weight_at_most, ..
				}),
			) => {
				if require_weight_at_most.all_lte(weight_limit) {
					Outcome::Complete {
						used: *require_weight_at_most,
					}
				} else {
					Outcome::Error {
						error: XcmError::WeightLimitReached(*require_weight_at_most),
					}
				}
			}
			// use 1000 to decide that it's not supported.
			_ => Outcome::Incomplete {
				used: Weight::from_parts(1000, 1000).min(weight_limit),
				error: XcmError::Unimplemented,
			},
		};
		o
	}

	fn charge_fees(_location: impl Into<Location>, _fees: Assets) -> XcmResult {
		Err(XcmError::Unimplemented)
	}
}

parameter_types! {
	pub const UnitWeightCost: Weight = Weight::from_parts(10, 10);
	pub const BaseXcmWeight: Weight = Weight::from_parts(100_000_000, 100_000_000);
	pub const MaxInstructions: u32 = 100;
	pub const MaxAssetsIntoHolding: u32 = 64;
	pub const MaxAssetsForTransfer: usize = 2;
}

impl orml_xtokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type CurrencyIdConvert = CurrencyIdConvert;
	type AccountIdToLocation = AccountIdToLocation;
	type SelfLocation = SelfLocation;
	type XcmExecutor = MockExec;
	type Weigher = FixedWeightBounds<UnitWeightCost, RuntimeCall, MaxInstructions>;
	type BaseXcmWeight = BaseXcmWeight;
	type UniversalLocation = UniversalLocation;
	type MaxAssetsForTransfer = MaxAssetsForTransfer;
	type MinXcmFee = ParachainMinFee;
	type LocationsFilter = Everything;
	type ReserveProvider = AbsoluteReserveProvider;
	type RateLimiter = ();
	type RateLimiterId = ();
}

parameter_types!(
	pub const LiquidCrowdloanCurrencyId: CurrencyId = LCDOT;
	pub LiquidCrowdloanPalletId: PalletId = PalletId(*b"aca/lqcl");
);

impl module_liquid_crowdloan::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LiquidCrowdloanCurrencyId = LiquidCrowdloanCurrencyId;
	type RelayChainCurrencyId = GetStakingCurrencyId;
	type PalletId = LiquidCrowdloanPalletId;
	type GovernanceOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

pub struct ParameterStoreImpl;
impl orml_traits::parameters::ParameterStore<module_earning::Parameters> for ParameterStoreImpl {
	fn get<K>(key: K) -> Option<K::Value>
	where
		K: orml_traits::parameters::Key
			+ Into<<module_earning::Parameters as orml_traits::parameters::AggregratedKeyValue>::AggregratedKey>,
		<module_earning::Parameters as orml_traits::parameters::AggregratedKeyValue>::AggregratedValue:
			TryInto<K::WrappedValue>,
	{
		let key = key.into();
		match key {
			module_earning::ParametersKey::InstantUnstakeFee(_) => Some(
				module_earning::ParametersValue::InstantUnstakeFee(sp_runtime::Permill::from_percent(10))
					.try_into()
					.ok()?
					.into(),
			),
		}
	}
}

parameter_types! {
	pub const MinBond: Balance = 1_000_000_000;
	pub const UnbondingPeriod: BlockNumber = 10_000;
	pub const EarningLockIdentifier: LockIdentifier = *b"aca/earn";
}

impl module_earning::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type ParameterStore = ParameterStoreImpl;
	type OnBonded = module_incentives::OnEarningBonded<Test>;
	type OnUnbonded = module_incentives::OnEarningUnbonded<Test>;
	type OnUnstakeFee = ();
	type MinBond = MinBond;
	type UnbondingPeriod = UnbondingPeriod;
	type MaxUnbondingChunks = ConstU32<10>;
	type LockIdentifier = EarningLockIdentifier;
	type WeightInfo = ();
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const EVA: AccountId = AccountId::new([5u8; 32]);
pub const REWARDS_SOURCE: AccountId = AccountId::new([3u8; 32]);
pub const HOMA_TREASURY: AccountId = AccountId::new([255u8; 32]);

pub fn alice() -> AccountId {
	<Test as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}

pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from(hex_literal::hex!("1000000000000000000000000000000000000001"))
}

pub fn bob() -> AccountId {
	<Test as module_evm::Config>::AddressMapping::get_account_id(&bob_evm_addr())
}

pub fn bob_evm_addr() -> EvmAddress {
	EvmAddress::from(hex_literal::hex!("1000000000000000000000000000000000000002"))
}

pub fn aca_evm_address() -> EvmAddress {
	EvmAddress::try_from(ACA).unwrap()
}

pub fn ausd_evm_address() -> EvmAddress {
	EvmAddress::try_from(AUSD).unwrap()
}

pub fn lp_aca_ausd_evm_address() -> EvmAddress {
	EvmAddress::try_from(LP_ACA_AUSD).unwrap()
}

pub fn erc20_address_not_exists() -> EvmAddress {
	EvmAddress::from(hex_literal::hex!("0000000000000000000000000000000200000001"))
}

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000;

pub type SignedExtra = (frame_system::CheckWeight<Test>,);
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, SignedExtra>;
pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Oracle: orml_oracle,
		Timestamp: pallet_timestamp,
		Tokens: orml_tokens exclude_parts { Call },
		Balances: pallet_balances,
		Currencies: module_currencies,
		CDPEngine: module_cdp_engine,
		CDPTreasury: module_cdp_treasury,
		Loans: module_loans,
		Honzon: module_honzon,
		EVMBridge: module_evm_bridge exclude_parts { Call },
		AssetRegistry: module_asset_registry,
		NFTModule: module_nft,
		TransactionPause: module_transaction_pause,
		TransactionPayment: module_transaction_payment,
		Prices: module_prices,
		Proxy: pallet_proxy,
		Utility: pallet_utility,
		Scheduler: pallet_scheduler,
		DexModule: module_dex,
		EVMModule: module_evm,
		EvmAccounts: module_evm_accounts,
		IdleScheduler: module_idle_scheduler,
		Homa: module_homa,
		Incentives: module_incentives,
		Rewards: orml_rewards,
		XTokens: orml_xtokens,
		StableAsset: nutsfinance_stable_asset,
		LiquidCrowdloan: module_liquid_crowdloan,
		Earning: module_earning,
	}
);

impl<LocalCall> SendTransactionTypes<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = UncheckedExtrinsic;
}

#[cfg(test)]
// This function basically just builds a genesis storage key/value store
// according to our desired mockup.
pub fn new_test_ext() -> sp_io::TestExternalities {
	use frame_support::assert_ok;
	use sp_runtime::BuildStorage;
	use sp_std::collections::btree_map::BTreeMap;

	let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

	let mut accounts = BTreeMap::new();
	let mut evm_genesis_accounts = crate::evm_genesis(vec![]);
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
	module_evm::GenesisConfig::<Test> {
		chain_id: 595,
		accounts,
	}
	.assimilate_storage(&mut storage)
	.unwrap();
	module_asset_registry::GenesisConfig::<Test> {
		assets: vec![(ACA, ExistenceRequirement::get()), (DOT, 0)],
	}
	.assimilate_storage(&mut storage)
	.unwrap();

	let mut ext = sp_io::TestExternalities::new(storage);
	ext.execute_with(|| {
		System::set_block_number(1);
		Timestamp::set_timestamp(1);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE,
			DOT,
			1_000_000_000_000
		));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE,
			AUSD,
			1_000_000_000
		));

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			EvmAddressMapping::<Test>::get_account_id(&alice_evm_addr()),
			DOT,
			1_000_000_000
		));

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			EvmAddressMapping::<Test>::get_account_id(&alice_evm_addr()),
			AUSD,
			1_000_000_000
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
