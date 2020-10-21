//! Unit tests for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use orml_rewards::PoolInfo;
use orml_traits::MultiCurrency;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn deposit_dex_share_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(TokensModule::deposit(BTC_AUSD_LP, &ALICE, 10000));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE), 10000);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			0
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE),
			(0, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE),
			(0, 0)
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000
		));
		let deposit_dex_share_event = TestEvent::incentives(RawEvent::DepositDEXShare(ALICE, BTC_AUSD_LP, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == deposit_dex_share_event));

		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE), 0);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			10000
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE),
			(10000, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE),
			(10000, 0)
		);
	});
}

#[test]
fn withdraw_dex_share_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(TokensModule::deposit(BTC_AUSD_LP, &ALICE, 10000));

		assert_noop!(
			IncentivesModule::withdraw_dex_share(Origin::signed(BOB), BTC_AUSD_LP, 10000),
			Error::<Runtime>::NotEnough,
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			10000
		));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE), 0);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			10000
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE),
			(10000, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE),
			(10000, 0)
		);

		assert_ok!(IncentivesModule::withdraw_dex_share(
			Origin::signed(ALICE),
			BTC_AUSD_LP,
			8000
		));
		let withdraw_dex_share_event = TestEvent::incentives(RawEvent::WithdrawDEXShare(ALICE, BTC_AUSD_LP, 8000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == withdraw_dex_share_event));

		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE), 8000);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			2000
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 2000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 2000,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE),
			(2000, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE),
			(2000, 0)
		);
	});
}

#[test]
fn update_loans_incentive_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_loans_incentive_rewards(Origin::signed(ALICE), vec![]),
			BadOrigin
		);
		assert_eq!(IncentivesModule::loans_incentive_rewards(BTC), 0);
		assert_eq!(IncentivesModule::loans_incentive_rewards(DOT), 0);

		assert_ok!(IncentivesModule::update_loans_incentive_rewards(
			Origin::signed(4),
			vec![(BTC, 200), (DOT, 1000),],
		));
		assert_eq!(IncentivesModule::loans_incentive_rewards(BTC), 200);
		assert_eq!(IncentivesModule::loans_incentive_rewards(DOT), 1000);

		assert_ok!(IncentivesModule::update_loans_incentive_rewards(
			Origin::signed(4),
			vec![(BTC, 100), (BTC, 300), (BTC, 500),],
		));
		assert_eq!(IncentivesModule::loans_incentive_rewards(BTC), 500);
	});
}

#[test]
fn update_dex_incentive_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_dex_incentive_rewards(Origin::signed(ALICE), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_dex_incentive_rewards(Origin::signed(4), vec![(BTC, 200), (DOT, 1000)],),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_eq!(IncentivesModule::dex_incentive_rewards(BTC_AUSD_LP), 0);
		assert_eq!(IncentivesModule::dex_incentive_rewards(DOT_AUSD_LP), 0);

		assert_ok!(IncentivesModule::update_dex_incentive_rewards(
			Origin::signed(4),
			vec![(BTC_AUSD_LP, 200), (DOT_AUSD_LP, 1000)],
		));
		assert_eq!(IncentivesModule::dex_incentive_rewards(BTC_AUSD_LP), 200);
		assert_eq!(IncentivesModule::dex_incentive_rewards(DOT_AUSD_LP), 1000);

		assert_ok!(IncentivesModule::update_dex_incentive_rewards(
			Origin::signed(4),
			vec![(BTC_AUSD_LP, 100), (BTC_AUSD_LP, 300), (BTC_AUSD_LP, 500),],
		));
		assert_eq!(IncentivesModule::dex_incentive_rewards(BTC_AUSD_LP), 500);
	});
}

#[test]
fn update_homa_incentive_reward_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_homa_incentive_reward(Origin::signed(ALICE), 100),
			BadOrigin
		);
		assert_eq!(IncentivesModule::homa_incentive_reward(), 0);

		assert_ok!(IncentivesModule::update_homa_incentive_reward(Origin::signed(4), 100));
		assert_eq!(IncentivesModule::homa_incentive_reward(), 100);
	});
}

