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

use crate::{
	AcalaOracle, AccountId, AggregatedDex, AssetRegistry, Aura, Balance, Currencies, CurrencyId, Dex,
	ExistentialDeposits, GetLiquidCurrencyId, GetNativeCurrencyId, GetStableCurrencyId, GetStakingCurrencyId,
	MinimumCount, NativeTokenExistentialDeposit, OperatorMembershipAcala, Price, Runtime, RuntimeOrigin, StableAsset,
	System, Timestamp,
};

use frame_benchmarking::account;
use frame_support::{
	assert_ok,
	traits::{tokens::fungibles, Contains, OnInitialize},
};
use frame_system::RawOrigin;
use module_support::{AggregatedSwapPath, Erc20InfoMapping};
use orml_traits::{GetByKey, MultiCurrencyExtended};
pub use parity_scale_codec::Encode;
use primitives::currency::AssetMetadata;
use runtime_common::{TokenInfo, LCDOT};
use sp_consensus_aura::AURA_ENGINE_ID;
use sp_runtime::{
	traits::{SaturatedConversion, StaticLookup, UniqueSaturatedInto},
	Digest, DigestItem, DispatchResult, MultiAddress,
};
use sp_std::prelude::*;

pub type SwapPath = AggregatedSwapPath<CurrencyId>;

pub const NATIVE: CurrencyId = GetNativeCurrencyId::get();
pub const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
pub const LIQUID: CurrencyId = GetLiquidCurrencyId::get();
pub const STAKING: CurrencyId = GetStakingCurrencyId::get();
const SEED: u32 = 0;

pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
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

pub fn feed_price(prices: Vec<(CurrencyId, Price)>) -> DispatchResult {
	for i in 0..MinimumCount::get() {
		let oracle: AccountId = account("oracle", 0, i);
		if !OperatorMembershipAcala::contains(&oracle) {
			OperatorMembershipAcala::add_member(RawOrigin::Root.into(), MultiAddress::Id(oracle.clone()))
				.map_or_else(|e| Err(e.error), |_| Ok(()))?;
		}
		AcalaOracle::feed_values(RawOrigin::Signed(oracle).into(), prices.to_vec().try_into().unwrap())
			.map_or_else(|e| Err(e.error), |_| Ok(()))?;
	}

	Ok(())
}

pub fn set_block_number_timestamp(block_number: u32, timestamp: u64) {
	let slot = timestamp / Aura::slot_duration();
	let digest = Digest {
		logs: vec![DigestItem::PreRuntime(AURA_ENGINE_ID, slot.encode())],
	};
	System::initialize(&block_number, &Default::default(), &digest);
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

pub fn register_stable_asset() -> DispatchResult {
	let asset_metadata = AssetMetadata {
		name: b"Token Name".to_vec(),
		symbol: b"TN".to_vec(),
		decimals: 12,
		minimal_balance: 1,
	};
	AssetRegistry::register_stable_asset(RawOrigin::Root.into(), Box::new(asset_metadata.clone()))
}

pub fn create_stable_pools(assets: Vec<CurrencyId>, precisions: Vec<u128>, initial_a: u128) -> DispatchResult {
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	let mint_fee = 2u128;
	let swap_fee = 3u128;
	let redeem_fee = 5u128;
	let fee_recipient: AccountId = account("fee", 0, SEED);
	let yield_recipient: AccountId = account("yield", 1, SEED);

	register_stable_asset()?;
	StableAsset::create_pool(
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
	)?;

	Ok(())
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
	create_stable_pools(assets_1.clone(), vec![1, 1], 10000u128)?;
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

#[cfg(test)]
pub mod tests {
	use sp_runtime::BuildStorage;

	pub fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}
}
