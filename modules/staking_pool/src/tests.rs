//! Unit tests for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	BondingDuration, CurrenciesModule, ExtBuilder, Runtime, StakingPoolModule, System, TestEvent, ALICE, BOB, DOT,
	LDOT, TOTAL_COMMISSION,
};

#[test]
fn mint_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::free_unbonded(), 0);
		assert_eq!(StakingPoolModule::mint(&ALICE, 500), Ok(5000));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 5000);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);

		let mint_liquid_event = TestEvent::staking_pool(RawEvent::MintLiquid(ALICE, 500, 5000));
		assert!(System::events().iter().any(|record| record.event == mint_liquid_event));
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalClaimedUnbonded::put(500);
		<ClaimedUnbond<Runtime>>::insert(ALICE, StakingPoolModule::current_era(), 200);
		assert_ok!(CurrenciesModule::deposit(DOT, &StakingPoolModule::account_id(), 500));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 0), 200);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 500);

		assert_eq!(StakingPoolModule::withdraw_unbonded(&ALICE), Ok(200));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1200);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			300
		);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 0), 0);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 300);
	});
}

#[test]
fn redeem_by_unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::bond(500));
		assert_ok!(CurrenciesModule::transfer(Some(BOB).into(), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_unbond(&ALICE, 5000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (0, 0));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 500);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 1000);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			0
		);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&ALICE, 1000));
		let redeem_by_unbond_event_1 = TestEvent::staking_pool(RawEvent::RedeemByUnbond(ALICE, 1000, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_unbond_event_1));

		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (100, 100));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 400);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 900);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			100
		);

		// over the communal_bonded
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 9000);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&BOB, 0 + 1 + BondingDuration::get()),
			0
		);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&BOB, 9000));
		let redeem_by_unbond_event_2 = TestEvent::staking_pool(RawEvent::RedeemByUnbond(BOB, 4000, 400));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_unbond_event_2));

		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 5000);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (500, 500));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 0);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 500);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&BOB, 0 + 1 + BondingDuration::get()),
			400
		);
	});
}

#[test]
fn redeem_by_free_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::bond(500));
		assert_ok!(CurrenciesModule::transfer(Some(BOB).into(), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_free_unbonded(&ALICE, 5000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 0);

		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&ALICE, 1000));
		let redeem_by_free_unbonded_event_1 =
			TestEvent::staking_pool(RawEvent::RedeemByFreeUnbonded(ALICE, 100, 900, 90));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_free_unbonded_event_1));

		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 410);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			410
		);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1090);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 20);

		assert_eq!(CurrenciesModule::free_balance(DOT, &BOB), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 9000);

		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&BOB, 9000));
		let redeem_by_free_unbonded_event_2 =
			TestEvent::staking_pool(RawEvent::RedeemByFreeUnbonded(BOB, 451, 4060, 410));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_free_unbonded_event_2));

		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 0);
		assert_eq!(CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()), 0);
		assert_eq!(CurrenciesModule::free_balance(DOT, &BOB), 410);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 4489);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 110);
	});
}

#[test]
fn redeem_by_claim_unbonding_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CurrenciesModule::transfer(Some(ALICE).into(), BOB, DOT, 1000));
		assert_eq!(StakingPoolModule::mint(&BOB, 2000), Ok(20000));
		assert_ok!(StakingPoolModule::bond(1000));
		assert_ok!(CurrenciesModule::transfer(Some(BOB).into(), ALICE, LDOT, 1000));

		TotalBonded::mutate(|bonded| *bonded -= 500);
		Unbonding::insert(2, (500, 0));
		UnbondingToFree::put(500);

		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 1000);
		assert_eq!(StakingPoolModule::unbonding(2), (500, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 500);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 2), 0);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(StakingPoolModule::current_era(), 0);

		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 0, BondingDuration::get() + 1),
			Error::<Runtime>::InvalidEra,
		);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1001, 2),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);

		assert_eq!(
			StakingPoolModule::claim_period_percent(2),
			Ratio::saturating_from_rational(2, 4 + 1)
		);
		assert_eq!(StakingPoolModule::calculate_claim_fee(1000, 2), 60);

		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1000, 2));
		let redeem_by_claimed_unbonding_event_1 =
			TestEvent::staking_pool(RawEvent::RedeemByClaimUnbonding(ALICE, 2, 60, 940, 94));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_claimed_unbonding_event_1));

		assert_eq!(StakingPoolModule::unbonding(2), (500, 94));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 406);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 2), 94);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 12);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);

		// over the communal_bonded
		assert_eq!(StakingPoolModule::claimed_unbond(&BOB, 2), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 19000);

		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&BOB, 19000, 2));
		let redeem_by_claimed_unbonding_event_2 =
			TestEvent::staking_pool(RawEvent::RedeemByClaimUnbonding(BOB, 2, 258, 4049, 406));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_claimed_unbonding_event_2));

		assert_eq!(StakingPoolModule::unbonding(2), (500, 500));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 0);
		assert_eq!(StakingPoolModule::claimed_unbond(&BOB, 2), 406);
		assert_eq!(TOTAL_COMMISSION.with(|v| *v.borrow_mut()), 63);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 14693);
	});
}

#[test]
fn rebalance_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(20000);
		Unbonding::insert(1, (20000, 10000));
		UnbondingToFree::put(10000);
		NextEraUnbond::put((5000, 5000));

		assert_eq!(StakingPoolModule::current_era(), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 20000);
		assert_eq!(StakingPoolModule::free_unbonded(), 0);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 0);
		assert_eq!(CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()), 0);
		assert_eq!(StakingPoolModule::unbonding(1), (20000, 10000));
		assert_eq!(StakingPoolModule::unbonding(1 + BondingDuration::get()), (0, 0));
		assert_eq!(StakingPoolModule::next_era_unbond(), (5000, 5000));

		CurrentEra::put(1);
		StakingPoolModule::rebalance(1);

		assert_eq!(StakingPoolModule::current_era(), 1);
		assert_eq!(StakingPoolModule::total_bonded(), 15000);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			20000
		);
		assert_eq!(StakingPoolModule::unbonding(1), (0, 0));
		assert_eq!(StakingPoolModule::unbonding(1 + BondingDuration::get()), (5000, 5000));
		assert_eq!(StakingPoolModule::next_era_unbond(), (0, 0));
	});
}
