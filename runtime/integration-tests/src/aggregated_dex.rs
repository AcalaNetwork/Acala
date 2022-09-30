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

use crate::setup::*;
use module_aggregated_dex::{DexSwap, SwapPath};
use module_support::{Swap, SwapLimit};
use primitives::currency::AssetMetadata;
use primitives::CurrencyId::LiquidCrowdloan;
use sp_std::collections::btree_map::BTreeMap;
use std::sync::{Arc, Mutex};

pub fn enable_stable_asset(currencies: Vec<CurrencyId>, amounts: Vec<u128>, minter: Option<AccountId>) {
	let pool_asset = CurrencyId::StableAssetPoolToken(0);
	let precisions = currencies.iter().map(|_| 1u128).collect::<Vec<_>>();
	assert_ok!(StableAsset::create_pool(
		Origin::root(),
		pool_asset,
		currencies, // assets
		precisions,
		0,                        // mint fee
		25_000_000u128,           // swap fee
		30_000_000u128,           // redeem fee
		3000,                     // initialA
		AccountId::from(BOB),     // fee recipient
		AccountId::from(CHARLIE), // yield recipient
		1_000_000_000_000u128,    // precision
	));

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

	assert_ok!(StableAsset::mint(
		Origin::signed(minter.unwrap_or(AccountId::from(ALICE))),
		0,
		amounts,
		0u128
	));
}

fn inject_liquidity(
	account_id: AccountId,
	currency_id_a: CurrencyId,
	currency_id_b: CurrencyId,
	max_amount_a: Balance,
	max_amount_b: Balance,
) -> Result<(), &'static str> {
	let _ = Dex::enable_trading_pair(Origin::root(), currency_id_a, currency_id_b);
	Dex::add_liquidity(
		Origin::signed(account_id),
		currency_id_a,
		currency_id_b,
		max_amount_a,
		max_amount_b,
		Default::default(),
		false,
	)?;
	Ok(())
}

#[test]
fn aggregated_dex_works() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1000_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				LIQUID_CURRENCY,
				1000_000_000 * dollar(LIQUID_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			let relay_amount = 7_810_966_981_981_661;
			let liquid_amount = 12_529_784_940_482_519;
			enable_stable_asset(
				vec![RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY],
				vec![relay_amount, liquid_amount],
				None,
			);
			assert_ok!(inject_liquidity(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				LIQUID_CURRENCY,
				865_557_657_840_895,
				7_223_448_012_928_381
			));

			// get_swap_amount of AcalaSwap + DexSwap
			let (_, output) = AcalaSwap::get_swap_amount(
				RELAY_CHAIN_CURRENCY,
				LIQUID_CURRENCY,
				SwapLimit::ExactSupply(1_000_000_000_000, 0),
			)
			.unwrap();
			let target = DexSwap::<Runtime>::get_swap_amount(
				LIQUID_CURRENCY,
				RELAY_CHAIN_CURRENCY,
				SwapLimit::ExactSupply(output, 0),
			)
			.unwrap();

			// get_swap_amount of aggregated swap with specified path.
			let aggregated = vec![
				SwapPath::Taiga(0, 0, 1),
				SwapPath::Dex(vec![LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY]),
			];
			assert_ok!(AggregatedDex::update_aggregated_swap_paths(
				Origin::root(),
				vec![((RELAY_CHAIN_CURRENCY, RELAY_CHAIN_CURRENCY), Some(aggregated))]
			));
			let target1 = AcalaSwap::get_swap_amount(
				RELAY_CHAIN_CURRENCY,
				RELAY_CHAIN_CURRENCY,
				SwapLimit::ExactSupply(1_000_000_000_000, 0),
			)
			.unwrap();
			assert_eq!(target.1, target1.1);

			// swap of AcalaSwap + DexSwap
			let (_, output1) = AcalaSwap::swap(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				LIQUID_CURRENCY,
				SwapLimit::ExactSupply(1_000_000_000_000, 0),
			)
			.unwrap();
			let target1 = DexSwap::<Runtime>::swap(
				&AccountId::from(ALICE),
				LIQUID_CURRENCY,
				RELAY_CHAIN_CURRENCY,
				SwapLimit::ExactSupply(output, 0),
			)
			.unwrap();
			assert_eq!(output, output1);
			assert_eq!(target, target1);
			assert_eq!(output, 10_059_807_103_250);

			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			assert_eq!(target, (output, 1_200_144_864_851));
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(target, (output, 1_202_549_032_278));

			let output2 = AcalaSwap::swap(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				RELAY_CHAIN_CURRENCY,
				SwapLimit::ExactSupply(1_000_000_000_000, 0),
			)
			.unwrap();
			#[cfg(any(feature = "with-karura-runtime", feature = "with-acala-runtime"))]
			assert_eq!(output2.1, 1196811102574);
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(output2.1, 1199205260813);
		});
}

