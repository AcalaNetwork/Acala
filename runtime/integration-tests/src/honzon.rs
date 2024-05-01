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
use frame_support::traits::fungible::Mutate;
use module_evm_accounts::EvmAddressMapping;
use module_support::{
	evm::{AddressMapping, LiquidationEvmBridge},
	InvokeContext,
};
use primitives::evm::EvmAddress;
use std::str::FromStr;

fn setup_default_collateral(currency_id: CurrencyId) {
	assert_ok!(CdpEngine::set_collateral_params(
		RuntimeOrigin::root(),
		currency_id,
		Change::NewValue(Some(Default::default())),
		Change::NoChange,
		Change::NoChange,
		Change::NoChange,
		Change::NewValue(10000),
	));
}

pub fn mock_liquidation_address_0() -> EvmAddress {
	EvmAddress::from_str("0xda548f126ece4d35e8ea3fc01f56e6d99e7afb38").unwrap()
}

pub fn mock_liquidation_address_1() -> EvmAddress {
	EvmAddress::from_str("0xa3716bf2d6a42cca05efe379fb7e9fec70739a1a").unwrap()
}

pub fn cdp_engine_pallet_account() -> AccountId {
	CDPEnginePalletId::get().into_account_truncating()
}

pub fn cdp_treasury_pallet_account() -> AccountId {
	CDPTreasuryPalletId::get().into_account_truncating()
}

pub fn account_id_to_address(who: &AccountId) -> EvmAddress {
	EvmAddressMapping::<Runtime>::get_evm_address(who).unwrap()
}

pub fn address_to_account_id(address: &EvmAddress) -> AccountId {
	EvmAddressMapping::<Runtime>::get_account_id(address)
}

pub fn repayment_evm_addr() -> EvmAddress {
	// EVM address of the CdpEngine Pallet account.
	account_id_to_address(&CDPEnginePalletId::get().into_account_truncating())
}

