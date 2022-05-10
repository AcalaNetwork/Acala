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

//! Mocks for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::EnsureSignedBy;
use nutsfinance_stable_asset::{
	PoolTokenIndex, RedeemProportionResult, StableAssetPoolId, StableAssetPoolInfo, SwapResult,
};
use orml_traits::{parameter_type_with_key, MultiReservableCurrency};
use primitives::{Amount, TokenSymbol};
use sp_core::H256;
use sp_runtime::{testing::Header, traits::IdentityLookup};
use sp_std::cell::RefCell;
use support::mocks::MockErc20InfoMapping;

pub type BlockNumber = u64;
pub type AccountId = u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::Token(TokenSymbol::RENBTC);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);

parameter_types! {
	pub static AUSDBTCPair: TradingPair = TradingPair::from_currency_ids(AUSD, BTC).unwrap();
	pub static AUSDDOTPair: TradingPair = TradingPair::from_currency_ids(AUSD, DOT).unwrap();
	pub static DOTBTCPair: TradingPair = TradingPair::from_currency_ids(DOT, BTC).unwrap();
}

mod dex {
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
	pub const ListingOrigin: AccountId = 3;
}

parameter_types! {
	pub const GetExchangeFee: (u32, u32) = (1, 100);
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
}

thread_local! {
	pub static AUSD_DOT_POOL_RECORD: RefCell<(Balance, Balance)> = RefCell::new((0, 0));
}

pub struct MockOnLiquidityPoolUpdated;
impl Happened<(TradingPair, Balance, Balance)> for MockOnLiquidityPoolUpdated {
	fn happened(info: &(TradingPair, Balance, Balance)) {
		let (trading_pair, new_pool_0, new_pool_1) = info;
		if *trading_pair == AUSDDOTPair::get() {
			AUSD_DOT_POOL_RECORD.with(|v| *v.borrow_mut() = (*new_pool_0, *new_pool_1));
		}
	}
}

