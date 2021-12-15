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

use crate::setup::*;

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
					Origin::signed(AccountId::from(ALICE)),
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
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				10_000 * dollar(RELAY_CHAIN_CURRENCY),
				10_000_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			let add_liquidity_event = Event::Dex(module_dex::Event::AddLiquidity(
				AccountId::from(ALICE),
				USD_CURRENCY,
				10_000_000 * dollar(USD_CURRENCY),
				RELAY_CHAIN_CURRENCY,
				10_000 * dollar(RELAY_CHAIN_CURRENCY),
				20_000_000 * dollar(USD_CURRENCY),
			));
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
				Origin::signed(AccountId::from(BOB)),
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
					Origin::signed(AccountId::from(BOB)),
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
				Origin::signed(AccountId::from(BOB)),
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
				Origin::signed(AccountId::from(BOB)),
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

			// CurrencyId::DexShare(Token, LiquidCroadloan)
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
				USD_CURRENCY,
				CurrencyId::LiquidCroadloan(1),
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(LiquidCroadloan, Token)
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
				CurrencyId::LiquidCroadloan(2),
				USD_CURRENCY,
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(Token, ForeignAsset)
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
				USD_CURRENCY,
				CurrencyId::ForeignAsset(1),
				10,
				100,
				100,
				1000,
				0,
			));

			// CurrencyId::DexShare(ForeignAsset, Token)
			assert_ok!(Dex::list_provisioning(
				Origin::root(),
				CurrencyId::ForeignAsset(2),
				USD_CURRENCY,
				10,
				100,
				100,
				1000,
				0,
			));
		});
}
