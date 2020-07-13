//! Unit tests for the dex module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::OnInitialize};
use mock::{
	DexModule, ExtBuilder, GetExchangeFee, Origin, Runtime, System, TestEvent, Tokens, ACA, ALICE, AUSD, BOB, BTC,
	CAROL, DOT, LDOT,
};
use sp_runtime::traits::BadOrigin;

#[test]
fn set_liquidity_incentive_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			DexModule::liquidity_incentive_rate(BTC),
			Rate::saturating_from_rational(1, 100)
		);
		assert_noop!(
			DexModule::set_liquidity_incentive_rate(Origin::signed(5), BTC, Rate::saturating_from_rational(5, 100)),
			BadOrigin
		);
		assert_ok!(DexModule::set_liquidity_incentive_rate(
			Origin::signed(1),
			BTC,
			Rate::saturating_from_rational(5, 100)
		));
		assert_eq!(
			DexModule::liquidity_incentive_rate(BTC),
			Rate::saturating_from_rational(5, 100)
		);
	});
}

#[test]
fn accumulate_interest_work() {
	ExtBuilder::default().build().execute_with(|| {
		LiquidityPool::insert(BTC, (100, 10000));
		assert_eq!(DexModule::total_interest(BTC), (0, 0));
		DexModule::accumulate_interest(BTC);
		assert_eq!(DexModule::total_interest(BTC), (100, 0));
	});
}

#[test]
fn claim_interest_work() {
	ExtBuilder::default().build().execute_with(|| {
		<Shares<Runtime>>::insert(BTC, ALICE, 2000);
		<TotalShares<Runtime>>::insert(BTC, 10000);
		TotalInterest::insert(BTC, (25000, 20000));
		<WithdrawnInterest<Runtime>>::insert(BTC, ALICE, 2000);
		assert_ok!(Tokens::deposit(AUSD, &DexModule::account_id(), 5000));
		let alice_former_balance = Tokens::free_balance(AUSD, &ALICE);
		assert_eq!(Tokens::free_balance(AUSD, &DexModule::account_id()), 5000);
		assert_ok!(DexModule::claim_interest(BTC, &ALICE));
		assert_eq!(DexModule::total_interest(BTC), (25000, 23000));
		assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 5000);
		assert_eq!(Tokens::free_balance(AUSD, &DexModule::account_id()), 2000);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), alice_former_balance + 3000);
	});
}

#[test]
fn withdraw_calculate_interest_work() {
	ExtBuilder::default().build().execute_with(|| {
		<Shares<Runtime>>::insert(BTC, ALICE, 2000);
		<TotalShares<Runtime>>::insert(BTC, 10000);
		TotalInterest::insert(BTC, (25000, 25000));
		<WithdrawnInterest<Runtime>>::insert(BTC, ALICE, 10000);
		assert_ok!(DexModule::withdraw_calculate_interest(BTC, &ALICE, 1000));
		assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 5000);
		assert_eq!(DexModule::total_interest(BTC), (22500, 20000));
	});
}

#[test]
fn deposit_calculate_interest_work() {
	ExtBuilder::default().build().execute_with(|| {
		<TotalShares<Runtime>>::insert(BTC, 5000);
		TotalInterest::insert(BTC, (10000, 2000));
		DexModule::deposit_calculate_interest(BTC, &ALICE, 4000);
		assert_eq!(DexModule::total_interest(BTC), (18000, 10000));
		assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 8000);
	});
}

#[test]
fn calculate_swap_target_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(DexModule::calculate_swap_target_amount(10000, 10000, 10000, GetExchangeFee::get()) <= 4950);
		// when target pool is 1
		assert_eq!(
			DexModule::calculate_swap_target_amount(10000, 1, 10000, GetExchangeFee::get()),
			0
		);
		// when supply is too big
		assert_eq!(
			DexModule::calculate_swap_target_amount(100, 100, 9901, GetExchangeFee::get()),
			0
		);
		// when target amount is too small to no fees
		assert_eq!(
			DexModule::calculate_swap_target_amount(100, 100, 9900, GetExchangeFee::get()),
			99
		);
	});
}

