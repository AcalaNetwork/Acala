//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{DexModule, ExtBuilder, Origin, System, TestEvent, Tokens, ALICE, AUSD, BOB, BTC, DOT};

#[test]
fn calculate_swap_target_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(DexModule::calculate_swap_target_amount(10000, 10000, 10000), 4950);
		// when target pool is 1
		assert_eq!(DexModule::calculate_swap_target_amount(10000, 1, 10000), 0);
		// when supply is too big
		assert_eq!(DexModule::calculate_swap_target_amount(100, 100, 9901), 0);
		// when target amount is too small to no fees
		assert_eq!(DexModule::calculate_swap_target_amount(100, 100, 9900), 99);
	});
}

#[test]
fn calculate_swap_supply_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(DexModule::calculate_swap_supply_amount(10000, 10000, 4950) <= 10000);
	});
}

#[test]
fn inject_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			DexModule::inject_liquidity(Origin::signed(ALICE), (AUSD, 10000), 2000),
			"BaseCurrencyIdNotAllowed",
		);
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (0, 0));
		assert_eq!(DexModule::total_shares(BTC), 0);
		assert_eq!(DexModule::shares(ALICE, BTC), 0);
		assert_noop!(
			DexModule::inject_liquidity(Origin::signed(ALICE), (BTC, 0), 10000000),
			"InvalidBalance",
		);
		assert_ok!(DexModule::inject_liquidity(
			Origin::signed(ALICE),
			(BTC, 10000),
			10000000
		),);
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(ALICE, BTC), 10000000);
		assert_ok!(DexModule::inject_liquidity(Origin::signed(BOB), (BTC, 1), 1000),);
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10001, 10001000));
		assert_eq!(DexModule::total_shares(BTC), 10001000);
		assert_eq!(DexModule::shares(BOB, BTC), 1000);
		assert_noop!(
			DexModule::inject_liquidity(Origin::signed(BOB), (BTC, 1), 999),
			"InvalidInject",
		);
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10001, 10001000));
		assert_eq!(DexModule::total_shares(BTC), 10001000);
		assert_eq!(DexModule::shares(BOB, BTC), 1000);
		assert_ok!(DexModule::inject_liquidity(Origin::signed(BOB), (BTC, 2), 1000));
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10002, 10002000));
		assert_ok!(DexModule::inject_liquidity(Origin::signed(BOB), (BTC, 1), 1001));
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10003, 10003000));
	});
}

#[test]
fn extract_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (0, 0));
		assert_eq!(DexModule::total_shares(BTC), 0);
		assert_eq!(DexModule::shares(ALICE, BTC), 0);
		assert_ok!(DexModule::inject_liquidity(
			Origin::signed(ALICE),
			(BTC, 10000),
			10000000
		),);
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(ALICE, BTC), 10000000);
		assert_ok!(DexModule::extract_liquidity(Origin::signed(ALICE), BTC, 10000));
		assert_eq!(DexModule::liquidity_pool(BTC, AUSD), (9990, 9990000));
		assert_eq!(DexModule::total_shares(BTC), 9990000);
		assert_eq!(DexModule::shares(ALICE, BTC), 9990000);
		assert_ok!(DexModule::extract_liquidity(Origin::signed(ALICE), BTC, 100));
		assert_eq!(DexModule::total_shares(BTC), 9989900);
		assert_eq!(DexModule::shares(ALICE, BTC), 9989900);
	});
}
