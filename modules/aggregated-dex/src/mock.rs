// This file is part of Acala.

// Copyright (C) 2022 Acala Foundation.
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

//! Mocks for the Aggregated DEX module.

#![cfg(test)]

use super::*;
use frame_support::{
	ord_parameter_types, parameter_types,
	traits::{ConstU32, ConstU64, Everything, Nothing},
	PalletId,
};
use frame_system::{EnsureSignedBy, RawOrigin};
use nutsfinance_stable_asset::{RedeemProportionResult, StableAssetPoolInfo, SwapResult};
pub use orml_traits::{parameter_type_with_key, MultiCurrency};
use primitives::{Amount, TokenSymbol, TradingPair};
use sp_runtime::{
	testing::{Header, H256},
	traits::IdentityLookup,
	AccountId32, FixedPointNumber,
};
pub use support::ExchangeRate;

pub type AccountId = AccountId32;
pub type BlockNumber = u64;

mod aggregated_dex {
	pub use super::super::*;
}

pub const ALICE: AccountId = AccountId32::new([1u8; 32]);
pub const BOB: AccountId = AccountId32::new([2u8; 32]);
pub const AUSD: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
pub const DOT: CurrencyId = CurrencyId::Token(TokenSymbol::DOT);
pub const LDOT: CurrencyId = CurrencyId::Token(TokenSymbol::LDOT);
pub const STABLE_ASSET: CurrencyId = CurrencyId::StableAssetPoolToken(0);

impl frame_system::Config for Runtime {
	type BaseCallFilter = Everything;
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
	type BlockHashCount = ConstU64<250>;
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
	type MaxReserves = ();
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Nothing;
}

ord_parameter_types! {
	pub const Admin: AccountId = BOB;
}

parameter_types! {
	pub const DEXPalletId: PalletId = PalletId(*b"aca/dexm");
	pub const GetExchangeFee: (u32, u32) = (0, 100);
	pub EnabledTradingPairs: Vec<TradingPair> = vec![];
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
	type ListingOrigin = EnsureSignedBy<Admin, AccountId>;
	type ExtendedProvisioningBlocks = ConstU64<0>;
	type OnLiquidityPoolUpdated = ();
}

#[derive(Clone)]
pub struct TaigaSwapStatus {
	pub currency_id_0: CurrencyId,
	pub currency_id_1: CurrencyId,
	pub stable_asset_id: CurrencyId,
	pub exchange_rate_1_for_0: ExchangeRate,
}

