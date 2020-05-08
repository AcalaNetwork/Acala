//! Unit tests for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	BondingDuration, CurrenciesModule, ExtBuilder, Runtime, StakingPoolModule, System, TestEvent, ALICE, BOB, DOT, LDOT,
};

#[test]
fn claim_period_percent_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_noop!(
			StakingPoolModule::bond(&ALICE, 1001),
			Error::<Runtime>::StakingCurrencyNotEnough,
		);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::bond(&ALICE, 500), Ok(5000));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 500);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 5000);

		let bond_and_mint_event = TestEvent::staking_pool(RawEvent::BondAndMint(ALICE, 500, 5000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == bond_and_mint_event));
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		<TotalClaimedUnbonded<Runtime>>::put(500);
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
		assert_eq!(StakingPoolModule::bond(&ALICE, 1000), Ok(10000));
		assert_eq!(StakingPoolModule::bond(&BOB, 1000), Ok(10000));
		assert_eq!(StakingPoolModule::total_bonded(), 2000);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 2000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 10000);
		assert_eq!(StakingPoolModule::next_era_unbond(), (0, 0));
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			0
		);
		assert_noop!(
			StakingPoolModule::redeem_by_unbond(&ALICE, 15000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);
		assert_ok!(StakingPoolModule::redeem_by_unbond(&ALICE, 5000));
		assert_eq!(StakingPoolModule::total_bonded(), 1500);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 1000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 5000);
		assert_eq!(StakingPoolModule::next_era_unbond(), (500, 500));
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			500
		);

		let redeem_by_unbond_event = TestEvent::staking_pool(RawEvent::RedeemByUnbond(ALICE, 5000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_unbond_event));
	});
}

#[test]
fn redeem_by_free_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::bond(&ALICE, 1000), Ok(10000));
		assert_eq!(StakingPoolModule::bond(&BOB, 1000), Ok(10000));
		<TotalBonded<Runtime>>::mutate(|bonded| *bonded -= 1500);
		<FreeUnbonded<Runtime>>::put(1500);
		assert_ok!(CurrenciesModule::deposit(DOT, &StakingPoolModule::account_id(), 1500));
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 1500);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			1500
		);

		assert_noop!(
			StakingPoolModule::redeem_by_free_unbonded(&ALICE, 15000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);

		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&ALICE, 10000));
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 600);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 900);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			600
		);

		let redeem_by_free_unbonded_event =
			TestEvent::staking_pool(RawEvent::RedeemByFreeUnbonded(ALICE, 1000, 9000, 900));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_free_unbonded_event));
	});
}

#[test]
fn redeem_by_claim_unbonding_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::bond(&ALICE, 1000), Ok(10000));
		assert_eq!(StakingPoolModule::bond(&BOB, 1000), Ok(10000));
		<TotalBonded<Runtime>>::mutate(|bonded| *bonded -= 1500);
		<Unbonding<Runtime>>::insert(2, (1500, 0));
		<UnbondingToFree<Runtime>>::put(1500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 10000);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::unbonding(2), (1500, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 1500);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 2), 0);

		assert_eq!(StakingPoolModule::current_era(), 0);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 0, BondingDuration::get() + 1),
			Error::<Runtime>::InvalidEra,
		);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 15000, 2),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);
		assert_eq!(
			StakingPoolModule::claim_period_percent(2),
			Ratio::from_rational(2, 4 + 1)
		);
		assert_eq!(StakingPoolModule::calculate_claim_fee(10000, 2), 600);
		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 10000, 2));
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::unbonding(2), (1500, 940));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 560);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 2), 940);

		let redeem_by_claim_unbonding_event =
			TestEvent::staking_pool(RawEvent::RedeemByClaimUnbonding(ALICE, 2, 600, 9400, 940));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_claim_unbonding_event));
	});
}

#[test]
fn rebalance_work() {
	ExtBuilder::default().build().execute_with(|| {
		<TotalBonded<Runtime>>::put(20000);
		<Unbonding<Runtime>>::insert(1, (20000, 10000));
		<UnbondingToFree<Runtime>>::put(10000);
		<NextEraUnbond<Runtime>>::put((5000, 5000));

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