pub fn deploy_liquidation_contracts() {
	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/MockLiquidationContract.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	// Deposits some funds used to call the contracts.
	assert_ok!(Balances::mint_into(
		&cdp_engine_pallet_account(),
		1_000 * dollar(NATIVE_CURRENCY)
	));
	assert_ok!(Balances::mint_into(
		&address_to_account_id(&mock_liquidation_address_0()),
		1_000 * dollar(NATIVE_CURRENCY)
	));
	assert_ok!(Balances::mint_into(
		&address_to_account_id(&mock_liquidation_address_1()),
		1_000 * dollar(NATIVE_CURRENCY)
	));
	assert_ok!(EVM::create(
		RuntimeOrigin::signed(cdp_engine_pallet_account()),
		code.clone(),
		0,
		500_000,
		15_000,
		vec![]
	));

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
		from: repayment_evm_addr(),
		contract: mock_liquidation_address_0(),
		logs: vec![],
		used_gas: 460625,
		used_storage: 11887,
	}));

	assert_ok!(EVM::create(
		RuntimeOrigin::signed(cdp_engine_pallet_account()),
		code,
		0,
		500_000,
		15_000,
		vec![]
	));

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
		from: repayment_evm_addr(),
		contract: mock_liquidation_address_1(),
		logs: vec![],
		used_gas: 460625,
		used_storage: 11887,
	}));
}

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
			setup_default_collateral(RELAY_CHAIN_CURRENCY);
			setup_default_collateral(LIQUID_CURRENCY);
			setup_default_collateral(USD_CURRENCY);

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
					RuntimeOrigin::signed(AccountId::from(ALICE)),
					1_000_000 * dollar(USD_CURRENCY)
				),
				module_emergency_shutdown::Error::<Runtime>::CanNotRefund,
			);
			assert_ok!(EmergencyShutdown::emergency_shutdown(RuntimeOrigin::root()));
			assert_ok!(EmergencyShutdown::open_collateral_refund(RuntimeOrigin::root()));
			assert_ok!(EmergencyShutdown::refund_collaterals(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
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
fn can_liquidate_cdp_via_dex() {
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
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				1_000_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
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
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(400, 100))),
				Change::NoChange,
			));

			// If asset cannot be liquidated automatically with reasonable slippage, use Auction.
			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			let liquidate_alice_xbtc_cdp_event =
				RuntimeEvent::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
					collateral_type: RELAY_CHAIN_CURRENCY,
					owner: AccountId::from(ALICE),
					collateral_amount: 50 * dollar(RELAY_CHAIN_CURRENCY),
					bad_debt_value: 250_000 * dollar(USD_CURRENCY),
					target_amount: Rate::saturating_from_rational(20, 100)
						.saturating_mul_acc_int(250_000 * dollar(USD_CURRENCY)),
				});
			System::assert_has_event(liquidate_alice_xbtc_cdp_event);
			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				0
			);
			assert!(AuctionManager::collateral_auctions(0).is_some());
			assert_eq!(CdpTreasury::debit_pool(), 250_000 * dollar(USD_CURRENCY));

			// Prioritize liquidation by Dex
			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY
			));

			let liquidate_bob_xbtc_cdp_event = RuntimeEvent::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(BOB),
				collateral_amount: dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 5_000 * dollar(USD_CURRENCY),
				target_amount: Rate::saturating_from_rational(20, 100)
					.saturating_mul_acc_int(5_000 * dollar(USD_CURRENCY)),
			});

			System::assert_has_event(liquidate_bob_xbtc_cdp_event);

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
				RuntimeOrigin::root(),
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
					RuntimeOrigin::none(),
					RELAY_CHAIN_CURRENCY,
					MultiAddress::Id(AccountId::from(ALICE))
				)
				.is_ok(),
				false
			);
			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NoChange,
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 1))),
				Change::NoChange,
				Change::NoChange,
				Change::NoChange,
			));
			assert_ok!(CdpEngine::liquidate(
				RuntimeOrigin::none(),
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
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10_000 * dollar(USD_CURRENCY)),
			));

			let maybe_new_collateral_params = CdpEngine::collateral_params(RELAY_CHAIN_CURRENCY);
			assert!(maybe_new_collateral_params.is_some());
			let new_collateral_params = maybe_new_collateral_params.unwrap();

			assert_eq!(
				new_collateral_params.interest_rate_per_sec.map(|v| v.into_inner()),
				Some(Rate::saturating_from_rational(1, 100000))
			);
			assert_eq!(
				new_collateral_params.liquidation_ratio,
				Some(Ratio::saturating_from_rational(3, 2))
			);
			assert_eq!(
				new_collateral_params.liquidation_penalty.map(|v| v.into_inner()),
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

			let settle_cdp_in_debit_event = RuntimeEvent::CdpEngine(module_cdp_engine::Event::SettleCDPInDebit {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
			});
			System::assert_has_event(settle_cdp_in_debit_event);

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
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 10000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(1_000_000 * dollar(USD_CURRENCY)),
			));
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			// Honzon loans work
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
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
			assert_eq!(CdpTreasury::get_surplus_pool(), 270716741782);
			assert_eq!(CdpTreasury::get_debit_pool(), 0);
			// Honzon generated cdp treasury surplus can be transfered
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)), 0);
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				// about 1/10
				Some(Ratio::saturating_from_rational(
					100541433483565674 as i64,
					1000000000000000000 as i64
				))
			);
			// Cdp treasury cannot be reaped
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(CdpTreasury::account_id()),
				sp_runtime::MultiAddress::Id(AccountId::from(BOB)),
				USD_CURRENCY,
				CdpTreasury::get_surplus_pool() - 1
			));
			assert_eq!(
				Currencies::free_balance(USD_CURRENCY, &AccountId::from(BOB)),
				270716741781
			);
			assert_eq!(Currencies::free_balance(USD_CURRENCY, &CdpTreasury::account_id()), 1);
			run_to_block(3);
			// Debt exchange rate updates
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				// Around 1/10, increasing from last check
				Some(Ratio::saturating_from_rational(
					100662149583216144 as i64,
					1000000000000000000 as i64
				))
			);

			// Closing loan will add to treasury debit_pool
			assert_ok!(Honzon::close_loan_has_debit_by_dex(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				5 * dollar(RELAY_CHAIN_CURRENCY),
			));
			// Just over 50 dollar(USD_CURRENCY), due to interest on loan
			assert_eq!(CdpTreasury::get_debit_pool(), 50331074791608);
			assert_eq!(Loans::total_positions(RELAY_CHAIN_CURRENCY).debit, 0);
			run_to_block(4);
			// Debt exchange rate doesn't update due to no debit positions
			assert_eq!(
				CdpEngine::debit_exchange_rate(RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(
					100662149583216144 as i64,
					1000000000000000000 as i64
				))
			)
		});
}

