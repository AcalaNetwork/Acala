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

use crate::setup::*;
use primitives::currency::AssetMetadata;
use sp_core::bounded::BoundedVec;

#[test]
fn test_dex_module() {
	ExtBuilder::default()
		.balances(vec![
			(
				// NetworkContractSource
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				USD_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY), (0, 0));
			assert_eq!(Currencies::total_issuance(LPTOKEN), 0);
			assert_eq!(Currencies::free_balance(LPTOKEN, &AccountId::from(ALICE)), 0);

			assert_noop!(
				Dex::add_liquidity(
					RuntimeOrigin::signed(AccountId::from(ALICE)),
					RELAY_CHAIN_CURRENCY,
					USD_CURRENCY,
					0,
					10_000_000 * dollar(USD_CURRENCY),
					0,
					false,
				),
				module_dex::Error::<Runtime>::InvalidLiquidityIncrement,
			);

			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				10_000 * dollar(RELAY_CHAIN_CURRENCY),
				10_000_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			let add_liquidity_event = RuntimeEvent::Dex(module_dex::Event::AddLiquidity {
				who: AccountId::from(ALICE),
				currency_0: USD_CURRENCY,
				pool_0: 10_000_000 * dollar(USD_CURRENCY),
				currency_1: RELAY_CHAIN_CURRENCY,
				pool_1: 10_000 * dollar(RELAY_CHAIN_CURRENCY),
				share_increment: 20_000_000 * dollar(USD_CURRENCY),
			});
			assert!(System::events()
				.iter()
				.any(|record| record.event == add_liquidity_event));

			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_000 * dollar(RELAY_CHAIN_CURRENCY), 10_000_000 * dollar(USD_CURRENCY))
			);
			assert_eq!(Currencies::total_issuance(LPTOKEN), 20_000_000 * dollar(USD_CURRENCY));
			assert_eq!(
				Currencies::free_balance(LPTOKEN, &AccountId::from(ALICE)),
				20_000_000 * dollar(USD_CURRENCY)
			);
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				1 * dollar(RELAY_CHAIN_CURRENCY),
				1_000 * dollar(USD_CURRENCY),
				0,
				false,
			));
			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_001 * dollar(RELAY_CHAIN_CURRENCY), 10_001_000 * dollar(USD_CURRENCY))
			);
			assert_eq!(Currencies::total_issuance(LPTOKEN), 20_002_000 * dollar(USD_CURRENCY));
			assert_eq!(
				Currencies::free_balance(LPTOKEN, &AccountId::from(BOB)),
				2000 * dollar(USD_CURRENCY)
			);
			assert_noop!(
				Dex::add_liquidity(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					RELAY_CHAIN_CURRENCY,
					USD_CURRENCY,
					1,
					999,
					0,
					false,
				),
				module_dex::Error::<Runtime>::InvalidLiquidityIncrement,
			);
			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_001 * dollar(RELAY_CHAIN_CURRENCY), 10_001_000 * dollar(USD_CURRENCY))
			);
			assert_eq!(Currencies::total_issuance(LPTOKEN), 20_002_000 * dollar(USD_CURRENCY));
			assert_eq!(
				Currencies::free_balance(LPTOKEN, &AccountId::from(BOB)),
				2_000 * dollar(USD_CURRENCY)
			);
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				2 * dollar(RELAY_CHAIN_CURRENCY),
				1_000 * dollar(USD_CURRENCY),
				0,
				false,
			));
			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_002 * dollar(RELAY_CHAIN_CURRENCY), 10_002_000 * dollar(USD_CURRENCY))
			);
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				1 * dollar(RELAY_CHAIN_CURRENCY),
				1_001 * dollar(USD_CURRENCY),
				0,
				false,
			));
			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_003 * dollar(RELAY_CHAIN_CURRENCY), 10_003_000 * dollar(USD_CURRENCY))
			);

			assert_eq!(Currencies::total_issuance(LPTOKEN), 20_005_999_999_999_999_995);
		});
}

#[test]
fn test_trading_pair() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				USD_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY), (0, 0));
			assert_eq!(Currencies::total_issuance(LPTOKEN), 0);
			assert_eq!(Currencies::free_balance(LPTOKEN, &AccountId::from(ALICE)), 0);

			// CurrencyId::DexShare(Token, LiquidCrowdloan)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				USD_CURRENCY,
				CurrencyId::LiquidCrowdloan(1),
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(LiquidCrowdloan, Token)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				CurrencyId::LiquidCrowdloan(2),
				USD_CURRENCY,
				10,
				100,
				100,
				1000,
				0,
			));

			assert_ok!(AssetRegistry::register_foreign_asset(
				RuntimeOrigin::root(),
				Box::new(
					Location::new(
						1,
						[
							Parachain(2002),
							Junction::from(BoundedVec::try_from(KAR.encode()).unwrap())
						]
					)
					.into()
				),
				Box::new(AssetMetadata {
					name: b"Sibling Token".to_vec(),
					symbol: b"ST".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			));

			// CurrencyId::DexShare(Token, ForeignAsset)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				USD_CURRENCY,
				CurrencyId::ForeignAsset(0),
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(ForeignAsset, Token)
			assert_ok!(Dex::list_provisioning(
				RuntimeOrigin::root(),
				CurrencyId::ForeignAsset(0),
				RELAY_CHAIN_CURRENCY,
				10,
				100,
				100,
				1000,
				0,
			));
		});
}
