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

//! Mocks for the Starport module.

#![cfg(test)]

use super::*;
use frame_support::{ord_parameter_types, parameter_types, PalletId};
use frame_system::{offchain::SendTransactionTypes, EnsureSignedBy};
use module_support::{
	mocks::MockAddressMapping, AuctionManager, EmergencyShutdown, Price, PriceProvider, Rate, RiskManager,
};
use orml_traits::{parameter_type_with_key, Happened, XcmTransfer};
use primitives::{Amount, AuctionId, Moment, TokenSymbol, TradingPair};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{AccountIdConversion, Convert, IdentityLookup, One as OneT},
	AccountId32,
};
use sp_std::cell::RefCell;
use std::collections::HashMap;
use xcm::opaque::v0::{Junction, MultiAsset, MultiLocation, NetworkId};

pub type AccountId = AccountId32;
pub type BlockNumber = u64;
use crate as module_homa_lite;

mod homa_lite {
	pub use super::super::*;
}

pub const ROOT: AccountId = AccountId32::new([255u8; 32]);
pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const INVALID_CALLER: AccountId = AccountId32::new([254u8; 32]);
pub const ACALA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const KSM: CurrencyId = CurrencyId::Token(TokenSymbol::KSM);
pub const LKSM: CurrencyId = CurrencyId::Token(TokenSymbol::LKSM);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const INITIAL_BALANCE: Balance = 1_000_000;
pub const MOCK_XCM_DESTINATION: MultiLocation = MultiLocation::X1(Junction::AccountId32 {
	network: NetworkId::Kusama,
	id: [1u8; 32],
});

/// For testing only. Does not check for overflow.
pub fn dollar(b: Balance) -> Balance {
	b * 1_000_000_000_000
}