#[test]
fn rebalance_swap_works() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1000_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				TreasuryAccount::get(),
				RELAY_CHAIN_CURRENCY,
				1000_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				LIQUID_CURRENCY,
				1000_000_000 * dollar(LIQUID_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			let relay_amount = 7_810_966_981_981_661;
			let liquid_amount = 12_529_784_940_482_519;
			enable_stable_asset(
				vec![RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY],
				vec![relay_amount, liquid_amount],
				None,
			);
			assert_ok!(inject_liquidity(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				LIQUID_CURRENCY,
				865_557_657_840_895,
				7_223_448_012_928_381
			));

			let force_path = vec![
				SwapPath::Taiga(0, 0, 1),
				SwapPath::Dex(vec![LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY]),
			];
			assert_noop!(
				AcalaSwap::swap(
					&AccountId::from(ALICE),
					RELAY_CHAIN_CURRENCY,
					RELAY_CHAIN_CURRENCY,
					SwapLimit::ExactSupply(1_000_000_000_000, 0)
				),
				module_aggregated_dex::Error::<Runtime>::CannotSwap
			);

			assert_noop!(
				AggregatedDex::force_rebalance_swap(Origin::root(), RELAY_CHAIN_CURRENCY, force_path.clone()),
				sp_runtime::traits::BadOrigin
			);
			assert_ok!(AggregatedDex::force_rebalance_swap(
				RawOrigin::None.into(),
				RELAY_CHAIN_CURRENCY,
				force_path.clone()
			));
			assert!(System::events().iter().all(|r| {
				!matches!(
					r.event,
					Event::Dex(module_dex::Event::Swap { .. })
						| Event::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped { .. })
				)
			}));

			assert_ok!(AggregatedDex::set_rebalance_swap_info(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000_000,
				1_000_000_000_001,
			));

			// AggregatedDex swap not worked because we're not setting AggregatedDex swap path.
			assert_noop!(
				AcalaSwap::swap(
					&AccountId::from(ALICE),
					RELAY_CHAIN_CURRENCY,
					RELAY_CHAIN_CURRENCY,
					SwapLimit::ExactSupply(1_000_000_000_000, 0)
				),
				module_aggregated_dex::Error::<Runtime>::CannotSwap
			);

			// Rebalance swap path only go into effect for offchain worker.
			assert_ok!(AggregatedDex::force_rebalance_swap(
				RawOrigin::None.into(),
				RELAY_CHAIN_CURRENCY,
				force_path.clone()
			));
			assert!(System::events().iter().any(|r| {
				matches!(
					r.event,
					Event::Dex(module_dex::Event::Swap { .. })
						| Event::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped { .. })
				)
			}));
			#[cfg(any(feature = "with-acala-runtime", feature = "with-karura-runtime"))]
			let target_amount = 1_200_144_864_851;
			#[cfg(feature = "with-mandala-runtime")]
			let target_amount = 1_202_549_032_278;
			System::assert_last_event(Event::AggregatedDex(module_aggregated_dex::Event::RebalanceTrading {
				currency_id: RELAY_CHAIN_CURRENCY,
				supply_amount: 1_000_000_000_000,
				target_amount,
				swap_path: force_path,
			}));
		});
}

