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

//! Common runtime benchmarking code.

use crate::{
	AcalaOracle, AccountId, AggregatedDex, AssetRegistry, Auction, AuctionId, AuctionManager, AuctionTimeToClose, Aura,
	Balance, CdpTreasury, Currencies, CurrencyId, Dex, DexOracle, ExistentialDeposits, GetLiquidCurrencyId,
	GetNativeCurrencyId, GetStableCurrencyId, GetStakingCurrencyId, MinimumCount, Moment,
	NativeTokenExistentialDeposit, OperatorMembershipAcala, Price, RawOrigin, Runtime, RuntimeOrigin, StableAsset,
	System, Timestamp, TradingPair, ACA, DOT, LCDOT, LDOT,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use frame_benchmarking::account;
use frame_support::assert_ok;
use frame_support::traits::fungibles;
use frame_support::traits::Contains;
use frame_support::traits::OnInitialize;
use frame_system::pallet_prelude::BlockNumberFor;
use module_aggregated_dex::SwapPath;
use module_support::Erc20InfoMapping;
use module_support::{AuctionManager as AuctionManagerTrait, CDPTreasury};
use orml_traits::GetByKey;
use orml_traits::MultiCurrencyExtended;
use primitives::currency::AssetMetadata;
use primitives::AuthoritysOriginId;
use runtime_common::TokenInfo;
use sp_runtime::MultiAddress;
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto},
	FixedPointNumber, FixedU128, SaturatedConversion,
};
use sp_std::vec;

/// Helper struct for benchmarking.
pub struct BenchmarkHelper<T>(sp_std::marker::PhantomData<T>);

/// Instance helper struct for benchmarking.
pub struct BenchmarkInstanceHelper<T, I>(sp_std::marker::PhantomData<(T, I)>);

impl<T, I> orml_oracle::BenchmarkHelper<T::OracleKey, T::OracleValue, T::MaxFeedValues>
	for BenchmarkInstanceHelper<T, I>
where
	T: orml_oracle::Config<I, OracleKey = CurrencyId, OracleValue = Price>,
{
	fn get_currency_id_value_pairs() -> sp_runtime::BoundedVec<(T::OracleKey, T::OracleValue), T::MaxFeedValues> {
		sp_runtime::BoundedVec::try_from(vec![
			(STAKING, FixedU128::saturating_from_rational(1, 1)),
			(LIQUID, FixedU128::saturating_from_rational(2, 1)),
			(STABLECOIN, FixedU128::saturating_from_rational(3, 1)),
		])
		.unwrap()
	}
}

impl<T> module_dex_oracle::BenchmarkHelper<CurrencyId, Moment> for BenchmarkHelper<T>
where
	T: module_dex_oracle::Config,
{
	fn setup_on_initialize(n: u32, u: u32) {
		let caller: AccountId = account("caller", 0, 0);

		let trading_pair_list = vec![
			TradingPair::from_currency_ids(NATIVE, STABLECOIN).unwrap(),
			TradingPair::from_currency_ids(NATIVE, STAKING).unwrap(),
			TradingPair::from_currency_ids(STAKING, STABLECOIN).unwrap(),
		];

		for i in 0..n {
			let trading_pair = trading_pair_list[i as usize];
			assert_ok!(inject_liquidity(
				caller.clone(),
				trading_pair.first(),
				trading_pair.second(),
				dollar(trading_pair.first()) * 100,
				dollar(trading_pair.second()) * 1000,
				false,
			));
			assert_ok!(DexOracle::enable_average_price(
				RawOrigin::Root.into(),
				trading_pair.first(),
				trading_pair.second(),
				240000
			));
		}
		for j in 0..u.min(n) {
			let update_pair = trading_pair_list[j as usize];
			assert_ok!(DexOracle::update_average_price_interval(
				RawOrigin::Root.into(),
				update_pair.first(),
				update_pair.second(),
				24000
			));
		}
		set_block_number_timestamp(1, 24000);
	}

	fn setup_inject_liquidity() -> Option<(CurrencyId, CurrencyId, Moment)> {
		let caller: AccountId = account("caller", 0, 0);

		assert_ok!(inject_liquidity(
			caller.clone(),
			NATIVE,
			STABLECOIN,
			dollar(NATIVE) * 100,
			dollar(STABLECOIN) * 1000,
			false,
		));

		Some((NATIVE, STABLECOIN, 24000))
	}
}

impl<T> module_prices::BenchmarkHelper<CurrencyId> for BenchmarkHelper<T>
where
	T: module_prices::Config,
{
	fn setup_feed_price() -> Option<CurrencyId> {
		feed_price(vec![(STAKING, dollar(STAKING).into())]);
		Some(STAKING)
	}
}

impl<T> orml_tokens::BenchmarkHelper<T::CurrencyId, T::Balance> for BenchmarkHelper<T>
where
	T: orml_tokens::Config<CurrencyId = CurrencyId, Balance = Balance>,
{
	fn get_currency_id_and_amount() -> Option<(T::CurrencyId, T::Balance)> {
		Some((STAKING, dollar(STAKING)))
	}
}