#[test]
fn update_dex_saving_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_dex_saving_rates(Origin::signed(ALICE), vec![]),
			BadOrigin
		);

		assert_noop!(
			IncentivesModule::update_dex_saving_rates(
				Origin::signed(4),
				vec![(BTC, Rate::saturating_from_rational(1, 10000)),],
			),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_eq!(IncentivesModule::dex_saving_rates(BTC_AUSD_LP), Rate::zero());
		assert_eq!(IncentivesModule::dex_saving_rates(DOT_AUSD_LP), Rate::zero());

		assert_ok!(IncentivesModule::update_dex_saving_rates(
			Origin::signed(4),
			vec![
				(BTC_AUSD_LP, Rate::saturating_from_rational(1, 10000)),
				(DOT_AUSD_LP, Rate::saturating_from_rational(1, 5000)),
			],
		));
		assert_eq!(
			IncentivesModule::dex_saving_rates(BTC_AUSD_LP),
			Rate::saturating_from_rational(1, 10000)
		);
		assert_eq!(
			IncentivesModule::dex_saving_rates(DOT_AUSD_LP),
			Rate::saturating_from_rational(1, 5000)
		);

		assert_ok!(IncentivesModule::update_dex_saving_rates(
			Origin::signed(4),
			vec![
				(BTC_AUSD_LP, Rate::saturating_from_rational(1, 20000)),
				(BTC_AUSD_LP, Rate::saturating_from_rational(1, 30000)),
				(BTC_AUSD_LP, Rate::saturating_from_rational(1, 40000)),
			],
		));
		assert_eq!(
			IncentivesModule::dex_saving_rates(BTC_AUSD_LP),
			Rate::saturating_from_rational(1, 40000)
		);
	});
}

#[test]
fn on_add_liquidity_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), ALICE),
			(0, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), ALICE),
			(0, 0)
		);

		OnAddLiquidity::<Runtime>::happened(&(ALICE, BTC, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC)),
			PoolInfo {
				total_shares: 100,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC)),
			PoolInfo {
				total_shares: 100,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), ALICE),
			(100, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), ALICE),
			(100, 0)
		);

		OnAddLiquidity::<Runtime>::happened(&(BOB, BTC, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC)),
			PoolInfo {
				total_shares: 200,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC)),
			PoolInfo {
				total_shares: 200,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), BOB),
			(100, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), BOB),
			(100, 0)
		);
	});
}

#[test]
fn on_remove_liquidity_works() {
	ExtBuilder::default().build().execute_with(|| {
		OnAddLiquidity::<Runtime>::happened(&(ALICE, BTC, 100));
		OnAddLiquidity::<Runtime>::happened(&(BOB, BTC, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC)),
			PoolInfo {
				total_shares: 200,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC)),
			PoolInfo {
				total_shares: 200,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), ALICE),
			(100, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), ALICE),
			(100, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), BOB),
			(100, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), BOB),
			(100, 0)
		);

		OnRemoveLiquidity::<Runtime>::happened(&(ALICE, BTC, 40));
		OnRemoveLiquidity::<Runtime>::happened(&(BOB, BTC, 70));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC)),
			PoolInfo {
				total_shares: 90,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC)),
			PoolInfo {
				total_shares: 90,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), ALICE),
			(60, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), ALICE),
			(60, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC), BOB),
			(30, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC), BOB),
			(30, 0)
		);
	});
}

#[test]
fn on_update_loan_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			RewardsModule::pools(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), ALICE),
			(0, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), BOB),
			(0, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE, BTC, 100, 0));
		assert_eq!(
			RewardsModule::pools(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 100,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), ALICE),
			(100, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB, BTC, 100, 500));
		assert_eq!(
			RewardsModule::pools(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 700,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), BOB),
			(600, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE, BTC, -50, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 650,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), ALICE),
			(50, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB, BTC, -650, 600));
		assert_eq!(
			RewardsModule::pools(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 50,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::Loans(BTC), BOB),
			(0, 0)
		);
	});
}

