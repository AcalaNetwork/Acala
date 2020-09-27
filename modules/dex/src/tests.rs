//! Unit tests for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	DexModule, ExtBuilder, Origin, Runtime, System, TestEvent, Tokens, ACA, ALICE, AUSD, BOB, BTC, BTC_AUSD_LP, CAROL,
	DOT, DOT_AUSD_LP, LDOT,
};

#[test]
fn target_and_supply_amount_calculation() {
	ExtBuilder::default().build().execute_with(|| {
		// target pool is drain
		assert_eq!(
			DexModule::calculate_swap_target_amount(
				1_000_000_000_000_000_000,
				0,
				1_000_000_000_000_000_000,
				Rate::zero()
			),
			0
		);

		// supply pool is drain
		assert_eq!(
			DexModule::calculate_swap_target_amount(
				0,
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				Rate::zero()
			),
			0
		);

		// supply amount is zero
		assert_eq!(
			DexModule::calculate_swap_target_amount(
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				0,
				Rate::zero()
			),
			0
		);

		// fee rate >= 100%
		assert_eq!(
			DexModule::calculate_swap_target_amount(
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				Rate::one()
			),
			0
		);

		// target pool <= target amount
		assert_eq!(
			DexModule::calculate_swap_supply_amount(
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				Rate::zero()
			),
			0
		);
		assert_eq!(
			DexModule::calculate_swap_supply_amount(
				0,
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				Rate::zero()
			),
			0
		);

		// fee rate >= 100%
		assert_eq!(
			DexModule::calculate_swap_supply_amount(
				1_000_000_000_000_000_000,
				1_000_000_000_000_000_000,
				1_000_000_000_000,
				Rate::one()
			),
			0
		);

		let supply_pool = 1_000_000_000_000_000_000_000_000;
		let target_pool = 1_000_000_000_000_000_000_000_000;
		let fee_rate = Rate::saturating_from_rational(1, 100);
		let supply_amount = 1_000_000_000_000_000_000;
		let target_amount = DexModule::calculate_swap_target_amount(supply_pool, target_pool, supply_amount, fee_rate);
		let supply_amount_at_least =
			DexModule::calculate_swap_supply_amount(supply_pool, target_pool, target_amount, fee_rate);
		assert!(supply_amount_at_least >= supply_amount);

		let supply_pool = 1_000_000;
		let target_pool = 1_000_000_000_000_000_000_000_000;
		let fee_rate = Rate::saturating_from_rational(1, 100);
		let supply_amount = 1_000_000_000_000_000_000;
		let target_amount = DexModule::calculate_swap_target_amount(supply_pool, target_pool, supply_amount, fee_rate);
		let supply_amount_at_least =
			DexModule::calculate_swap_supply_amount(supply_pool, target_pool, target_amount, fee_rate);
		assert!(supply_amount_at_least >= supply_amount);

		let supply_pool = 195_703_422_673_811_993_405_238u128;
		let target_pool = 8_303_589_956_323_875_342_979u128;
		let fee_rate = Rate::saturating_from_rational(1, 1000); // 0.1%
		let target_amount = 1_000_000_000_000_000u128;
		let supply_amount_at_least =
			DexModule::calculate_swap_supply_amount(supply_pool, target_pool, target_amount, fee_rate);
		let actual_target_amount =
			DexModule::calculate_swap_target_amount(supply_pool, target_pool, supply_amount_at_least, fee_rate);
		assert!(actual_target_amount >= target_amount);
	});
}