#[test]
fn produciton_rebalance_swap_path() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				NATIVE_CURRENCY,
				10_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				USD_CURRENCY,
				10_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				LIQUID_CURRENCY,
				10_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				LiquidCrowdloan(13),
				10_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				TreasuryAccount::get(),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			let relay_amount = 280_051_126_541_970;
			let liquid_amount = 1_205_900_671_542_331;

			enable_stable_asset(
				vec![RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY],
				vec![relay_amount, liquid_amount],
				None,
			);

			let minimal_balance = Balances::minimum_balance() / 10;
			assert_ok!(AssetRegistry::register_foreign_asset(
				Origin::root(),
				Box::new(MultiLocation::new(1, X1(Parachain(2001))).into()),
				Box::new(AssetMetadata {
					name: b"interlay BTC".to_vec(),
					symbol: b"iBTC".to_vec(),
					decimals: 8,
					minimal_balance
				})
			));
			assert_ok!(Currencies::deposit(
				CurrencyId::ForeignAsset(0),
				&AccountId::from(ALICE),
				1_000_000_000 * dollar(NATIVE_CURRENCY)
			));

			let trading_pair_values_map = dex_pools();

			let rebalance_paths = Arc::new(Mutex::new(BTreeMap::new()));
			#[cfg(any(feature = "with-acala-runtime", feature = "with-karura-runtime"))]
			let last_currency_id = USD_CURRENCY;
			#[cfg(feature = "with-mandala-runtime")]
			let last_currency_id = RELAY_CHAIN_CURRENCY;
			assert_eq!(
				Ok((true, Some(last_currency_id))),
				module_aggregated_dex::Pallet::<Runtime>::calculate_rebalance_paths(
					10,
					None,
					None,
					|currency_id, swap_path| {
						rebalance_paths.lock().unwrap().insert(currency_id, swap_path);
						()
					}
				)
			);

			let rebalance_paths = rebalance_paths.lock().unwrap();
			for (currency_id, swap_path) in rebalance_paths.iter() {
				let pairs = trading_pair_values_map.get(currency_id).unwrap();
				assert_eq!(swap_path, pairs);
			}
		});
}

fn dex_pools() -> BTreeMap<CurrencyId, Vec<SwapPath>> {
	assert_ok!(inject_liquidity(
		AccountId::from(ALICE),
		USD_CURRENCY,
		LIQUID_CURRENCY,
		3_760_432_120_659_928_418,
		1_276_237_324_617_138
	));
	assert_ok!(inject_liquidity(
		AccountId::from(ALICE),
		USD_CURRENCY,
		CurrencyId::ForeignAsset(0),
		1_691_588_973_592_287_955_985,
		274_627_356
	));
	assert_ok!(inject_liquidity(
		AccountId::from(ALICE),
		USD_CURRENCY,
		LiquidCrowdloan(13),
		8_849_894_746_344_149_013,
		229_803_827_696_334
	));
	assert_ok!(inject_liquidity(
		AccountId::from(ALICE),
		NATIVE_CURRENCY,
		USD_CURRENCY,
		338_237_683_713_930_724,
		9_083_201_752_524_763_780
	));
	assert_ok!(inject_liquidity(
		AccountId::from(ALICE),
		RELAY_CHAIN_CURRENCY,
		LiquidCrowdloan(13),
		1_727_685_431_872_727,
		4_401_401_349_880_089
	));

	let mut trading_pair_values_map: BTreeMap<CurrencyId, Vec<SwapPath>> = BTreeMap::new();
	let native_pairs = vec![
		SwapPath::Dex(vec![NATIVE_CURRENCY, USD_CURRENCY, RELAY_CHAIN_CURRENCY]),
		SwapPath::Dex(vec![RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY]),
	];
	let usd_pairs = vec![
		SwapPath::Dex(vec![USD_CURRENCY, RELAY_CHAIN_CURRENCY, LiquidCrowdloan(13)]),
		SwapPath::Dex(vec![LiquidCrowdloan(13), USD_CURRENCY]),
	];
	trading_pair_values_map.insert(NATIVE_CURRENCY, native_pairs);
	trading_pair_values_map.insert(USD_CURRENCY, usd_pairs);
	trading_pair_values_map
}