/// For testing only. Does not check for overflow.
pub fn millicent(b: Balance) -> Balance {
	b * 10_000_000
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

/// A mock XCM transfer.
/// Only fails if it is called by "INVALID_CALLER". Otherwise returns OK with 0 weight.
pub struct MockXcm;
impl XcmTransfer<AccountId, Balance, CurrencyId> for MockXcm {
	fn transfer(
		who: AccountId,
		_currency_id: CurrencyId,
		_amount: Balance,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		match who {
			INVALID_CALLER => Err(DispatchError::Other("invalid caller")),
			_ => Ok(()),
		}
	}

	/// Transfer `MultiAsset`
	fn transfer_multi_asset(
		_who: AccountId,
		_asset: MultiAsset,
		_dest: MultiLocation,
		_dest_weight: Weight,
	) -> DispatchResult {
		Ok(())
	}
}

impl frame_system::Config for Runtime {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
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
	type DustRemovalWhitelist = ();
}

parameter_types! {
	pub const NativeTokenExistentialDeposit: Balance = 0;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = NativeTokenExistentialDeposit;
	type AccountStore = frame_system::Pallet<Runtime>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

pub type AdaptedBasicCurrency = module_currencies::BasicCurrencyAdapter<Runtime, PalletBalances, Amount, BlockNumber>;

parameter_types! {
	pub const GetNativeCurrencyId: CurrencyId = ACALA;
}

impl module_currencies::Config for Runtime {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = AdaptedBasicCurrency;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type WeightInfo = ();
	type AddressMapping = MockAddressMapping;
	type EVMBridge = ();
}

// mock convert
pub struct MockConvert;
impl Convert<(CurrencyId, Balance), Balance> for MockConvert {
	fn convert(a: (CurrencyId, Balance)) -> Balance {
		a.1 / Balance::from(2u64)
	}
}

// mock risk manager
pub struct MockRiskManager;
impl RiskManager<AccountId, CurrencyId, Balance, Balance> for MockRiskManager {
	fn get_bad_debt_value(currency_id: CurrencyId, debit_balance: Balance) -> Balance {
		MockConvert::convert((currency_id, debit_balance))
	}

	fn check_position_valid(
		currency_id: CurrencyId,
		_collateral_balance: Balance,
		_debit_balance: Balance,
		check_required_ratio: bool,
	) -> DispatchResult {
		match currency_id {
			KSM => {
				if check_required_ratio {
					Err(sp_runtime::DispatchError::Other(
						"mock below required collateral ratio error",
					))
				} else {
					Err(sp_runtime::DispatchError::Other("mock below liquidation ratio error"))
				}
			}
			_ => Err(sp_runtime::DispatchError::Other("mock below liquidation ratio error")),
		}
	}

	fn check_debit_cap(currency_id: CurrencyId, total_debit_balance: Balance) -> DispatchResult {
		match (currency_id, total_debit_balance) {
			(KSM, 1000) => Err(sp_runtime::DispatchError::Other("mock exceed debit value cap error")),
			(_, _) => Ok(()),
		}
	}
}

thread_local! {
	pub static DOT_SHARES: RefCell<HashMap<AccountId, Balance>> = RefCell::new(HashMap::new());
}

pub struct MockOnUpdateLoan;
impl Happened<(AccountId, CurrencyId, Amount, Balance)> for MockOnUpdateLoan {
	fn happened(info: &(AccountId, CurrencyId, Amount, Balance)) {
		let (who, currency_id, adjustment, previous_amount) = info;
		let adjustment_abs =
			sp_std::convert::TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();
		let new_share_amount = if adjustment.is_positive() {
			previous_amount.saturating_add(adjustment_abs)
		} else {
			previous_amount.saturating_sub(adjustment_abs)
		};

		if *currency_id == KSM {
			DOT_SHARES.with(|v| {
				let mut old_map = v.borrow().clone();
				old_map.insert(who.clone(), new_share_amount);
				*v.borrow_mut() = old_map;
			});
		}
	}
}

parameter_types! {
	pub const LoansPalletId: PalletId = PalletId(*b"aca/loan");
}

impl module_loans::Config for Runtime {
	type Event = Event;
	type Convert = MockConvert;
	type Currency = Currencies;
	type RiskManager = MockRiskManager;
	type CDPTreasury = CDPTreasury;
	type PalletId = LoansPalletId;
	type OnUpdateLoan = MockOnUpdateLoan;
}

thread_local! {
	static RELATIVE_PRICE: RefCell<Option<Price>> = RefCell::new(Some(Price::one()));
}

pub struct MockPriceSource;
impl MockPriceSource {
	pub fn set_relative_price(price: Option<Price>) {
		RELATIVE_PRICE.with(|v| *v.borrow_mut() = price);
	}
}
impl PriceProvider<CurrencyId> for MockPriceSource {
	fn get_relative_price(base: CurrencyId, quote: CurrencyId) -> Option<Price> {
		match (base, quote) {
			(AUSD, KSM) => RELATIVE_PRICE.with(|v| *v.borrow_mut()),
			(KSM, AUSD) => RELATIVE_PRICE.with(|v| *v.borrow_mut()),
			_ => None,
		}
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		unimplemented!()
	}
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

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub TreasuryAccount: AccountId = PalletId(*b"aca/hztr").into_account();
}

impl cdp_treasury::Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = DEXModule;
	type MaxAuctionsCount = MaxAuctionsCount;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub const TradingPathLimit: u32 = 3;
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, KSM).unwrap(),
		TradingPair::from_currency_ids(ACALA, KSM).unwrap(),
		TradingPair::from_currency_ids(ACALA, AUSD).unwrap(),
	];
}

impl dex::Config for Runtime {
	type Event = Event;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = TradingPathLimit;
	type PalletId = DEXPalletId;
	type CurrencyIdMapping = ();
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<One, AccountId>;
}