#[test]
fn make_sure_get_supply_amount_needed_can_affort_target() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			500000000000,
			100000000000000000
		));
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(BOB),
			DOT_AUSD_LP,
			80000000000,
			4000000000000000
		));

		let target_amount_btc_ausd = 90000000000000;
		let surply_amount_btc_ausd = DexModule::get_supply_amount_needed(BTC, AUSD, target_amount_btc_ausd);
		assert!(DexModule::get_target_amount_available(BTC, AUSD, surply_amount_btc_ausd) >= target_amount_btc_ausd);

		let target_amount_ausd_dot = 8000000000000;
		let surply_amount_ausd_dot = DexModule::get_supply_amount_needed(BTC, AUSD, target_amount_ausd_dot);
		assert!(DexModule::get_target_amount_available(BTC, AUSD, surply_amount_ausd_dot) >= target_amount_ausd_dot);

		let target_amount_btc_dot = 60000000000;
		let surply_amount_btc_dot = DexModule::get_supply_amount_needed(BTC, AUSD, target_amount_btc_dot);
		assert!(DexModule::get_target_amount_available(BTC, AUSD, surply_amount_btc_dot) >= target_amount_btc_dot);
	});
}

#[test]
fn add_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(ALICE), AUSD, 10000, 2000),
			Error::<Runtime>::CurrencyIdNotAllowed,
		);
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000,
			10000000
		));

		let add_liquidity_event = TestEvent::dex(RawEvent::AddLiquidity(ALICE, BTC_AUSD_LP, 10000, 10000000, 10000000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == add_liquidity_event));

		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 10000000);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &ALICE), 10000000);
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC_AUSD_LP, 1, 1000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10001, 10001000));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 10001000);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &BOB), 1000);
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(BOB), BTC_AUSD_LP, 1, 999),
			Error::<Runtime>::InvalidLiquidityIncrement,
		);
		assert_eq!(DexModule::liquidity_pool(BTC), (10001, 10001000));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 10001000);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &BOB), 1000);
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC_AUSD_LP, 2, 1000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10002, 10002000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC_AUSD_LP, 1, 1001));
		assert_eq!(DexModule::liquidity_pool(BTC), (10003, 10003000));
	});
}

#[test]
fn withdraw_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(DexModule::liquidity_pool(BTC), (0, 0));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 0);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &ALICE), 0);
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000,
			10000000
		));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 10000000);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &ALICE), 10000000);
		assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC_AUSD_LP, 10000));

		let withdraw_liquidity_event =
			TestEvent::dex(RawEvent::WithdrawLiquidity(ALICE, BTC_AUSD_LP, 10, 10000, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == withdraw_liquidity_event));

		assert_eq!(DexModule::liquidity_pool(BTC), (9990, 9990000));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 9990000);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &ALICE), 9990000);
		assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC_AUSD_LP, 100));
		assert_eq!(Tokens::total_issuance(BTC_AUSD_LP), 9989900);
		assert_eq!(Tokens::free_balance(BTC_AUSD_LP, &ALICE), 9989900);
	});
}

#[test]
fn swap_other_to_base_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000,
			10000000
		));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, BTC, 10000));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 10000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 0);
		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), BTC, 10001, AUSD, 0).is_ok(),
			false
		);
		assert_noop!(
			DexModule::swap_currency(Origin::signed(CAROL), BTC, 10000, AUSD, 5000000),
			Error::<Runtime>::UnacceptablePrice,
		);

		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), BTC, 10000, AUSD, 4950000).is_ok(),
			true
		);
		let swap_event = TestEvent::dex(RawEvent::Swap(CAROL, BTC, 10000, AUSD, 4950000));
		assert!(System::events().iter().any(|record| record.event == swap_event));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 0);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 4950000);
		assert_eq!(DexModule::liquidity_pool(BTC), (20000, 5050000));
	});
}

#[test]
fn swap_base_to_other_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000,
			10000
		));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, AUSD, 10000));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 0);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 10000);
		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), AUSD, 10001, BTC, 0).is_ok(),
			false
		);
		assert_noop!(
			DexModule::swap_currency(Origin::signed(CAROL), AUSD, 10000, BTC, 5000),
			Error::<Runtime>::UnacceptablePrice,
		);

		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), AUSD, 10000, BTC, 4950).is_ok(),
			true
		);
		let swap_event = TestEvent::dex(RawEvent::Swap(CAROL, AUSD, 10000, BTC, 4950));
		assert!(System::events().iter().any(|record| record.event == swap_event));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 4950);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 0);
		assert_eq!(DexModule::liquidity_pool(BTC), (5050, 20000));
	});
}