#[test]
fn calculate_swap_supply_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(DexModule::calculate_swap_supply_amount(10000, 10000, 4950, GetExchangeFee::get()) >= 10000);
		// when target amount is too big
		assert_eq!(
			DexModule::calculate_swap_supply_amount(10000, 10000, 10000, GetExchangeFee::get()),
			0
		);
		// when target amount is zero
		assert_eq!(
			DexModule::calculate_swap_supply_amount(10000, 10000, 0, GetExchangeFee::get()),
			0
		);
	});
}

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

		let supply_pool = 1_000_000_000_000_000_000_000_000;
		let target_pool = 1_000_000;
		let fee_rate = Rate::saturating_from_rational(1, 100);
		let supply_amount = 1_000_000_000_000_000_000;
		let target_amount = DexModule::calculate_swap_target_amount(supply_pool, target_pool, supply_amount, fee_rate);
		let supply_amount_at_least =
			DexModule::calculate_swap_supply_amount(supply_pool, target_pool, target_amount, fee_rate);
		assert!(supply_amount_at_least >= supply_amount);
	});
}

#[test]
fn make_sure_get_supply_amount_needed_can_affort_target() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(ALICE),
			BTC,
			500000000000,
			100000000000000000
		));
		assert_ok!(DexModule::add_liquidity(
			Origin::signed(BOB),
			DOT,
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
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));

		let add_liquidity_event = TestEvent::dex(RawEvent::AddLiquidity(ALICE, BTC, 10000, 10000000, 10000000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == add_liquidity_event));

		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(BTC, ALICE), 10000000);
		assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC, 1, 1000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10001, 10001000));
		assert_eq!(DexModule::total_shares(BTC), 10001000);
		assert_eq!(DexModule::shares(BTC, BOB), 1000);
		assert_noop!(
			DexModule::add_liquidity(Origin::signed(BOB), BTC, 1, 999),
			Error::<Runtime>::InvalidLiquidityIncrement,
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
fn add_liquidity_and_calculate_interest() {
	ExtBuilder::default()
		.set_balance(CAROL, AUSD, 1_000_000_000_000_000_000u128)
		.set_balance(CAROL, BTC, 1_000_000_000_000_000_000u128)
		.build()
		.execute_with(|| {
			assert_noop!(
				DexModule::add_liquidity(Origin::signed(ALICE), ACA, 10000, 2000),
				Error::<Runtime>::CurrencyIdNotAllowed,
			);
			assert_noop!(
				DexModule::add_liquidity(Origin::signed(ALICE), AUSD, 10000, 2000),
				Error::<Runtime>::CurrencyIdNotAllowed,
			);
			assert_eq!(DexModule::liquidity_pool(BTC), (0, 0));
			assert_eq!(DexModule::total_shares(BTC), 0);
			assert_eq!(DexModule::shares(BTC, ALICE), 0);
			assert_noop!(
				DexModule::add_liquidity(Origin::signed(ALICE), BTC, 0, 10000000),
				Error::<Runtime>::InvalidLiquidityIncrement,
			);

			// ALICE add_liquidity 8000
			assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 800, 8000));
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::total_shares(BTC), 8000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 0);
			assert_eq!(DexModule::total_interest(BTC), (0, 0));

			// BOB add_liquidity 2000
			assert_ok!(DexModule::add_liquidity(Origin::signed(BOB), BTC, 200, 2000));
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::total_shares(BTC), 10000);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::total_interest(BTC), (0, 0));

			// accumulate interest
			<DexModule as OnInitialize<u64>>::on_initialize(1);
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::total_shares(BTC), 10000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::total_interest(BTC), (100, 0));

			// CAROL add_liquidity 500
			assert_ok!(DexModule::add_liquidity(Origin::signed(CAROL), BTC, 500, 10000));
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 15000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (150, 50));

			// claim interest
			assert_ok!(DexModule::claim_interest(BTC, &ALICE));
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 15000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 79);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (150, 129));

			// accumulate interest
			<DexModule as OnInitialize<u64>>::on_initialize(1);
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 15000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 79);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (300, 129));

			// claim interest
			assert_ok!(DexModule::claim_interest(BTC, &ALICE));
			assert_eq!(DexModule::shares(BTC, ALICE), 8000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 15000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 159);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (300, 209));

			// ALICE withdraw liquidity 5000
			assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC, 5000));
			assert_eq!(DexModule::shares(BTC, ALICE), 3000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 10000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 60);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (201, 110));

			// accumulate interest
			<DexModule as OnInitialize<u64>>::on_initialize(1);
			assert_eq!(DexModule::shares(BTC, ALICE), 3000);
			assert_eq!(DexModule::shares(BTC, BOB), 2000);
			assert_eq!(DexModule::shares(BTC, CAROL), 5000);
			assert_eq!(DexModule::total_shares(BTC), 10000);
			assert_eq!(DexModule::withdrawn_interest(BTC, ALICE), 60);
			assert_eq!(DexModule::withdrawn_interest(BTC, BOB), 0);
			assert_eq!(DexModule::withdrawn_interest(BTC, CAROL), 50);
			assert_eq!(DexModule::total_interest(BTC), (301, 110));
		});
}

