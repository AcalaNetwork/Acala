//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{DexModule, ExtBuilder, Origin, Tokens, ALICE, AUSD, BOB, BTC, CAROL, DOT};

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
		assert_eq!(DexModule::calculate_swap_supply_amount(10000, 1, 1), 0);
	});
}

#[test]
fn add_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(ALICE), AUSD, 10000, 2000),
			"BaseCurrencyIdNotAllowed",
		);
		assert_eq!(DexModule::liquidity_pool(BTC), (0, 0));
		assert_eq!(DexModule::total_shares(BTC), 0);
		assert_eq!(DexModule::shares(BTC, ALICE), 0);
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(ALICE), BTC, 0, 10000000),
			"InvalidBalance",
		);
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(BTC, ALICE), 10000000);
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC, 1, 1000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10001, 10001000));
		assert_eq!(DexModule::total_shares(BTC), 10001000);
		assert_eq!(DexModule::shares(BTC, BOB), 1000);
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(BOB), BTC, 1, 999),
			"InvalidLiquidityIncrement",
		);
		assert_eq!(DexModule::liquidity_pool(BTC), (10001, 10001000));
		assert_eq!(DexModule::total_shares(BTC), 10001000);
		assert_eq!(DexModule::shares(BTC, BOB), 1000);
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC, 2, 1000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10002, 10002000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC, 1, 1001));
		assert_eq!(DexModule::liquidity_pool(BTC), (10003, 10003000));
	});
}

#[test]
fn withdraw_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(DexModule::liquidity_pool(BTC), (0, 0));
		assert_eq!(DexModule::total_shares(BTC), 0);
		assert_eq!(DexModule::shares(BTC, ALICE), 0);
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(BTC, ALICE), 10000000);
		assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC, 10000));
		assert_eq!(DexModule::liquidity_pool(BTC), (9990, 9990000));
		assert_eq!(DexModule::total_shares(BTC), 9990000);
		assert_eq!(DexModule::shares(BTC, ALICE), 9990000);
		assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC, 100));
		assert_eq!(DexModule::total_shares(BTC), 9989900);
		assert_eq!(DexModule::shares(BTC, ALICE), 9989900);
	});
}

#[test]
fn swap_other_to_base_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, BTC, 10000));
		assert_eq!(Tokens::balance(BTC, CAROL), 10000);
		assert_eq!(Tokens::balance(AUSD, CAROL), 0);
		assert_noop!(
			DexModule::swap_other_to_base(CAROL, BTC, 10001, 0),
			Error::TokenNotEnough,
		);
		assert_noop!(
			DexModule::swap_other_to_base(CAROL, BTC, 10000, 5000000),
			Error::InacceptablePrice,
		);
		assert_ok!(DexModule::swap_other_to_base(CAROL, BTC, 10000, 4950000));
		assert_eq!(Tokens::balance(BTC, CAROL), 0);
		assert_eq!(Tokens::balance(AUSD, CAROL), 4950000);
		assert_eq!(DexModule::liquidity_pool(BTC), (20000, 5050000));
	});
}

#[test]
fn swap_base_to_other_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, AUSD, 10000));
		assert_eq!(Tokens::balance(BTC, CAROL), 0);
		assert_eq!(Tokens::balance(AUSD, CAROL), 10000);
		assert_noop!(
			DexModule::swap_base_to_other(CAROL, BTC, 10001, 0),
			Error::TokenNotEnough,
		);
		assert_noop!(
			DexModule::swap_base_to_other(CAROL, BTC, 10000, 5000),
			Error::InacceptablePrice,
		);
		assert_ok!(DexModule::swap_base_to_other(CAROL, BTC, 10000, 4950));
		assert_eq!(Tokens::balance(BTC, CAROL), 4950);
		assert_eq!(Tokens::balance(AUSD, CAROL), 0);
		assert_eq!(DexModule::liquidity_pool(BTC), (5050, 20000));
	});
}

#[test]
fn swap_other_to_other_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 10000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 1000, 10000));
		assert_eq!(DexModule::liquidity_pool(BTC), (100, 10000));
		assert_eq!(DexModule::liquidity_pool(DOT), (1000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, DOT, 1000));
		assert_eq!(Tokens::balance(BTC, CAROL), 0);
		assert_eq!(Tokens::balance(DOT, CAROL), 1000);
		assert_noop!(
			DexModule::swap_other_to_other(CAROL, DOT, 1001, BTC, 0),
			Error::TokenNotEnough,
		);
		assert_noop!(
			DexModule::swap_other_to_other(CAROL, DOT, 1000, BTC, 35),
			Error::InacceptablePrice,
		);
		assert_ok!(DexModule::swap_other_to_other(CAROL, DOT, 1000, BTC, 34));
		assert_eq!(Tokens::balance(BTC, CAROL), 34);
		assert_eq!(Tokens::balance(DOT, CAROL), 0);
		assert_eq!(DexModule::liquidity_pool(BTC), (66, 14950));
		assert_eq!(DexModule::liquidity_pool(DOT), (2000, 5050));
	});
}

#[test]
fn swap_currency_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 10000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 1000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, BTC, 100));
		assert_noop!(
			DexModule::swap_currency(Origin::signed(CAROL), (BTC, 10000), (BTC, 1000)),
			"CanNotSwapItself"
		);
		assert_noop!(
			DexModule::swap_currency(Origin::signed(CAROL), (BTC, 101), (DOT, 1000)),
			"TokenNotEnough"
		);
		assert_ok!(DexModule::swap_currency(
			Origin::signed(CAROL),
			(BTC, 100),
			(AUSD, 4950)
		));
		assert_ok!(DexModule::swap_currency(Origin::signed(CAROL), (AUSD, 4950), (BTC, 90)));
		assert_ok!(DexModule::swap_currency(Origin::signed(CAROL), (BTC, 90), (DOT, 300)));
	});
}

#[test]
fn exchange_currency_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 10000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 1000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, BTC, 100));
		assert_noop!(
			DexModule::exchange_currency(CAROL, (BTC, 10000), (BTC, 1000)),
			Error::CanNotSwapItself
		);
		assert_noop!(
			DexModule::exchange_currency(CAROL, (BTC, 101), (DOT, 1000)),
			Error::TokenNotEnough
		);
		assert_ok!(DexModule::exchange_currency(CAROL, (BTC, 100), (AUSD, 4950)));
		assert_ok!(DexModule::exchange_currency(CAROL, (AUSD, 4950), (BTC, 90)));
		assert_ok!(DexModule::exchange_currency(CAROL, (BTC, 90), (DOT, 300)));
	});
}

#[test]
fn get_supply_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000));
		let supply_amount = DexModule::get_supply_amount(BTC, AUSD, 4950);
		assert_ok!(DexModule::exchange_currency(BOB, (BTC, supply_amount), (AUSD, 4950)));
	});
}