impl Config for Runtime {
	type Event = Event;
	type Currency = Tokens;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = ConstU32<3>;
	type PalletId = DEXPalletId;
	type Erc20InfoMapping = MockErc20InfoMapping;
	type WeightInfo = ();
	type DEXIncentives = MockDEXIncentives;
	type ListingOrigin = EnsureSignedBy<ListingOrigin, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<2000>;
	type OnLiquidityPoolUpdated = MockOnLiquidityPoolUpdated;
	type StableAsset = MockStableAsset;
}

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Runtime>;
type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Storage, Config, Event<T>},
		DexModule: dex::{Pallet, Storage, Call, Event<T>, Config<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
	initial_listing_trading_pairs: Vec<(TradingPair, (Balance, Balance), (Balance, Balance), BlockNumber)>,
	initial_enabled_trading_pairs: Vec<TradingPair>,
	initial_added_liquidity_pools: Vec<(AccountId, Vec<(TradingPair, (Balance, Balance))>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, AUSD, 1_000_000_000_000_000_000u128),
				(BOB, AUSD, 1_000_000_000_000_000_000u128),
				(ALICE, BTC, 1_000_000_000_000_000_000u128),
				(BOB, BTC, 1_000_000_000_000_000_000u128),
				(ALICE, DOT, 1_000_000_000_000_000_000u128),
				(BOB, DOT, 1_000_000_000_000_000_000u128),
			],
			initial_listing_trading_pairs: vec![],
			initial_enabled_trading_pairs: vec![],
			initial_added_liquidity_pools: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn initialize_enabled_trading_pairs(mut self) -> Self {
		self.initial_enabled_trading_pairs = vec![AUSDDOTPair::get(), AUSDBTCPair::get(), DOTBTCPair::get()];
		self
	}

	pub fn initialize_added_liquidity_pools(mut self, who: AccountId) -> Self {
		self.initial_added_liquidity_pools = vec![(
			who,
			vec![
				(AUSDDOTPair::get(), (1_000_000u128, 2_000_000u128)),
				(AUSDBTCPair::get(), (1_000_000u128, 2_000_000u128)),
				(DOTBTCPair::get(), (1_000_000u128, 2_000_000u128)),
			],
		)];
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		orml_tokens::GenesisConfig::<Runtime> {
			balances: self.balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		dex::GenesisConfig::<Runtime> {
			initial_listing_trading_pairs: self.initial_listing_trading_pairs,
			initial_enabled_trading_pairs: self.initial_enabled_trading_pairs,
			initial_added_liquidity_pools: self.initial_added_liquidity_pools,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

pub struct MockStableAsset;

impl StableAsset for MockStableAsset {
	type AssetId = CurrencyId;
	type AtLeast64BitUnsigned = Balance;
	type Balance = Balance;
	type AccountId = AccountId;
	type BlockNumber = BlockNumber;

	fn pool_count() -> StableAssetPoolId {
		unimplemented!()
	}

	fn pool(
		_id: StableAssetPoolId,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		Some(StableAssetPoolInfo {
			pool_asset: CurrencyId::StableAssetPoolToken(0),
			assets: vec![
				CurrencyId::Token(TokenSymbol::RENBTC),
				CurrencyId::Token(TokenSymbol::DOT),
			],
			precisions: vec![1, 1],
			mint_fee: 0,
			swap_fee: 0,
			redeem_fee: 0,
			total_supply: 0,
			a: 100,
			a_block: 1,
			future_a: 100,
			future_a_block: 1,
			balances: vec![0, 0],
			fee_recipient: 0,
			account_id: 1,
			yield_recipient: 2,
			precision: 1,
		})
	}

	fn create_pool(
		_pool_asset: Self::AssetId,
		_assets: Vec<Self::AssetId>,
		_precisions: Vec<Self::Balance>,
		_mint_fee: Self::Balance,
		_swap_fee: Self::Balance,
		_redeem_fee: Self::Balance,
		_initial_a: Self::Balance,
		_fee_recipient: Self::AccountId,
		_yield_recipient: Self::AccountId,
		_precision: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn mint(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amounts: Vec<Self::Balance>,
		_min_mint_amount: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn swap(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_i: PoolTokenIndex,
		_j: PoolTokenIndex,
		_dx: Self::Balance,
		_min_dy: Self::Balance,
		_asset_length: u32,
	) -> sp_std::result::Result<(Self::Balance, Self::Balance), DispatchError> {
		Ok((100_000, 100_000))
	}

	fn redeem_proportion(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_min_redeem_amounts: Vec<Self::Balance>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn redeem_single(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_i: PoolTokenIndex,
		_min_redeem_amount: Self::Balance,
		_asset_length: u32,
	) -> DispatchResult {
		unimplemented!()
	}

	fn redeem_multi(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amounts: Vec<Self::Balance>,
		_max_redeem_amount: Self::Balance,
	) -> DispatchResult {
		unimplemented!()
	}

	fn collect_fee(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn update_balance(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn collect_yield(
		_pool_id: StableAssetPoolId,
		_pool_info: &mut StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> DispatchResult {
		unimplemented!()
	}

	fn modify_a(_pool_id: StableAssetPoolId, _a: Self::Balance, _future_a_block: Self::BlockNumber) -> DispatchResult {
		unimplemented!()
	}

	fn get_collect_yield_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		unimplemented!()
	}

	fn get_balance_update_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		unimplemented!()
	}

	fn get_redeem_proportion_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
		_amount_bal: Self::Balance,
	) -> Option<RedeemProportionResult<Self::Balance>> {
		unimplemented!()
	}

	fn get_best_route(
		_input_asset: Self::AssetId,
		_output_asset: Self::AssetId,
		limit: Self::Balance,
	) -> Option<
		StableAssetPoolInfo<
			Self::AssetId,
			Self::AtLeast64BitUnsigned,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	> {
		if limit > 100_000 {
			None
		} else {
			Some(StableAssetPoolInfo {
				pool_asset: CurrencyId::StableAssetPoolToken(0),
				assets: vec![AUSD, DOT],
				precisions: vec![1u128, 1u128],
				mint_fee: 1u128,
				swap_fee: 1u128,
				redeem_fee: 1u128,
				total_supply: 0u128,
				a: 1u128,
				a_block: 0,
				future_a: 1u128,
				future_a_block: 0,
				balances: vec![0, 0],
				fee_recipient: 1u128,
				account_id: 2u128,
				yield_recipient: 3u128,
				precision: 1000000000000000000u128,
			})
		}
	}

	fn get_swap_amount_exact(
		_pool_id: StableAssetPoolId,
		_input_index: PoolTokenIndex,
		_output_index: PoolTokenIndex,
		dy_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		if dy_bal > 100_000 {
			None
		} else {
			Some(SwapResult {
				dx: 100_000,
				dy: dy_bal,
				y: 100_100,
				balance_i: 100_000,
			})
		}
	}
}