#[test]
fn cdp_engine_minimum_collateral_amount_works() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(ALICE), USD_CURRENCY, 100 * dollar(USD_CURRENCY)),
			(AccountId::from(ALICE), NATIVE_CURRENCY, 100 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			set_oracle_price(vec![
				(NATIVE_CURRENCY, Price::saturating_from_rational(1, 1)),
				(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 1)),
			]);

			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				NATIVE_CURRENCY,
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 10000))),
				Change::NewValue(None),
				Change::NewValue(None),
				Change::NewValue(1_000_000 * dollar(NATIVE_CURRENCY)),
			));
			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Rate::saturating_from_rational(1, 10000))),
				Change::NewValue(None),
				Change::NewValue(None),
				Change::NewValue(1_000_000 * dollar(RELAY_CHAIN_CURRENCY)),
			));

			let native_minimum_collateral_amount = NativeTokenExistentialDeposit::get() * 100;
			let relaychain_minimum_collateral_amount = ExistentialDeposits::get(&RELAY_CHAIN_CURRENCY) * 100;

			#[cfg(feature = "with-acala-runtime")]
			{
				assert_eq!(native_minimum_collateral_amount, 10 * dollar(ACA));
				assert_eq!(relaychain_minimum_collateral_amount, dollar(DOT));
			}

			#[cfg(feature = "with-mandala-runtime")]
			{
				assert_eq!(native_minimum_collateral_amount, 10 * dollar(ACA));
				assert_eq!(relaychain_minimum_collateral_amount, cent(DOT));
			}

			#[cfg(feature = "with-karura-runtime")]
			{
				assert_eq!(native_minimum_collateral_amount, 10 * dollar(KAR));
				assert_eq!(relaychain_minimum_collateral_amount, cent(KSM));
			}

			// Native add shares cannot be below the minimum share
			assert_noop!(
				CdpEngine::adjust_position(
					&AccountId::from(ALICE),
					NATIVE_CURRENCY,
					(NativeTokenExistentialDeposit::get() - 1) as i128,
					0i128,
				),
				orml_rewards::Error::<Runtime>::ShareBelowMinimal
			);

			// Native collateral cannot be below the minimum when debit is 0
			assert_noop!(
				CdpEngine::adjust_position(
					&AccountId::from(ALICE),
					NATIVE_CURRENCY,
					(native_minimum_collateral_amount - 1) as i128,
					0i128,
				),
				module_cdp_engine::Error::<Runtime>::CollateralAmountBelowMinimum
			);

			// Other token add shares cannot be below the minimum share
			assert_noop!(
				CdpEngine::adjust_position(
					&AccountId::from(ALICE),
					RELAY_CHAIN_CURRENCY,
					(ExistentialDeposits::get(&RELAY_CHAIN_CURRENCY) - 1) as i128,
					0i128,
				),
				orml_rewards::Error::<Runtime>::ShareBelowMinimal
			);

			// Other token collaterals cannot be below the minimum when debit is 0
			assert_noop!(
				CdpEngine::adjust_position(
					&AccountId::from(ALICE),
					RELAY_CHAIN_CURRENCY,
					(relaychain_minimum_collateral_amount - 1) as i128,
					0i128,
				),
				module_cdp_engine::Error::<Runtime>::CollateralAmountBelowMinimum
			);

			// Native collateral minimum not enforced when debit is non-zero
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				NATIVE_CURRENCY,
				(native_minimum_collateral_amount - 1) as i128,
				(MinimumDebitValue::get() * 10) as i128,
			));

			// Other token collateral minimum not enforced when debit is non-zero
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(relaychain_minimum_collateral_amount - 1) as i128,
				(MinimumDebitValue::get() * 10) as i128,
			));

			// Native collateral can be withdrawal in its entirety if debit is 0
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				NATIVE_CURRENCY,
				1i128 - (native_minimum_collateral_amount as i128),
				-((MinimumDebitValue::get() * 10) as i128),
			));

			// Other tokens collateral can be withdrawal in its entirety if debit is 0
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1i128 - (relaychain_minimum_collateral_amount as i128),
				-((MinimumDebitValue::get() * 10) as i128),
			));
		});
}

#[test]
fn can_deploy_liquidation_contract() {
	ExtBuilder::default().build().execute_with(|| {
		deploy_liquidation_contracts();
		assert_ok!(module_evm_bridge::LiquidationEvmBridge::<Runtime>::liquidate(
			InvokeContext {
				contract: mock_liquidation_address_0(),
				sender: repayment_evm_addr(),
				origin: repayment_evm_addr(),
			},
			RELAY_CHAIN_CURRENCY.erc20_address().unwrap(),
			repayment_evm_addr(),
			1,
			0,
		));
	});
}