#[test]
fn pay_out_works_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(ACA, &LoansIncentivePool::get(), 10000));
		assert_ok!(TokensModule::deposit(ACA, &DexIncentivePool::get(), 10000));
		assert_ok!(TokensModule::deposit(AUSD, &DexIncentivePool::get(), 10000));
		assert_ok!(TokensModule::deposit(ACA, &HomaIncentivePool::get(), 10000));

		assert_eq!(TokensModule::free_balance(ACA, &LoansIncentivePool::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE), 0);
		IncentivesModule::payout(&ALICE, PoolId::Loans(BTC), 1000);
		assert_eq!(TokensModule::free_balance(ACA, &LoansIncentivePool::get()), 9000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE), 1000);

		assert_eq!(TokensModule::free_balance(ACA, &DexIncentivePool::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 0);
		IncentivesModule::payout(&BOB, PoolId::DexIncentive(BTC), 1000);
		assert_eq!(TokensModule::free_balance(ACA, &DexIncentivePool::get()), 9000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 1000);

		assert_eq!(TokensModule::free_balance(AUSD, &DexIncentivePool::get()), 10000);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE), 0);
		IncentivesModule::payout(&ALICE, PoolId::DexSaving(BTC), 1000);
		assert_eq!(TokensModule::free_balance(AUSD, &DexIncentivePool::get()), 9000);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE), 1000);

		assert_eq!(TokensModule::free_balance(ACA, &HomaIncentivePool::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 1000);
		IncentivesModule::payout(&BOB, PoolId::Homa, 3000);
		assert_eq!(TokensModule::free_balance(ACA, &HomaIncentivePool::get()), 7000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 4000);
	});
}

#[test]
fn accumulate_reward_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(IncentivesModule::update_loans_incentive_rewards(
			Origin::signed(4),
			vec![(BTC, 1000), (DOT, 2000),],
		));
		assert_ok!(IncentivesModule::update_dex_incentive_rewards(
			Origin::signed(4),
			vec![(BTC_AUSD_LP, 100), (DOT_AUSD_LP, 200),],
		));
		assert_ok!(IncentivesModule::update_homa_incentive_reward(Origin::signed(4), 30));
		assert_ok!(IncentivesModule::update_dex_saving_rates(
			Origin::signed(4),
			vec![
				(BTC_AUSD_LP, Rate::saturating_from_rational(1, 100)),
				(DOT_AUSD_LP, Rate::saturating_from_rational(1, 100)),
			],
		));

		assert_eq!(IncentivesModule::accumulate_reward(10, |_, _| {}), vec![]);

		RewardsModule::add_share(&ALICE, PoolId::Loans(BTC), 1);
		assert_eq!(IncentivesModule::accumulate_reward(20, |_, _| {}), vec![(ACA, 1000)]);

		RewardsModule::add_share(&ALICE, PoolId::Loans(DOT), 1);
		assert_eq!(IncentivesModule::accumulate_reward(30, |_, _| {}), vec![(ACA, 3000)]);

		RewardsModule::add_share(&ALICE, PoolId::DexIncentive(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE, PoolId::DexSaving(BTC_AUSD_LP), 1);
		assert_eq!(
			IncentivesModule::accumulate_reward(40, |_, _| {}),
			vec![(ACA, 3100), (AUSD, 5)]
		);

		RewardsModule::add_share(&ALICE, PoolId::DexIncentive(DOT_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE, PoolId::DexSaving(DOT_AUSD_LP), 1);
		assert_eq!(
			IncentivesModule::accumulate_reward(50, |_, _| {}),
			vec![(ACA, 3300), (AUSD, 9)]
		);

		RewardsModule::add_share(&ALICE, PoolId::Homa, 1);
		assert_eq!(
			IncentivesModule::accumulate_reward(50, |_, _| {}),
			vec![(ACA, 3330), (AUSD, 9)]
		);

		assert_eq!(IncentivesModule::accumulate_reward(59, |_, _| {}), vec![]);

		mock_shutdown();
		assert_eq!(IncentivesModule::accumulate_reward(60, |_, _| {}), vec![]);
	});
}
