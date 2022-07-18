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

//! Mocks for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use primitives::{TokenSymbol, TradingPair};
use sp_core::H256;
use sp_runtime::{
	testing::{Header, TestXt},
	traits::{AccountIdConversion, IdentityLookup, One as OneT},
};
use sp_std::cell::RefCell;
pub use support::Price;
use support::{mocks::MockStableAsset, SpecificJointsSwap};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type AuctionId = u32;
pub type Amount = i64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CAROL: AccountId = 3;
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::RENBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);

mod auction_manager {
	pub use super::super::*;
}

impl frame_system::Config for Runtime {
	type Origin = Origin;
	type Index = u64;
	type BlockNumber = BlockNumber;
	type Call = Call;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
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
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}

impl orml_auction::Config for Runtime {
	type Event = Event;
	type Balance = Balance;
	type AuctionId = AuctionId;
	type Handler = AuctionManagerModule;
	type WeightInfo = ();
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const MaxAuctionsCount: u32 = 10_000;
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub TreasuryAccount: AccountId = PalletId(*b"aca/hztr").into_account_truncating();
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![DOT],
	];
}

impl cdp_treasury::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = AuctionManagerModule;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type DEX = DEXModule;
	type Swap = SpecificJointsSwap<DEXModule, AlternativeSwapPathJointList>;
	type MaxAuctionsCount = MaxAuctionsCount;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
	type StableAsset = MockStableAsset<CurrencyId, Balance, AccountId, BlockNumber>;
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
	fn get_relative_price(_base: CurrencyId, _quote: CurrencyId) -> Option<Price> {
		RELATIVE_PRICE.with(|v| *v.borrow_mut())
	}

	fn get_price(_currency_id: CurrencyId) -> Option<Price> {
		None
	}
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, BTC).unwrap(),
		TradingPair::from_currency_ids(DOT, BTC).unwrap(),
		TradingPair::from_currency_ids(AUSD, DOT).unwrap()
	];
}

impl module_dex::Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
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

parameter_types! {
	pub MinimumIncrementSize: Rate = Rate::saturating_from_rational(1, 20);
}

impl Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type Auction = AuctionModule;
	type MinimumIncrementSize = MinimumIncrementSize;
	type AuctionTimeToClose = ConstU64<100>;
	type AuctionDurationSoftCap = ConstU64<2000>;
	type GetStableCurrencyId = GetStableCurrencyId;
	type CDPTreasury = CDPTreasuryModule;
	type PriceSource = MockPriceSource;
	type UnsignedPriority = ConstU64<1048576>; // 1 << 20
	type EmergencyShutdown = MockEmergencyShutdown;
	type WeightInfo = ();
}

pub type Block = sp_runtime::generic::Block<Header, UncheckedExtrinsic>;
pub type UncheckedExtrinsic = sp_runtime::generic::UncheckedExtrinsic<u32, Call, u32, ()>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		AuctionManagerModule: auction_manager::{Pallet, Storage, Call, Event<T>, ValidateUnsigned},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
		AuctionModule: orml_auction::{Pallet, Storage, Call, Event<T>},
		CDPTreasuryModule: cdp_treasury::{Pallet, Storage, Call, Event<T>},
		DEXModule: module_dex::{Pallet, Storage, Call, Event<T>, Config<T>},
	}
);

pub type Extrinsic = TestXt<Call, ()>;

impl<LocalCall> frame_system::offchain::SendTransactionTypes<LocalCall> for Runtime
where
	Call: From<LocalCall>,
{
	type OverarchingCall = Call;
	type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, AUSD, 1000),
				(BOB, AUSD, 1000),
				(CAROL, AUSD, 1000),
				(ALICE, BTC, 1000),
				(BOB, BTC, 1000),
				(CAROL, BTC, 1000),
				(ALICE, DOT, 1000),
				(BOB, DOT, 1000),
				(CAROL, DOT, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
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

		t.into()
	}

	pub fn lots_of_accounts() -> Self {
		let mut balances = Vec::new();
		for i in 0..1001 {
			let account_id: AccountId = i;
			balances.push((account_id, BTC, 1000));
		}
		Self { balances }
	}
}