#[test]
fn withdraw_liquidity_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(DexModule::liquidity_pool(BTC), (0, 0));
		assert_eq!(DexModule::total_shares(BTC), 0);
		assert_eq!(DexModule::shares(BTC, ALICE), 0);
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));
		assert_eq!(DexModule::liquidity_pool(BTC), (10000, 10000000));
		assert_eq!(DexModule::total_shares(BTC), 10000000);
		assert_eq!(DexModule::shares(BTC, ALICE), 10000000);
		assert_ok!(DexModule::withdraw_liquidity(Origin::signed(ALICE), BTC, 10000));

		let withdraw_liquidity_event = TestEvent::dex(RawEvent::WithdrawLiquidity(ALICE, BTC, 10, 10000, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == withdraw_liquidity_event));

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
		System::set_block_number(1);
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000000));
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
			Error::<Runtime>::InacceptablePrice,
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
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000));
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
			Error::<Runtime>::InacceptablePrice,
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
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 10000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 1000, 10000));
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
			DexModule::swap_currency(Origin::signed(CAROL), DOT, 1000, BTC, 35),
			Error::<Runtime>::InacceptablePrice,
		);

		assert_eq!(
			DexModule::swap_currency(Origin::signed(CAROL), DOT, 1000, BTC, 34).is_ok(),
			true
		);
		let swap_event = TestEvent::dex(RawEvent::Swap(CAROL, DOT, 1000, BTC, 34));
		assert!(System::events().iter().any(|record| record.event == swap_event));
		assert_eq!(Tokens::free_balance(BTC, &CAROL), 34);
		assert_eq!(Tokens::free_balance(DOT, &CAROL), 0);
		assert_eq!(DexModule::liquidity_pool(BTC), (66, 14950));
		assert_eq!(DexModule::liquidity_pool(DOT), (2000, 5050));
	});
}

#[test]
fn do_exchange_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 10000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 1000, 10000));
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
			Error::<Runtime>::InacceptablePrice,
		);
		assert_ok!(DexModule::do_exchange(&CAROL, BTC, 100, AUSD, 4950));
		assert_ok!(DexModule::do_exchange(&CAROL, AUSD, 4950, BTC, 90));
		assert_ok!(DexModule::do_exchange(&CAROL, BTC, 90, DOT, 300));
	});
}

#[test]
fn get_supply_amount_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 10000, 10000));
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
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 1000));
		assert_ok!(DexModule::add_liquidity(Origin::signed(ALICE), DOT, 200, 2000));
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
