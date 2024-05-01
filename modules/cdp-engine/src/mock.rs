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

//! Mocks for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
use module_support::{
	mocks::{MockStableAsset, TestRandomness},
	AuctionManager, EmergencyShutdown, SpecificJointsSwap,
};
use orml_traits::parameter_type_with_key;
use primitives::{evm::convert_decimals_to_evm, DexShare, Moment, ReserveIdentifier, TokenSymbol, TradingPair};
use sp_core::crypto::AccountId32;
use sp_runtime::{
	testing::TestXt,
	traits::{AccountIdConversion, IdentityLookup, One as OneT},
	BuildStorage,
};
use sp_std::str::FromStr;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
pub type AuctionId = u32;

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const CAROL: AccountId = AccountId32::new([3u8; 32]);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::ForeignAsset(255);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LP_AUSD_DOT: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::AUSD), DexShare::Token(TokenSymbol::DOT));
pub const LP_DOT_BTC: CurrencyId = CurrencyId::DexShare(DexShare::ForeignAsset(255), DexShare::Token(TokenSymbol::DOT));

mod cdp_engine {
	pub use super::super::*;
}

#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Block = Block;
	type AccountData = pallet_balances::AccountData<Balance>;
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

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ReserveIdentifier;
	type WeightInfo = ();
	type RuntimeHoldReason = RuntimeHoldReason;
	type RuntimeFreezeReason = RuntimeFreezeReason;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
}
pub type AdaptedBasicCurrency = orml_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACA;
}

impl orml_currencies::Config for Runtime {
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"aca/loan");
}

impl module_loans::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type RiskManager = CDPEngineModule;
	type CDPTreasury = CDPTreasuryModule;
	type PalletId = LoansPalletId;
	type OnUpdateLoan = ();
}

parameter_types! {
	static BtcPrice: Option<Price> = Some(Price::one());
	static DotPrice: Option<Price> = Some(Price::one());
	static LpAusdDotPrice: Option<Price> = Some(Price::one());
	static LpDotBtcPrice: Option<Price> = Some(Price::one());
}

pub struct MockPriceSource;
impl MockPriceSource {
	pub fn set_price(currency_id: CurrencyId, price: Option<Price>) {
		match currency_id {
			BTC => BtcPrice::mutate(|v| *v = price),
			DOT => DotPrice::mutate(|v| *v = price),
			LP_AUSD_DOT => LpAusdDotPrice::mutate(|v| *v = price),
			LP_DOT_BTC => LpDotBtcPrice::mutate(|v| *v = price),
			_ => {}
		}
	}
}
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_price(currency_id: CurrencyId) -> Option<Price> {
		match currency_id {
			BTC => BtcPrice::get(),
			DOT => DotPrice::get(),
			AUSD => Some(Price::one()),
			LP_AUSD_DOT => LpAusdDotPrice::get(),
			LP_DOT_BTC => LpDotBtcPrice::get(),
			_ => None,
		}
	}
}

parameter_types! {
	pub static Auction: Option<(AccountId, CurrencyId, Balance, Balance)> = None;
}

pub struct MockAuctionManager;
impl MockAuctionManager {
	pub fn auction() -> Option<(AccountId, CurrencyId, Balance, Balance)> {
		Auction::get()
	}
}
impl AuctionManager<AccountId> for MockAuctionManager {
	type Balance = Balance;
	type CurrencyId = CurrencyId;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		refund_recipient: &AccountId,
		currency_id: Self::CurrencyId,
		amount: Self::Balance,
		target: Self::Balance,
	) -> DispatchResult {
		Auction::mutate(|v| *v = Some((refund_recipient.clone(), currency_id, amount, target)));
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		Auction::mutate(|v| *v = None);
		Ok(())
	}

	fn get_total_target_in_auction() -> Self::Balance {
		Self::auction().map(|auction| auction.3).unwrap_or_default()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		Self::auction().map(|auction| auction.2).unwrap_or_default()
	}
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub TreasuryAccount: AccountId = PalletId(*b"aca/hztr").into_account_truncating();
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![ACA],
	];
}

