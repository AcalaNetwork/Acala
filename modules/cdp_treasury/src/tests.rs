//! Unit tests for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{CdpTreasuryModule, Currencies, DexModule, ExtBuilder, Origin, Runtime, ALICE, AUSD, BOB, BTC};
use sp_runtime::traits::{BadOrigin, OnFinalize};

#[test]
fn set_collateral_auction_maximum_size_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CdpTreasuryModule::collateral_auction_maximum_size(BTC), 0);
		assert_noop!(
			CdpTreasuryModule::set_collateral_auction_maximum_size(Origin::signed(5), BTC, 200),
			BadOrigin
		);
		assert_ok!(CdpTreasuryModule::set_collateral_auction_maximum_size(
			Origin::signed(1),
			BTC,
			200
		));
		assert_ok!(CdpTreasuryModule::set_collateral_auction_maximum_size(
			Origin::ROOT,
			BTC,
			200
		));
		assert_eq!(CdpTreasuryModule::collateral_auction_maximum_size(BTC), 200);
	});
}

#[test]
fn set_debit_and_surplus_handle_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CdpTreasuryModule::set_debit_and_surplus_handle_params(
				Origin::signed(5),
				Some(100),
				Some(1000),
				Some(200),
				Some(100),
			),
			BadOrigin
		);
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::signed(1),
			Some(100),
			Some(1000),
			Some(200),
			Some(100),
		));
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(100),
			Some(1000),
			Some(200),
			Some(100),
		));
		assert_eq!(CdpTreasuryModule::surplus_auction_fixed_size(), 100);
		assert_eq!(CdpTreasuryModule::surplus_buffer_size(), 1000);
		assert_eq!(CdpTreasuryModule::initial_amount_per_debit_auction(), 200);
		assert_eq!(CdpTreasuryModule::debit_auction_fixed_size(), 100);
	});
}

#[test]
fn create_surplus_auction_on_finailize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(100),
			Some(1000),
			None,
			None,
		));
		CdpTreasuryModule::on_system_surplus(1099);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1099);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1099);
		CdpTreasuryModule::on_system_surplus(102);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1201);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1001);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1001);
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(0),
			None,
			None,
			None,
		));
		CdpTreasuryModule::on_system_surplus(99);
		CdpTreasuryModule::on_finalize(3);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1100);
	});
}

#[test]
fn create_debit_auction_on_finailize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			None,
			None,
			Some(200),
			Some(100),
		));
		CdpTreasuryModule::on_system_debit(99);
		assert_eq!(CdpTreasuryModule::debit_pool(), 99);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(CdpTreasuryModule::debit_pool(), 99);
		CdpTreasuryModule::on_system_debit(2);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(CdpTreasuryModule::debit_pool(), 1);
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			None,
			None,
			Some(0),
			None,
		));
		CdpTreasuryModule::on_system_debit(99);
		CdpTreasuryModule::on_finalize(3);
		assert_eq!(CdpTreasuryModule::debit_pool(), 100);
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			None,
			None,
			Some(200),
			Some(0),
		));
		CdpTreasuryModule::on_finalize(4);
		assert_eq!(CdpTreasuryModule::debit_pool(), 100);
	});
}

#[test]
fn on_system_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 1000);
	});
}

#[test]
fn on_system_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		CdpTreasuryModule::on_system_surplus(1000);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
	});
}

#[test]
fn offset_debit_and_surplus_on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_surplus(1000);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(300);
		assert_eq!(CdpTreasuryModule::debit_pool(), 300);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 700);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 700);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(800);
		assert_eq!(CdpTreasuryModule::debit_pool(), 800);
		CdpTreasuryModule::on_finalize(3);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 100);
	});
}

#[test]
fn deposit_backed_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(CdpTreasuryModule::deposit_backed_debit(&ALICE, 1000));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 2000);
	});
}

#[test]
fn withdraw_backed_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(CdpTreasuryModule::withdraw_backed_debit(&ALICE, 1000));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 0);
		assert_noop!(
			CdpTreasuryModule::withdraw_backed_debit(&ALICE, 1000),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);
	});
}

#[test]
fn emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(100),
			Some(1000),
			None,
			None,
		));
		CdpTreasuryModule::on_system_surplus(2000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 2000);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CdpTreasuryModule::is_shutdown(), false);
		CdpTreasuryModule::emergency_shutdown();
		assert_eq!(CdpTreasuryModule::is_shutdown(), true);
		CdpTreasuryModule::on_system_surplus(1000);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 2000);
	});
}

#[test]
fn deposit_system_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(Currencies::balance(BTC, &CdpTreasuryModule::account_id()), 0);
		CdpTreasuryModule::deposit_system_collateral(BTC, 100);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(Currencies::balance(BTC, &CdpTreasuryModule::account_id()), 100);
	});
}

#[test]
fn transfer_system_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpTreasuryModule::deposit_system_collateral(BTC, 500);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 500);
		assert_eq!(Currencies::balance(BTC, &CdpTreasuryModule::account_id()), 500);
		assert_noop!(
			CdpTreasuryModule::transfer_system_collateral(BTC, &BOB, 501),
			Error::<Runtime>::CollateralNotEnough,
		);
		assert_ok!(CdpTreasuryModule::transfer_system_collateral(BTC, &BOB, 400));
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(Currencies::balance(BTC, &CdpTreasuryModule::account_id()), 100);
		assert_eq!(Currencies::balance(BTC, &BOB), 400);
	});
}

#[test]
fn get_total_collaterals_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpTreasuryModule::deposit_system_collateral(BTC, 500);
		assert_eq!(CdpTreasuryModule::get_total_collaterals(BTC), 500);
	});
}

#[test]
fn get_surplus_pool_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpTreasuryModule::on_system_surplus(1000);
		assert_eq!(CdpTreasuryModule::get_surplus_pool(), 1000);
	});
}

#[test]
fn get_stable_currency_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CdpTreasuryModule::get_stable_currency_ratio(100),
			Ratio::from_rational(100, Currencies::total_issuance(AUSD))
		);
	});
}

#[test]
fn swap_collateral_to_stable_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 1000));
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		CdpTreasuryModule::deposit_system_collateral(BTC, 100);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 100);
		CdpTreasuryModule::swap_collateral_to_stable(BTC, 100, 500);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 500);
	});
}

#[test]
fn create_collateral_auctions_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpTreasuryModule::set_collateral_auction_maximum_size(
			Origin::ROOT,
			BTC,
			100
		));
		CdpTreasuryModule::deposit_system_collateral(BTC, 1000);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 1000);
		CdpTreasuryModule::create_collateral_auctions(BTC, 700, 2100, ALICE);
		assert_eq!(CdpTreasuryModule::total_collaterals(BTC), 300);
	});
}