#[test]
fn can_liquidate_cdp_via_intended_priority() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY, 1000 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				1_000_000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(USD_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			deploy_liquidation_contracts();
			assert_ok!(CdpEngine::register_liquidation_contract(
				RuntimeOrigin::root(),
				mock_liquidation_address_0()
			));
			assert_ok!(CdpEngine::register_liquidation_contract(
				RuntimeOrigin::root(),
				mock_liquidation_address_1()
			));
			assert_eq!(
				CdpEngine::liquidation_contracts(),
				vec![mock_liquidation_address_0(), mock_liquidation_address_1()]
			);

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 1))]);

			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				100 * dollar(USD_CURRENCY),
				0,
				false,
			));

			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))), // 2:1 collateral ratio
				Change::NewValue(Some(Rate::zero())),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(1_000_000 * dollar(USD_CURRENCY)),
			));

			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(2000 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				(1000 * dollar(USD_CURRENCY)) as i128,
			));

			// Set the price so the position is unsafe.
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 100))]);

			System::reset_events();
			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			//
			// If both dex and contract cannot liquidate, then go to auction.
			//
			System::assert_has_event(RuntimeEvent::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
				collateral_amount: 2000 * dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 100 * dollar(USD_CURRENCY),
				target_amount: 100 * dollar(USD_CURRENCY),
			}));

			System::assert_has_event(RuntimeEvent::AuctionManager(
				module_auction_manager::Event::NewCollateralAuction {
					auction_id: 0,
					collateral_type: RELAY_CHAIN_CURRENCY,
					collateral_amount: 2_000 * dollar(RELAY_CHAIN_CURRENCY),
					target_bid_price: 100 * dollar(USD_CURRENCY),
				},
			));

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 1))]);
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(2000 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				(1000 * dollar(USD_CURRENCY)) as i128,
			));

			// Give contracts enough funds for liquidation
			assert_ok!(Tokens::deposit(
				USD_CURRENCY,
				&address_to_account_id(&mock_liquidation_address_1()),
				1000 * dollar(USD_CURRENCY)
			));

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 100))]);

			//
			// When dex cannot liquidate, try to liquidate using EVM Contracts instead.
			//
			assert_eq!(Tokens::free_balance(USD_CURRENCY, &cdp_engine_pallet_account()), 0);
			System::reset_events();
			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			// Check liquidation happened successfully via contract
			assert_eq!(
				Tokens::free_balance(USD_CURRENCY, &cdp_engine_pallet_account()),
				100 * dollar(USD_CURRENCY)
			);
			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				0
			);
			System::assert_has_event(RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
				currency_id: USD_CURRENCY,
				from: address_to_account_id(&mock_liquidation_address_1()),
				to: cdp_engine_pallet_account(),
				amount: 100 * dollar(USD_CURRENCY),
			}));

			System::assert_has_event(RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
				currency_id: RELAY_CHAIN_CURRENCY,
				from: cdp_treasury_pallet_account(),
				to: address_to_account_id(&mock_liquidation_address_1()),
				amount: 2000 * dollar(RELAY_CHAIN_CURRENCY),
			}));

			System::assert_has_event(RuntimeEvent::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
				collateral_amount: 2000 * dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 100 * dollar(USD_CURRENCY),
				target_amount: 100 * dollar(USD_CURRENCY),
			}));

			//
			// When dex has enough liquidity, Liquidate using DEX as first priority
			//
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				1000 * dollar(RELAY_CHAIN_CURRENCY),
				1000 * dollar(USD_CURRENCY),
				0,
				false,
			));
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 1))]);
			assert_ok!(CdpEngine::adjust_position(
				&AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				(2000 * dollar(RELAY_CHAIN_CURRENCY)) as i128,
				(1000 * dollar(USD_CURRENCY)) as i128,
			));
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(1, 100))]);

			System::reset_events();
			assert_ok!(CdpEngine::liquidate_unsafe_cdp(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY
			));

			// Liquidation done by swapping using DEX
			#[cfg(feature = "with-mandala-runtime")]
			let liquidity_change = 1_101_101_101_102u128;
			#[cfg(feature = "with-karura-runtime")]
			let liquidity_change = 110_330_992_978_937u128;
			#[cfg(feature = "with-acala-runtime")]
			let liquidity_change = 1_103_309_929_790u128;
			System::assert_has_event(RuntimeEvent::Dex(module_dex::Event::Swap {
				trader: cdp_treasury_pallet_account(),
				path: vec![RELAY_CHAIN_CURRENCY, USD_CURRENCY],
				liquidity_changes: vec![liquidity_change, 100_000_000_000_000],
			}));

			// Remaining collaterals are returned to the user
			#[cfg(feature = "with-mandala-runtime")]
			let collateral_returned = 18_898_898_898_898u128;
			#[cfg(feature = "with-karura-runtime")]
			let collateral_returned = 1_889_669_007_021_063u128;
			#[cfg(feature = "with-acala-runtime")]
			let collateral_returned = 18_896_690_070_210u128;
			System::assert_has_event(RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
				currency_id: RELAY_CHAIN_CURRENCY,
				from: cdp_treasury_pallet_account(),
				to: AccountId::from(ALICE),
				amount: collateral_returned,
			}));

			System::assert_has_event(RuntimeEvent::CdpEngine(module_cdp_engine::Event::LiquidateUnsafeCDP {
				collateral_type: RELAY_CHAIN_CURRENCY,
				owner: AccountId::from(ALICE),
				collateral_amount: 2000 * dollar(RELAY_CHAIN_CURRENCY),
				bad_debt_value: 100 * dollar(USD_CURRENCY),
				target_amount: 100 * dollar(USD_CURRENCY),
			}));

			assert_eq!(Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit, 0);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				0
			);
		});
}