#[test]
fn swap_other_to_other_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC_AUSD_LP, 100, 10000));
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			DOT_AUSD_LP,
			1000,
			10000
		));
		assert_eq!(DexModule::liquidity_pool(BTC), (100, 10000));
		assert_eq!(DexModule::liquidity_pool(DOT), (1000, 10000));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, DOT, 1000));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 0);
		assert_eq!(Tokens::free_balance(DOT, &CAROL), 1000);
		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), DOT, 1001, BTC, 0).is_ok(),
			false
		);
		assert_noop!(
			DexModule::swap_currency(Origin::signed(CAROL), DOT, 1000, BTC, 34),
			Error::<Runtime>::UnacceptablePrice,
		);

		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), DOT, 1000, BTC, 33).is_ok(),
			true
		);
		let swap_event = TestEvent::dex(RawEvent::Swap(CAROL, DOT, 1000, BTC, 33));
		assert!(System::events().iter().any(|record| record.event == swap_event));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 33);
		assert_eq!(Tokens::free_balance(DOT, &CAROL), 0);
		assert_eq!(DexModule::liquidity_pool(BTC), (67, 14950));
		assert_eq!(DexModule::liquidity_pool(DOT), (2000, 5050));
	});
}

#[test]
fn do_exchange_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC_AUSD_LP, 100, 10000));
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			DOT_AUSD_LP,
			1000,
			10000
		));
		assert_ok!(Tokens::transfer(Origin::signed(BOB), CAROL, BTC, 100));
		assert_noop!(
			DexModule::do_exchange(&CAROL, AUSD, 10000, LDOT, 1000),
			Error::<Runtime>::CurrencyIdNotAllowed,
		);
		assert_noop!(
			DexModule::do_exchange(&CAROL, BTC, 10000, BTC, 1000),
			Error::<Runtime>::CurrencyIdNotAllowed,
		);
		assert_noop!(
			DexModule::do_exchange(&CAROL, BTC, 100, DOT, 2000),
			Error::<Runtime>::UnacceptablePrice,
		);
		assert_ok!(DexModule::do_exchange(&CAROL, BTC, 100, AUSD, 4950));
		assert_ok!(DexModule::do_exchange(&CAROL, AUSD, 4950, BTC, 90));
		assert_ok!(DexModule::do_exchange(&CAROL, BTC, 90, DOT, 300));
	});
}

#[test]
fn get_supply_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000,
			10000
		));
		let supply_amount = DexModule::get_supply_amount(BTC, AUSD, 4950);
		assert_eq!(
			DexModule::exchange_currency(BOB, BTC, supply_amount, AUSD, 4950).is_ok(),
			true
		);
	});
}

#[test]
fn get_exchange_slippage_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC_AUSD_LP, 100, 1000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT_AUSD_LP, 200, 2000));
		assert_eq!(DexModule::get_exchange_slippage(BTC, BTC, 100), None);
		assert_eq!(DexModule::get_exchange_slippage(ACA, AUSD, 100), Some(Ratio::one()));
		assert_eq!(DexModule::get_exchange_slippage(BTC, AUSD, 0), Some(Ratio::zero()));
		assert_eq!(
			DexModule::get_exchange_slippage(BTC, AUSD, 10),
			Some(Ratio::saturating_from_rational(10, 110))
		);
		assert_eq!(
			DexModule::get_exchange_slippage(AUSD, BTC, 100),
			Some(Ratio::saturating_from_rational(100, 1100))
		);
		assert_eq!(
			DexModule::get_exchange_slippage(BTC, DOT, 100),
			Some(Ratio::saturating_from_rational(3, 5))
		);
	});
}
