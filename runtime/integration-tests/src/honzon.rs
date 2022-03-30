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

#[test]
fn emergency_shutdown_and_cdp_treasury() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 2_000_000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), USD_CURRENCY, 8_000_000 * dollar(USD_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				300_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				AccountId::from(BOB),
				LIQUID_CURRENCY,
				50_000_000 * dollar(LIQUID_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(CdpTreasury::deposit_collateral(
				&AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				200_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			));
			assert_ok!(CdpTreasury::deposit_collateral(
				&AccountId::from(BOB),
				LIQUID_CURRENCY,
				40_000_000 * dollar(LIQUID_CURRENCY)
			));
			assert_eq!(
				CdpTreasury::total_collaterals(RELAY_CHAIN_CURRENCY),
				200_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				CdpTreasury::total_collaterals(LIQUID_CURRENCY),
				40_000_000 * dollar(LIQUID_CURRENCY)
			);

			// Total liquidity to collaterize is calculated using Stable currency - USD
			assert_noop!(
				EmergencyShutdown::refund_collaterals(
					Origin::signed(AccountId::from(ALICE)),
					1_000_000 * dollar(USD_CURRENCY)
				),
				module_emergency_shutdown::Error::<Runtime>::CanNotRefund,
			);
			assert_ok!(EmergencyShutdown::emergency_shutdown(Origin::root()));
			assert_ok!(EmergencyShutdown::open_collateral_refund(Origin::root()));
			assert_ok!(EmergencyShutdown::refund_collaterals(
				Origin::signed(AccountId::from(ALICE)),
				1_000_000 * dollar(USD_CURRENCY)
			));

			assert_eq!(
				CdpTreasury::total_collaterals(RELAY_CHAIN_CURRENCY),
				180_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				CdpTreasury::total_collaterals(LIQUID_CURRENCY),
				36_000_000 * dollar(LIQUID_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE)),
				1_000_000 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
				20_000_000 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY, &AccountId::from(ALICE)),
				4_000_000 * dollar(LIQUID_CURRENCY)
			);
		});
}

#[test]
fn liquidate_cdp() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				51 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_001 * dollar(USD_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				102 * dollar(RELAY_CHAIN_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(10000, 1))]); // 10000 usd

			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				1_000_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(1_000_000 * dollar(USD_CURRENCY)),
			));

			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(50 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				(2_500_000 * dollar(USD_CURRENCY)) as i128,
			));

			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				dollar(RELAY_CHAIN_CURRENCY) as i128,
				(50_000 * dollar(USD_CURRENCY)) as i128,
			));

			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				2_500_000 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				50 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(BOB)).debit,
				50_000 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(BOB)).collateral,
				dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(CdpTreasury::debit_pool(), 0);
			assert_eq!(AuctionManager::collateral_auctions(0), None);

			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
			));

			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			let liquidate_alice_xbtc_cdp_event = Event::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
				collateral_amount: 50 * dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 250_000 * dollar(USD_CURRENCY),
				target_amount: Rate::saturating_from_rational(20, 100)
					.saturating_mul_acc_int(250_000 * dollar(USD_CURRENCY)),
			});
			assert!(System::events()
				.iter()
				.any(|record| record.event == liquidate_alice_xbtc_cdp_event));
			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				0
			);
			assert!(AuctionManager::collateral_auctions(0).is_some());
			assert_eq!(CdpTreasury::debit_pool(), 250_000 * dollar(USD_CURRENCY));

			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY
			));

			let liquidate_bob_xbtc_cdp_event = Event::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(BOB),
				collateral_amount: dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 5_000 * dollar(USD_CURRENCY),
				target_amount: Rate::saturating_from_rational(20, 100)
					.saturating_mul_acc_int(5_000 * dollar(USD_CURRENCY)),
			});

			assert!(System::events()
				.iter()
				.any(|record| record.event == liquidate_bob_xbtc_cdp_event));

			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(BOB)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(BOB)).collateral,
				0
			);
			assert_eq!(CdpTreasury::debit_pool(), 255_000 * dollar(USD_CURRENCY));
			assert!(CdpTreasury::surplus_pool() >= 5_000 * dollar(USD_CURRENCY));
		});
}

#[test]
fn test_honzon_module() {
	ExtBuilder::default()
		.balances(vec![(
			AccountId::from(ALICE),
			RELAY_CHAIN_CURRENCY,
			1_000 * dollar(RELAY_CHAIN_CURRENCY),
		)])
		.build()
		.execute_with(|| {
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 1))]);

			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10_000 * dollar(USD_CURRENCY)),
			));
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(100 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				(500 * dollar(USD_CURRENCY)) as i128
			));
			assert_eq!(
				Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
				900 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE)),
				50 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				500 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				100 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				CdpEngine::liquidate(
					Origin::none(),
					RELAY_CHAIN_CURRENCY,
					MultiAddress::Id(AccountId::from(ALICE))
				)
				.is_ok(),
				false
			);
			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
			));
			assert_ok!(CdpEngine::liquidate(
				Origin::none(),
				RELAY_CHAIN_CURRENCY,
				MultiAddress::Id(AccountId::from(ALICE))
			));

			assert_eq!(
				Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
				900 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE)),
				50 * dollar(USD_CURRENCY)
			);
			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				0
			);
		});
}

