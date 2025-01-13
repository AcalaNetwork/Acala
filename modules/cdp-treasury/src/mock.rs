// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

//! Mocks for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, derive_impl, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, EitherOfDiverse, Nothing},
};
use frame_system::{EnsureRoot, EnsureSignedBy};
use module_support::SpecificJointsSwap;
use nutsfinance_stable_asset::traits::StableAsset;
use nutsfinance_stable_asset::{
	PoolTokenIndex, RedeemProportionResult, StableAssetPoolId, StableAssetPoolInfo, SwapResult,
};
use orml_traits::parameter_type_with_key;
use primitives::{DexShare, TokenSymbol, TradingPair};
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Amount = i64;
pub type AuctionId = u32;

pub const ALICE: AccountId = 0;
pub const BOB: AccountId = 1;
pub const CHARLIE: AccountId = 2;
pub const ACA: CurrencyId = CurrencyId::Token(TokenSymbol::ACA);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const BTC: CurrencyId = CurrencyId::ForeignAsset(255);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const STABLE_ASSET_LP: CurrencyId = CurrencyId::StableAssetPoolToken(0);
pub const LP_AUSD_DOT: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::AUSD), DexShare::Token(TokenSymbol::DOT));

mod cdp_treasury {
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
	type ReserveIdentifier = [u8; 8];
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
	type ReserveIdentifier = [u8; 8];
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
	pub const GetStableCurrencyId: CurrencyId = AUSD;
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![
		TradingPair::from_currency_ids(AUSD, BTC).unwrap(),
		TradingPair::from_currency_ids(AUSD, DOT).unwrap(),
		TradingPair::from_currency_ids(BTC, DOT).unwrap(),
	];
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
}

impl module_dex::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetExchangeFee = GetExchangeFee;
	type TradingPathLimit = ConstU32<4>;
	type PalletId = DEXPalletId;
	type GetNativeCurrencyId = GetNativeCurrencyId;
	type Erc20InfoMapping = ();
	type DEXIncentives = ();
	type WeightInfo = ();
	type ListingOrigin = EnsureSignedBy<One, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<0>;
	type OnLiquidityPoolUpdated = ();
}

parameter_types! {
	pub static TotalCollateralAuction: u32 = 0;
	pub static TotalCollateralInAuction: Balance = 0;
}

pub struct MockAuctionManager;
impl AuctionManager<AccountId> for MockAuctionManager {
	type CurrencyId = CurrencyId;
	type Balance = Balance;
	type AuctionId = AuctionId;

	fn new_collateral_auction(
		_refund_recipient: &AccountId,
		_currency_id: Self::CurrencyId,
		amount: Self::Balance,
		_target: Self::Balance,
	) -> DispatchResult {
		TotalCollateralAuction::mutate(|v| *v += 1);
		TotalCollateralInAuction::mutate(|v| *v += amount);
		Ok(())
	}

	fn cancel_auction(_id: Self::AuctionId) -> DispatchResult {
		unimplemented!()
	}

	fn get_total_collateral_in_auction(_id: Self::CurrencyId) -> Self::Balance {
		TOTAL_COLLATERAL_IN_AUCTION.with(|v| *v.borrow_mut())
	}

	fn get_total_target_in_auction() -> Self::Balance {
		unimplemented!()
	}
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

parameter_types! {
	pub const CDPTreasuryPalletId: PalletId = PalletId(*b"aca/cdpt");
	pub const TreasuryAccount: AccountId = 10;
	pub AlternativeSwapPathJointList: Vec<Vec<CurrencyId>> = vec![
		vec![DOT],
	];
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type GetStableCurrencyId = GetStableCurrencyId;
	type AuctionManagerHandler = MockAuctionManager;
	type UpdateOrigin = EitherOfDiverse<EnsureRoot<AccountId>, EnsureSignedBy<One, AccountId>>;
	type DEX = DEXModule;
	type Swap = SpecificJointsSwap<DEXModule, AlternativeSwapPathJointList>;
	type MaxAuctionsCount = ConstU32<5>;
	type PalletId = CDPTreasuryPalletId;
	type TreasuryAccount = TreasuryAccount;
	type WeightInfo = ();
	type StableAsset = MockStableAsset;
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime {
		System: frame_system,
		CDPTreasuryModule: cdp_treasury,
		Currencies: orml_currencies,
		Tokens: orml_tokens,
		PalletBalances: pallet_balances,
		DEXModule: module_dex,
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, DOT, 1000),
				(ALICE, AUSD, 1000),
				(ALICE, BTC, 1000),
				(ALICE, STABLE_ASSET_LP, 1000),
				(BOB, DOT, 1000),
				(BOB, AUSD, 1000),
				(BOB, BTC, 1000),
				(BOB, STABLE_ASSET_LP, 1000),
				(CHARLIE, DOT, 1000),
				(CHARLIE, BTC, 1000),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
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
			assets: vec![CurrencyId::ForeignAsset(255), CurrencyId::Token(TokenSymbol::DOT)],
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
		unimplemented!()
	}

	fn redeem_proportion(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_min_redeem_amounts: Vec<Self::Balance>,
	) -> DispatchResult {
		Ok(())
	}

	fn redeem_single(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_i: PoolTokenIndex,
		_min_redeem_amount: Self::Balance,
		_asset_length: u32,
	) -> sp_std::result::Result<(Self::Balance, Self::Balance), DispatchError> {
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
		Some(StableAssetPoolInfo {
			pool_asset: CurrencyId::StableAssetPoolToken(0),
			assets: vec![CurrencyId::ForeignAsset(255), CurrencyId::Token(TokenSymbol::DOT)],
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

	fn get_balance_update_amount(
		_pool_info: &StableAssetPoolInfo<
			Self::AssetId,
			Self::Balance,
			Self::Balance,
			Self::AccountId,
			Self::BlockNumber,
		>,
	) -> Option<StableAssetPoolInfo<Self::AssetId, Self::Balance, Self::Balance, Self::AccountId, Self::BlockNumber>> {
		Some(StableAssetPoolInfo {
			pool_asset: CurrencyId::StableAssetPoolToken(0),
			assets: vec![CurrencyId::ForeignAsset(255), CurrencyId::Token(TokenSymbol::DOT)],
			precisions: vec![1, 1],
			mint_fee: 0,
			swap_fee: 0,
			redeem_fee: 0,
			total_supply: 1000,
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
		Some(RedeemProportionResult {
			amounts: vec![100, 100],
			balances: vec![0, 0],
			fee_amount: 0,
			total_supply: 0,
			redeem_amount: 0,
		})
	}

	fn get_best_route(
		_input_asset: Self::AssetId,
		_output_asset: Self::AssetId,
		_input_amount: Self::Balance,
	) -> Option<(StableAssetPoolId, PoolTokenIndex, PoolTokenIndex, Self::Balance)> {
		unimplemented!()
	}

	fn get_swap_output_amount(
		_pool_id: StableAssetPoolId,
		_input_index: PoolTokenIndex,
		_output_index: PoolTokenIndex,
		_dx_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		unimplemented!()
	}

	fn get_swap_input_amount(
		_pool_id: StableAssetPoolId,
		_input_index: PoolTokenIndex,
		_output_index: PoolTokenIndex,
		_dy_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		unimplemented!()
	}
}