impl<T> orml_vesting::BenchmarkHelper<T::AccountId, <T as pallet_balances::Config>::Balance> for BenchmarkHelper<T>
where
	T: frame_system::Config<AccountId = AccountId> + pallet_balances::Config<Balance = Balance> + orml_vesting::Config,
{
	fn get_vesting_account_and_amount() -> Option<(T::AccountId, <T as pallet_balances::Config>::Balance)> {
		Some((get_vesting_account(), dollar(NATIVE)))
	}
}

impl<T> orml_auction::BenchmarkHelper<BlockNumberFor<T>, T::AccountId, T::Balance> for BenchmarkHelper<T>
where
	T: orml_auction::Config<AccountId = AccountId, Balance = Balance>,
{
	fn setup_bid() -> Option<(T::AccountId, T::Balance)> {
		let bidder: AccountId = account("bidder", 0, 0);
		let previous_bidder: AccountId = account("previous_bidder", 0, 0);
		let funder: AccountId = account("funder", 0, 0);
		let collateral_amount: Balance = 100 * dollar(STAKING);
		let target_amount: Balance = 10_000 * dollar(STABLECOIN);
		let previous_bid_price: Balance = 5_000u128 * dollar(STABLECOIN);
		let bid_price: Balance = 10_000u128 * dollar(STABLECOIN);
		let auction_id: AuctionId = 0;

		set_balance(STAKING, &funder, collateral_amount);
		set_balance(STABLECOIN, &bidder, bid_price);
		set_balance(STABLECOIN, &previous_bidder, previous_bid_price);
		assert_ok!(<CdpTreasury as CDPTreasury<_>>::deposit_collateral(
			&funder,
			STAKING,
			collateral_amount
		));
		assert_ok!(AuctionManager::new_collateral_auction(
			&funder,
			STAKING,
			collateral_amount,
			target_amount
		));
		assert_ok!(Auction::bid(
			RawOrigin::Signed(previous_bidder).into(),
			auction_id,
			previous_bid_price
		));

		Some((bidder, bid_price))
	}

	fn setup_on_finalize(rand: u32) -> Option<BlockNumberFor<T>> {
		let bidder = account("bidder", 0, 0);
		let funder = account("funder", 0, 0);
		let collateral_amount = 100 * dollar(STAKING);
		let target_amount = 10_000 * dollar(STABLECOIN);
		let bid_price = 5_000u128 * dollar(STABLECOIN);

		System::set_block_number(1);
		for auction_id in 0..rand {
			set_balance(STAKING, &funder, collateral_amount);
			assert_ok!(<CdpTreasury as CDPTreasury<_>>::deposit_collateral(
				&funder,
				STAKING,
				collateral_amount
			));
			assert_ok!(AuctionManager::new_collateral_auction(
				&funder,
				STAKING,
				collateral_amount,
				target_amount
			));
			set_balance(STABLECOIN, &bidder, bid_price);
			assert_ok!(Auction::bid(
				RawOrigin::Signed(bidder.clone()).into(),
				auction_id,
				bid_price
			));
		}
		Some((System::block_number() + AuctionTimeToClose::get()).into())
	}
}

impl<T> orml_authority::BenchmarkHelper<T::AsOriginId> for BenchmarkHelper<T>
where
	T: orml_authority::Config<AsOriginId = AuthoritysOriginId>,
{
	fn get_as_origin_id() -> Option<T::AsOriginId> {
		Some(AuthoritysOriginId::Root)
	}
}

pub const NATIVE: CurrencyId = GetNativeCurrencyId::get();
pub const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
pub const LIQUID: CurrencyId = GetLiquidCurrencyId::get();
pub const STAKING: CurrencyId = GetStakingCurrencyId::get();

fn get_vesting_account() -> super::AccountId {
	super::TreasuryPalletId::get().into_account_truncating()
}

fn get_benchmarking_collateral_currency_ids() -> Vec<CurrencyId> {
	vec![ACA, DOT, LDOT, CurrencyId::StableAssetPoolToken(0)]
}

pub fn register_native_asset(assets: Vec<CurrencyId>) {
	assets.iter().for_each(|asset| {
		let ed = if *asset == GetNativeCurrencyId::get() {
			NativeTokenExistentialDeposit::get()
		} else {
			ExistentialDeposits::get(&asset)
		};
		assert_ok!(AssetRegistry::register_native_asset(
			RuntimeOrigin::root(),
			*asset,
			Box::new(AssetMetadata {
				name: asset.name().unwrap().as_bytes().to_vec(),
				symbol: asset.symbol().unwrap().as_bytes().to_vec(),
				decimals: asset.decimals().unwrap(),
				minimal_balance: ed,
			})
		));
	});
}