impl module_cdp_treasury::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = DEXModule;
	type Swap = SpecificJointsSwap<DEXModule, AlternativeSwapPathJointList>;
	type MaxAuctionsCount = ConstU32<10_000>;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
	type StableAsset = MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber>;
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, BTC).unwrap(),
		TradingPair::from_currency_ids(AUSD, DOT).unwrap(),
		TradingPair::from_currency_ids(ACA, BTC).unwrap(),
		TradingPair::from_currency_ids(ACA, DOT).unwrap(),
		TradingPair::from_currency_ids(ACA, AUSD).unwrap(),
	];
}

impl module_dex::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = ConstU32<4>;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = ();
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<One, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<0>;
	type OnLiquidityPoolUpdated = ();
}

impl pallet_timestamp::Config for Runtime {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = ConstU64<1_000>;
	type WeightInfo = ();
}

impl module_evm_accounts::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = PalletBalances;
	type ChainId = ();
	type AddressMapping = module_evm_accounts::EvmAddressMapping<Runtime>;
	type TransferAll = Currencies;
	type WeightInfo = ();
}

parameter_types! {
	pub NetworkContractSource: EvmAddress = EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap();
}

ord_parameter_types! {
	pub const CouncilAccount: AccountId = AccountId::from([1u8; 32]);
	pub const NetworkContractAccount: AccountId = AccountId::from([0u8; 32]);
	pub const StorageDepositPerByte: u128 = convert_decimals_to_evm(10);
}

impl module_evm::Config for Runtime {
	type AddressMapping = module_evm_accounts::EvmAddressMapping<Runtime>;
	type Currency = PalletBalances;
	type TransferAll = ();
	type NewContractExtraBytes = ConstU32<1>;
	type StorageDepositPerByte = StorageDepositPerByte;
	type TxFeePerGas = ConstU128<10>;
	type RuntimeEvent = RuntimeEvent;
	type PrecompilesType = ();
	type PrecompilesValue = ();
	type GasToWeight = ();
	type ChargeTransactionPayment = module_support::mocks::MockReservedTransactionPayment<PalletBalances>;
	type NetworkContractOrigin = EnsureSignedBy<NetworkContractAccount, AccountId>;
	type NetworkContractSource = NetworkContractSource;

	type DeveloperDeposit = ConstU128<1000>;
	type PublicationFee = ConstU128<200>;
	type TreasuryAccount = TreasuryAccount;
	type FreePublicationOrigin = EnsureSignedBy<CouncilAccount, AccountId>;

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
	static IsShutdown: bool = false;
}

pub fn mock_shutdown() {
	IsShutdown::mutate(|v| *v = true)
}

pub fn liquidation_contract_addr() -> EvmAddress {
	EvmAddress::from_str(&"0x1000000000000000000000000000000000000000").unwrap()
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IsShutdown::get()
	}
}

parameter_types! {
	static LIQUIDATED: (EvmAddress, EvmAddress, Balance, Balance) = (EvmAddress::default(), EvmAddress::default(), 0, 0);
	static TRANSFERRED: (EvmAddress, Balance) = (EvmAddress::default(), 0);
	static REFUNDED: (EvmAddress, Balance) = (EvmAddress::default(), 0);
	static LiquidationResult: DispatchResult = Err(Error::<Runtime>::LiquidationFailed.into());
	static REPAYMENT: Option<Balance> = None;
}

pub struct MockLiquidationEvmBridge;
impl MockLiquidationEvmBridge {
	pub fn liquidated() -> (EvmAddress, EvmAddress, Balance, Balance) {
		LIQUIDATED::get()
	}
	pub fn transferred() -> (EvmAddress, Balance) {
		TRANSFERRED::get()
	}
	pub fn refunded() -> (EvmAddress, Balance) {
		REFUNDED::get()
	}
	pub fn reset() {
		LiquidationResult::mutate(|v| *v = Err(Error::<Runtime>::LiquidationFailed.into()));
		REPAYMENT::mutate(|v| *v = None);
	}
	pub fn set_liquidation_result(r: DispatchResult) {
		LiquidationResult::mutate(|v| *v = r);
	}
	pub fn set_repayment(repayment: Balance) {
		REPAYMENT::mutate(|v| *v = Some(repayment));
	}
}
impl LiquidationEvmBridge for MockLiquidationEvmBridge {
	fn liquidate(
		_context: InvokeContext,
		collateral: EvmAddress,
		repay_dest: EvmAddress,
		amount: Balance,
		min_repayment: Balance,
	) -> DispatchResult {
		let result = LiquidationResult::get();
		if result.is_ok() {
			let repayment = if let Some(r) = REPAYMENT::get() {
				r
			} else {
				min_repayment
			};
			let _ = Currencies::deposit(GetStableCurrencyId::get(), &CDPEngineModule::account_id(), repayment);
		}
		LIQUIDATED::mutate(|v| *v = (collateral, repay_dest, amount, min_repayment));
		result
	}
	fn on_collateral_transfer(_context: InvokeContext, collateral: EvmAddress, amount: Balance) {
		TRANSFERRED::mutate(|v| *v = (collateral, amount));
	}
	fn on_repayment_refund(_context: InvokeContext, collateral: EvmAddress, repayment: Balance) {
		REFUNDED::mutate(|v| *v = (collateral, repayment));
	}
}