parameter_types! {
	pub const MinimumPeriod: Moment = 1000;
}
impl pallet_timestamp::Config for Runtime {
	type Moment = Moment;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

thread_local! {
	static IS_SHUTDOWN: RefCell<bool> = RefCell::new(false);
}

pub fn mock_shutdown() {
	IS_SHUTDOWN.with(|v| *v.borrow_mut() = true)
}

pub struct MockEmergencyShutdown;
impl EmergencyShutdown for MockEmergencyShutdown {
	fn is_shutdown() -> bool {
		IS_SHUTDOWN.with(|v| *v.borrow_mut())
	}
}

ord_parameter_types! {
	pub const One: AccountId = ALICE;
}

parameter_types! {
	pub DefaultLiquidationRatio: Ratio = Ratio::saturating_from_rational(3, 2);
	pub DefaultDebitExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(1, 10);
	pub DefaultLiquidationPenalty: Rate = Rate::saturating_from_rational(10, 100);
	pub const MinimumDebitValue: Balance = 2;
	pub MaxSwapSlippageCompareToOracle: Ratio = Ratio::saturating_from_rational(50, 100);
	pub const UnsignedPriority: u64 = 1 << 20;
	pub CollateralCurrencyIds: Vec<CurrencyId> = vec![KSM];
	pub DefaultSwapParitalPathList: Vec<Vec<CurrencyId>> = vec![
		vec![AUSD],
		vec![ACALA, AUSD],
	];
}

impl cdp_engine::Config for Runtime {
	type Event = Event;
	type PriceSource = MockPriceSource;
	type CollateralCurrencyIds = CollateralCurrencyIds;
	type DefaultLiquidationRatio = DefaultLiquidationRatio;
	type DefaultDebitExchangeRate = DefaultDebitExchangeRate;
	type DefaultLiquidationPenalty = DefaultLiquidationPenalty;
	type MinimumDebitValue = MinimumDebitValue;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CDPTreasury;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type MaxSwapSlippageCompareToOracle = MaxSwapSlippageCompareToOracle;
	type UnsignedPriority = UnsignedPriority;
	type EmergencyShutdown = MockEmergencyShutdown;
	type UnixTime = Timestamp;
	type DefaultSwapParitalPathList = DefaultSwapParitalPathList;
	type WeightInfo = ();
}

parameter_types! {
	pub const StakingCurrencyId: CurrencyId = KSM;
	pub const LiquidCurrencyId: CurrencyId = LKSM;
	pub MinimumMintThreshold: Balance = millicent(1);
	pub const MockXcmDestination: MultiLocation = MOCK_XCM_DESTINATION;
	pub DefaultExchangeRate: ExchangeRate = ExchangeRate::saturating_from_rational(10, 1);
	pub const MaxRewardPerEra: Permill = Permill::from_percent(1);
	pub MintFee: Balance = millicent(1000);
}
ord_parameter_types! {
	pub const Root: AccountId = ROOT;
}

impl Config for Runtime {
	type Event = Event;
	type WeightInfo = ();
	type Currency = Currencies;
	type StakingCurrencyId = StakingCurrencyId;
	type LiquidCurrencyId = LiquidCurrencyId;
	type GovernanceOrigin = EnsureSignedBy<Root, AccountId>;
	type Loan = CDPEngine;
	type MinimumMintThreshold = MinimumMintThreshold;
	type XcmTransfer = MockXcm;
	type SovereignSubAccountLocation = MockXcmDestination;
	type DefaultExchangeRate = DefaultExchangeRate;
	type MaxRewardPerEra = MaxRewardPerEra;
	type MintFee = MintFee;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

frame_support::construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		HomaLite: module_homa_lite::{Pallet, Call, Storage, Event<T>},
		PalletBalances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		Currencies: module_currencies::{Pallet, Call, Event<T>},
		Loans: module_loans::{Pallet, Storage, Call, Event<T>},
		CDPTreasury: cdp_treasury::{Pallet, Storage, Call, Event<T>},
		DEXModule: dex::{Pallet, Storage, Call, Event<T>, Config<T>},
		Timestamp: pallet_timestamp::{Pallet, Call, Storage, Inherent},
		CDPEngine: cdp_engine::{Pallet, Storage, Call, Event<T>, Config, ValidateUnsigned},
	}
);

/// An extrinsic type used for tests.
pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> SendTransactionTypes<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
	tokens_balances: Vec<(AccountId, CurrencyId, Balance)>,
	native_balances: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		let initial = dollar(INITIAL_BALANCE);
		Self {
			tokens_balances: vec![
				(ALICE, KSM, initial),
				(BOB, KSM, initial),
				(ROOT, LKSM, initial),
				(INVALID_CALLER, KSM, initial),
			],
			native_balances: vec![
				(ALICE, initial),
				(BOB, initial),
				(ROOT, initial),
				(INVALID_CALLER, initial),
			],
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