#[test]
fn test_cdp_engine_module() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 2_000 * dollar(USD_CURRENCY)),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				2_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10_000 * dollar(USD_CURRENCY)),
			));

			let new_collateral_params = CdpEngine::collateral_params(RELAY_CHAIN_CURRENCY);

			assert_eq!(
				new_collateral_params.interest_rate_per_sec,
				Some(Rate::saturating_from_rational(1, 100000))
			);
			assert_eq!(
				new_collateral_params.liquidation_ratio,
				Some(Ratio::saturating_from_rational(3, 2))
			);
			assert_eq!(
				new_collateral_params.liquidation_penalty,
				Some(Rate::saturating_from_rational(2, 10))
			);
			assert_eq!(
				new_collateral_params.required_collateral_ratio,
				Some(Ratio::saturating_from_rational(9, 5))
			);
			assert_eq!(
				new_collateral_params.maximum_total_debit_value,
				10_000 * dollar(USD_CURRENCY)
			);

			assert_eq!(
				CdpEngine::calculate_collateral_ratio(
					RELAY_CHAIN_CURRENCY,
					100 * dollar(RELAY_CHAIN_CURRENCY),
					50 * dollar(USD_CURRENCY),
					Price::saturating_from_rational(1 * dollar(USD_CURRENCY), dollar(RELAY_CHAIN_CURRENCY)),
				),
				Ratio::saturating_from_rational(100 * 10, 50)
			);

			assert_ok!(CdpEngine::check_debit_cap(
				RELAY_CHAIN_CURRENCY,
				99_999 * dollar(USD_CURRENCY)
			));
			assert_eq!(
				CdpEngine::check_debit_cap(RELAY_CHAIN_CURRENCY, 100_001 * dollar(USD_CURRENCY)).is_ok(),
				false
			);

			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(200 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				0
			));
			assert_eq!(
				Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
				1800 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				200 * dollar(RELAY_CHAIN_CURRENCY)
			);

			assert_noop!(
				CdpEngine::settle_cdp_has_debit(AccountId::from(ALICE), RELAY_CHAIN_CURRENCY),
				module_cdp_engine::Error::<Runtime>::NoDebitValue,
			);

			set_oracle_price(vec![
				(USD_CURRENCY, Price::saturating_from_rational(1, 1)),
				(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(3, 1)),
			]);

			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				0,
				(500 * dollar(USD_CURRENCY)) as i128
			));
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				500 * dollar(USD_CURRENCY)
			);
			assert_eq!(CdpTreasury::debit_pool(), 0);
			assert_eq!(CdpTreasury::total_collaterals(RELAY_CHAIN_CURRENCY), 0);
			assert_ok!(CdpEngine::settle_cdp_has_debit(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			let settle_cdp_in_debit_event = Event::CdpEngine(module_cdp_engine::Event::SettleCDPInDebit {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
			});
			assert!(System::events()
				.iter()
				.any(|record| record.event == settle_cdp_in_debit_event));

			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(CdpTreasury::debit_pool(), 50 * dollar(USD_CURRENCY));

			// DOT is 10 decimal places where as ksm is 12 decimals. Hence the difference in collaterals.
			#[cfg(feature = "with-mandala-runtime")]
			assert_eq!(CdpTreasury::total_collaterals(RELAY_CHAIN_CURRENCY), 166_666_666_666);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(CdpTreasury::total_collaterals(RELAY_CHAIN_CURRENCY), 16_666_666_666_666);
		});
}

// Honzon's surplus can be transfered and DebitExchangeRate updates accordingly
#[test]
fn cdp_treasury_handles_honzon_surplus_correctly() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 10_000 * dollar(USD_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(100, 1))]);
			assert_ok!(CdpEngine::set_collateral_params(
				Origin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 10000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(1_000_000 * dollar(USD_CURRENCY)),
			));
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			// Honzon loans work
			assert_ok!(Honzon::adjust_loan(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				50 * dollar(RELAY_CHAIN_CURRENCY) as i128,
				500 * dollar(USD_CURRENCY) as i128
			));
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				50 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				500 * dollar(USD_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(RELAY_CHAIN_CURRENCY, &AccountId::from(ALICE)),
				50 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE)),
				50 * dollar(USD_CURRENCY)
			);
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &CdpTreasury::account_id()), 0);
			assert_eq!(CdpTreasury::get_surplus_pool(), 0);
			assert_eq!(CdpTreasury::get_debit_pool(), 0);
			run_to_block(2);

			// Empty treasury recieves stablecoins into surplus pool from loan
			assert_eq!(CdpTreasury::get_surplus_pool(), 160248248179);
			assert_eq!(CdpTreasury::get_debit_pool(), 0);
			// Honzon generated cdp treasury surplus can be transfered
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)), 0);
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				// about 1/10
				Some(Ratio::saturating_from_rational(
					100320496496359801 as i64,
					1000000000000000000 as i64
				))
			);
			// Cdp treasury cannot be reaped
			assert_ok!(Currencies::transfer(
				Origin::signed(CdpTreasury::account_id()),
				sp_runtime::MultiAddress::Id(AccountId::from(BOB)),
				USD_CURRENCY,
				CdpTreasury::get_surplus_pool() - 1
			));
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)),
				160248248178
			);
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &CdpTreasury::account_id()), 1);
			run_to_block(3);
			// Debt exchange rate updates
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				// Around 1/10, increasing from last check
				Some(Ratio::saturating_from_rational(
					100330528546009436 as i64,
					1000000000000000000 as i64
				))
			);

			// Closing loan will add to treasury debit_pool
			assert_ok!(Honzon::close_loan_has_debit_by_dex(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				5 * dollar(RELAY_CHAIN_CURRENCY),
			));
			// Just over 50 dollar(USD_CURRENCY), due to interest on loan
			assert_eq!(CdpTreasury::get_debit_pool(), 50165264273004);
			assert_eq!(Loans::total_positions(RELAY_CHAIN_CURRENCY).debit, 0);
			run_to_block(4);
			// Debt exchange rate doesn't update due to no debit positions
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(
					100330528546009436 as i64,
					1000000000000000000 as i64
				))
			)
		});
}