pub struct MockStableAsset;
impl StableAssetT for MockStableAsset {
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
		TaigaConfig::get().map(|taiga_config| StableAssetPoolInfo {
			pool_asset: taiga_config.stable_asset_id,
			assets: vec![taiga_config.currency_id_0, taiga_config.currency_id_1],
			precisions: vec![],
			mint_fee: Default::default(),
			swap_fee: Default::default(),
			redeem_fee: Default::default(),
			total_supply: Default::default(),
			a: Default::default(),
			a_block: Default::default(),
			future_a: Default::default(),
			future_a_block: Default::default(),
			balances: Default::default(),
			fee_recipient: BOB,
			account_id: BOB,
			yield_recipient: BOB,
			precision: Default::default(),
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
		who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		i: PoolTokenIndex,
		j: PoolTokenIndex,
		dx: Self::Balance,
		min_dy: Self::Balance,
		_asset_length: u32,
	) -> sp_std::result::Result<(Self::Balance, Self::Balance), DispatchError> {
		if let Some(taiga_config) = TaigaConfig::get() {
			let (supply_currency_id, target_currency_id, swap_exchange_rate) = match (i, j) {
				(0, 1) => (
					taiga_config.currency_id_0,
					taiga_config.currency_id_1,
					taiga_config.exchange_rate_1_for_0,
				),
				(1, 0) => (
					taiga_config.currency_id_1,
					taiga_config.currency_id_0,
					taiga_config.exchange_rate_1_for_0.reciprocal().unwrap(),
				),
				_ => return Err(Error::<Runtime>::CannotSwap.into()),
			};
			let actual_target = swap_exchange_rate.saturating_mul_int(dx);
			ensure!(actual_target >= min_dy, Error::<Runtime>::CannotSwap);

			Tokens::withdraw(supply_currency_id, who, dx)?;
			Tokens::deposit(target_currency_id, who, actual_target)?;

			Ok((dx, actual_target))
		} else {
			Err(Error::<Runtime>::CannotSwap.into())
		}
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
		input_asset: Self::AssetId,
		output_asset: Self::AssetId,
		input_amount: Self::Balance,
	) -> Option<(StableAssetPoolId, PoolTokenIndex, PoolTokenIndex, Self::Balance)> {
		TaigaConfig::get().and_then(|taiga_config| {
			if input_asset == taiga_config.currency_id_0 && output_asset == taiga_config.currency_id_1 {
				Some((
					0,
					0,
					1,
					taiga_config.exchange_rate_1_for_0.saturating_mul_int(input_amount),
				))
			} else if output_asset == taiga_config.currency_id_0 && input_asset == taiga_config.currency_id_1 {
				Some((
					0,
					1,
					0,
					taiga_config
						.exchange_rate_1_for_0
						.reciprocal()
						.unwrap()
						.saturating_mul_int(input_amount),
				))
			} else {
				None
			}
		})
	}

	fn get_swap_output_amount(
		_pool_id: StableAssetPoolId,
		input_index: PoolTokenIndex,
		output_index: PoolTokenIndex,
		dx_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		TaigaConfig::get().and_then(|taiga_config| {
			let input_to_output_rate = match (input_index, output_index) {
				(0, 1) => taiga_config.exchange_rate_1_for_0,
				(1, 0) => taiga_config.exchange_rate_1_for_0.reciprocal().unwrap(),
				_ => return None,
			};
			let target_amount = input_to_output_rate.saturating_mul_int(dx_bal);

			Some(SwapResult {
				dx: dx_bal,
				dy: target_amount,
				..Default::default()
			})
		})
	}

	fn get_swap_input_amount(
		_pool_id: StableAssetPoolId,
		input_index: PoolTokenIndex,
		output_index: PoolTokenIndex,
		dy_bal: Self::Balance,
	) -> Option<SwapResult<Self::Balance>> {
		TaigaConfig::get().and_then(|taiga_config| {
			let output_to_input_rate = match (input_index, output_index) {
				(0, 1) => taiga_config.exchange_rate_1_for_0.reciprocal().unwrap(),
				(1, 0) => taiga_config.exchange_rate_1_for_0,
				_ => return None,
			};
			let supply_amount = output_to_input_rate.saturating_mul_int(dy_bal);

			Some(SwapResult {
				dx: supply_amount,
				dy: dy_bal,
				..Default::default()
			})
		})
	}

	fn xcm_mint(
		_who: &Self::AccountId,
		_target_pool_id: StableAssetPoolId,
		_amounts: Vec<Self::Balance>,
		_min_mint_amount: Self::Balance,
		_source_pool_id: StableAssetPoolId,
	) -> DispatchResult {
		unimplemented!()
	}

	fn xcm_redeem_single(
		_who: &Self::AccountId,
		_pool_id: StableAssetPoolId,
		_amount: Self::Balance,
		_i: PoolTokenIndex,
		_min_redeem_amount: Self::Balance,
		_asset_length: u32,
		_source_pool_id: StableAssetPoolId,
	) -> DispatchResult {
		unimplemented!()
	}
}

pub fn set_taiga_swap(currency_id_0: CurrencyId, currency_id_1: CurrencyId, exchange_rate_1_for_0: ExchangeRate) {
	TaigaConfig::set(Some(TaigaSwapStatus {
		currency_id_0,
		currency_id_1,
		stable_asset_id: STABLE_ASSET,
		exchange_rate_1_for_0,
	}));
}

pub fn set_dex_swap_joint_list(joints: Vec<Vec<CurrencyId>>) {
	DexSwapJointList::set(joints);
}

pub fn inject_liquidity(
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	// set balance
	Tokens::deposit(currency_id_a, &BOB, max_amount_a)?;
	Tokens::deposit(currency_id_b, &BOB, max_amount_b)?;

	let _ = Dex::enable_trading_pair(RawOrigin::Signed(BOB.clone()).into(), currency_id_a, currency_id_b);
	Dex::add_liquidity(
		RawOrigin::Signed(BOB).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;

	Ok(())
}

parameter_types! {
	pub static DexSwapJointList: Vec<Vec<CurrencyId>> = vec![];
	pub static TaigaConfig: Option<TaigaSwapStatus> = None;
}

impl Config for Runtime {
	type DEX = Dex;
	type StableAsset = MockStableAsset;
	type GovernanceOrigin = EnsureSignedBy<Admin, AccountId>;
	type DexSwapJointList = DexSwapJointList;
	type SwapPathLimit = ConstU32<3>;
	type WeightInfo = ();
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
		AggregatedDex: aggregated_dex::{Pallet, Call, Storage},
		Dex: module_dex::{Pallet, Call, Storage, Config<T>, Event<T>},
		Tokens: orml_tokens::{Pallet, Storage, Event<T>, Config<T>},
	}
);

pub struct ExtBuilder {
	balances: Vec<(AccountId, CurrencyId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			balances: vec![
				(ALICE, DOT, 100_000_000_000),
				(BOB, AUSD, 1_000_000_000_000_000_000),
				(BOB, DOT, 1_000_000_000_000_000_000),
				(BOB, LDOT, 1_000_000_000_000_000_000),
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

		t.into()
	}
}