pub fn set_balance(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

pub fn feed_price(prices: Vec<(CurrencyId, Price)>) {
	for i in 0..MinimumCount::get() {
		let oracle: AccountId = account("oracle", 0, i);
		if !OperatorMembershipAcala::contains(&oracle) {
			assert_ok!(OperatorMembershipAcala::add_member(
				RawOrigin::Root.into(),
				MultiAddress::Id(oracle.clone())
			));
		}
		assert_ok!(AcalaOracle::feed_values(
			RawOrigin::Signed(oracle).into(),
			prices.to_vec().try_into().unwrap()
		));
	}
}

pub fn set_block_number_timestamp(block_number: u32, timestamp: u64) {
	System::initialize(&block_number, &Default::default(), &Default::default());
	Aura::on_initialize(block_number);
	Timestamp::set_timestamp(timestamp);
}

#[allow(dead_code)]
pub fn set_balance_fungibles(currency_id: CurrencyId, who: &AccountId, balance: Balance) {
	assert_ok!(<orml_tokens::Pallet<Runtime> as fungibles::Mutate<AccountId>>::mint_into(currency_id, who, balance));
}

pub fn dollar(currency_id: CurrencyId) -> Balance {
	if matches!(currency_id, CurrencyId::Token(_))
		&& module_asset_registry::EvmErc20InfoMapping::<Runtime>::decimals(currency_id).is_none()
	{
		register_native_asset(vec![currency_id]);
	}
	if let Some(decimals) = module_asset_registry::EvmErc20InfoMapping::<Runtime>::decimals(currency_id) {
		10u128.saturating_pow(decimals.into())
	} else {
		panic!("{:?} not support decimals", currency_id);
	}
}

pub fn inject_liquidity(
	maker: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
	deposit: bool,
) -> Result<(), &'static str> {
	// set balance
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_a,
		&maker,
		max_amount_a.unique_saturated_into(),
	)?;
	<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id_b,
		&maker,
		max_amount_b.unique_saturated_into(),
	)?;

	let _ = Dex::enable_trading_pair(RawOrigin::Root.into(), currency_id_a, currency_id_b);

	Dex::add_liquidity(
		RawOrigin::Signed(maker.clone()).into(),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		deposit,
	)?;
	Ok(())
}

pub fn register_stable_asset() {
	let asset_metadata = AssetMetadata {
		name: b"Token Name".to_vec(),
		symbol: b"TN".to_vec(),
		decimals: 12,
		minimal_balance: 1,
	};
	assert_ok!(AssetRegistry::register_stable_asset(
		RawOrigin::Root.into(),
		Box::new(asset_metadata.clone())
	));
}

pub fn create_stable_pools(assets: Vec<CurrencyId>, precisions: Vec<u128>, initial_a: u128) {
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	let mint_fee = 2u128;
	let swap_fee = 3u128;
	let redeem_fee = 5u128;
	let fee_recipient: AccountId = account("fee", 0, 0);
	let yield_recipient: AccountId = account("yield", 1, 0);

	register_stable_asset();
	assert_ok!(StableAsset::create_pool(
		RawOrigin::Root.into(),
		pool_asset,
		assets,
		precisions,
		mint_fee,
		swap_fee,
		redeem_fee,
		initial_a,
		fee_recipient,
		yield_recipient,
		1000000000000000000u128,
	));
}

/// Initializes all pools used in AggregatedDex `Swap` for trading to stablecoin
pub fn initialize_swap_pools(maker: AccountId) -> Result<(), &'static str> {
	// Inject liquidity into all possible `AlternativeSwapPathJointList`
	inject_liquidity(
		maker.clone(),
		LIQUID,
		STABLECOIN,
		10_000 * dollar(LIQUID),
		10_000 * dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(
		maker.clone(),
		STAKING,
		LIQUID,
		10_000 * dollar(STAKING),
		10_000 * dollar(LIQUID),
		false,
	)?;

	// purposly inject too little liquidity to have failed path, still reads dexs to check for viable
	// swap paths
	inject_liquidity(
		maker.clone(),
		STAKING,
		STABLECOIN,
		10 * dollar(STAKING),
		10 * dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(
		maker.clone(),
		LCDOT,
		STABLECOIN,
		dollar(LCDOT),
		dollar(STABLECOIN),
		false,
	)?;
	inject_liquidity(maker.clone(), LCDOT, STAKING, dollar(LCDOT), dollar(STAKING), false)?;

	// Add and initialize stable pools, is manually added with changes to runtime
	let assets_1 = vec![STAKING, LIQUID];
	create_stable_pools(assets_1.clone(), vec![1, 1], 10000u128);
	for asset in assets_1 {
		<Currencies as MultiCurrencyExtended<_>>::update_balance(asset, &maker, 1_000_000_000_000_000)?;
	}
	StableAsset::mint(
		RawOrigin::Signed(maker.clone()).into(),
		0,
		vec![1_000_000_000_000, 1_000_000_000_000],
		0,
	)?;

	// Adds `AggregatedSwapPaths`, also mirrors runtimes state
	AggregatedDex::update_aggregated_swap_paths(
		RawOrigin::Root.into(),
		vec![
			(
				(STAKING, STABLECOIN),
				Some(vec![SwapPath::Taiga(0, 0, 1), SwapPath::Dex(vec![LIQUID, STABLECOIN])]),
			),
			(
				(LIQUID, STABLECOIN),
				Some(vec![SwapPath::Taiga(0, 1, 0), SwapPath::Dex(vec![STAKING, STABLECOIN])]),
			),
		],
	)?;

	Ok(())
}