ord_parameter_types! {
	pub const One: AccountId = ALICE;
}

parameter_type_with_key! {
	pub MinimumCollateralAmount: |_currency_id: CurrencyId| -> Balance {
		10
	};
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(3, 2);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: FractionalRate = FractionalRate::try_from(Rate::saturating_from_rational(10, 100)).unwrap();
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::saturating_from_rational(50, 100);
	pub MaxLiquidationContractSlippage: Ratio = Ratio::saturating_from_rational(80, 100);
	pub const CDPEnginePalletId: PalletId = PalletId(*b"aca/cdpe");
	pub const SettleErc20EvmOrigin: AccountId = AccountId32::new([255u8; 32]);
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type PriceSource = MockPriceSource;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = ConstU128<2>;
	type MinimumCollateralAmount = MinimumCollateralAmount;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CDPTreasuryModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type UnsignedPriority = ConstU64<1048576>; // 1 << 20
	type EmergencyShutdown = MockEmergencyShutdown;
	type UnixTime = Timestamp;
	type Currency = Currencies;
	type DEX = DEXModule;
	type LiquidationContractsUpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxLiquidationContractSlippage = MaxLiquidationContractSlippage;
	type MaxLiquidationContracts = ConstU32<10>;
	type LiquidationEvmBridge = MockLiquidationEvmBridge;
	type PalletId = CDPEnginePalletId;
	type EvmAddressMapping = module_evm_accounts::EvmAddressMapping<Runtime>;
	type Swap = SpecificJointsSwap<DEXModule, AlternativeSwapPathJointList>;
	type EVMBridge = module_evm_bridge::EVMBridge<Runtime>;
	type SettleErc20EvmOrigin = SettleErc20EvmOrigin;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		CDPEngineModule: cdp_engine,
		CDPTreasuryModule: module_cdp_treasury,
		Currencies: orml_currencies,
		Tokens: orml_tokens,
		LoansModule: module_loans,
		PalletBalances: pallet_balances,
		DEXModule: module_dex,
		Timestamp: pallet_timestamp,
		EvmAccounts: module_evm_accounts,
		EVM: module_evm,
		EVMBridge: module_evm_bridge,
	}
);

/// An extrinsic type used for tests.
pub type Extrinsic = TestXt<RuntimeCall, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for Runtime
where
	RuntimeCall: From<LocalCall>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, BTC, 1000),
				(BOB, BTC, 1000),
				(CAROL, BTC, 10000),
				(ALICE, DOT, 1000),
				(BOB, DOT, 1000),
				(CAROL, DOT, 10000),
				(CAROL, AUSD, 10000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<Runtime> {
			balances: vec![(CAROL, 10000)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		module_dex::GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: EnabledTradingPairs::get(),
			initial_added_liquidity_pools: vec![],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		MockLiquidationEvmBridge::reset();

		t.into()
	}

	pub fn lots_of_accounts() -> Self {
		let mut balances = Vec::new();
		for i in 0..1001u32 {
			balances.push((account_id_from_u32(i), BTC, 1000));
		}
		Self { balances }
	}
}

pub fn account_id_from_u32(num: u32) -> AccountId {
	let mut data = [0u8; 32];
	let index = num.to_le_bytes();
	data[0..4].copy_from_slice(&index[..]);
	AccountId::new(data)
}
