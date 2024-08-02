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

//! Unit tests for the incentives module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeEvent, *};
use orml_rewards::PoolInfo;
use orml_traits::MultiCurrency;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn deposit_dex_share_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(BTC_AUSD_LP, &ALICE::get(), 10000));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 10000);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			0
		);
		assert_eq!(RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)), PoolInfo::default(),);

		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			Default::default(),
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			RuntimeOrigin::signed(ALICE::get()),
			BTC_AUSD_LP,
			10000
		));
		System::assert_last_event(RuntimeEvent::IncentivesModule(crate::Event::DepositDexShare {
			who: ALICE::get(),
			dex_share_type: BTC_AUSD_LP,
			deposit: 10000,
		}));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 0);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			10000
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(10000, Default::default())
		);
	});
}

#[test]
fn withdraw_dex_share_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(BTC_AUSD_LP, &ALICE::get(), 10000));

		assert_noop!(
			IncentivesModule::withdraw_dex_share(RuntimeOrigin::signed(BOB::get()), BTC_AUSD_LP, 10000),
			Error::<Runtime>::NotEnough,
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			RuntimeOrigin::signed(ALICE::get()),
			BTC_AUSD_LP,
			10000
		));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 0);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			10000
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(10000, Default::default())
		);

		assert_ok!(IncentivesModule::withdraw_dex_share(
			RuntimeOrigin::signed(ALICE::get()),
			BTC_AUSD_LP,
			8000
		));
		System::assert_last_event(RuntimeEvent::IncentivesModule(crate::Event::WithdrawDexShare {
			who: ALICE::get(),
			dex_share_type: BTC_AUSD_LP,
			withdraw: 8000,
		}));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 8000);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			2000
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 2000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(2000, Default::default())
		);
	});
}

#[test]
fn update_incentive_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_incentive_rewards(RuntimeOrigin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_incentive_rewards(
				RuntimeOrigin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT), vec![])]
			),
			Error::<Runtime>::InvalidPoolId
		);

		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), ACA),
			0
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), DOT),
			0
		);
		assert_eq!(IncentivesModule::incentive_reward_amounts(PoolId::Loans(DOT), ACA), 0);

		assert_ok!(IncentivesModule::update_incentive_rewards(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 1000), (DOT, 100)]),
				(PoolId::Loans(DOT), vec![(ACA, 500)]),
			],
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::IncentiveRewardAmountUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				reward_currency_id: ACA,
				reward_amount_per_period: 1000,
			},
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::IncentiveRewardAmountUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				reward_currency_id: DOT,
				reward_amount_per_period: 100,
			},
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::IncentiveRewardAmountUpdated {
				pool: PoolId::Loans(DOT),
				reward_currency_id: ACA,
				reward_amount_per_period: 500,
			},
		));
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), ACA),
			1000
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), DOT),
			true
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), DOT),
			100
		);
		assert_eq!(IncentivesModule::incentive_reward_amounts(PoolId::Loans(DOT), ACA), 500);

		assert_ok!(IncentivesModule::update_incentive_rewards(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 200), (DOT, 0)]),
				(PoolId::Loans(DOT), vec![(ACA, 500)]),
			],
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::IncentiveRewardAmountUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				reward_currency_id: ACA,
				reward_amount_per_period: 200,
			},
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::IncentiveRewardAmountUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				reward_currency_id: DOT,
				reward_amount_per_period: 0,
			},
		));
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), ACA),
			200
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), DOT),
			false
		);
		assert_eq!(IncentivesModule::incentive_reward_amounts(PoolId::Loans(DOT), ACA), 500);
	});
}

#[test]
fn update_claim_reward_deduction_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(RuntimeOrigin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(
				RuntimeOrigin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(
				RuntimeOrigin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(101, 100)),]
			),
			Error::<Runtime>::InvalidRate,
		);

		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(2, 100))
			]
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::ClaimRewardDeductionRateUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				deduction_rate: Rate::saturating_from_rational(1, 100),
			},
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::ClaimRewardDeductionRateUpdated {
				pool: PoolId::Dex(BTC_AUSD_LP),
				deduction_rate: Rate::saturating_from_rational(2, 100),
			},
		));
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			true
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(BTC_AUSD_LP)),
			Rate::saturating_from_rational(2, 100)
		);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(5, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::zero())
			]
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::ClaimRewardDeductionRateUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				deduction_rate: Rate::saturating_from_rational(5, 100),
			},
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::ClaimRewardDeductionRateUpdated {
				pool: PoolId::Dex(BTC_AUSD_LP),
				deduction_rate: Rate::zero(),
			},
		));
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(5, 100)
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(&PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);
	});
}

#[test]
fn on_update_loan_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(RewardsModule::pool_infos(PoolId::Loans(BTC)), PoolInfo::default(),);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			Default::default(),
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			Default::default(),
		);

		assert_ok!(OnUpdateLoan::<Runtime>::handle(&(ALICE::get(), BTC, 100, 0)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 100,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, Default::default())
		);

		assert_ok!(OnUpdateLoan::<Runtime>::handle(&(ALICE::get(), BTC, 100, 100)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(200, Default::default())
		);

		assert_ok!(OnUpdateLoan::<Runtime>::handle(&(BOB::get(), BTC, 600, 0)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 800,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(600, Default::default())
		);

		assert_ok!(OnUpdateLoan::<Runtime>::handle(&(ALICE::get(), BTC, -50, 200)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 750,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(150, Default::default())
		);

		assert_ok!(OnUpdateLoan::<Runtime>::handle(&(BOB::get(), BTC, -600, 600)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 150,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			Default::default(),
		);
	});
}

#[test]
fn payout_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			BTreeMap::default()
		);

		IncentivesModule::payout(&ALICE::get(), &PoolId::Loans(BTC), ACA, 1000);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			vec![(ACA, 1000)].into_iter().collect()
		);

		IncentivesModule::payout(&ALICE::get(), &PoolId::Loans(BTC), ACA, 1000);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			vec![(ACA, 2000)].into_iter().collect()
		);
	});
}

#[test]
fn transfer_failed_when_claim_rewards() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 27));
		assert_ok!(TokensModule::deposit(DOT, &VAULT::get(), 30));
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 100));
		assert_ok!(RewardsModule::add_share(&BOB::get(), &PoolId::Loans(BTC), 200));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), AUSD, 27));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), DOT, 30));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 27);
		assert_eq!(TokensModule::free_balance(DOT, &VAULT::get()), 30);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(TokensModule::free_balance(DOT, &ALICE::get()), 0);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (27, 0)), (DOT, (30, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			Default::default()
		);

		// Alice claim rewards:
		// payout AUSD failed for ED, the pending reward record of AUSD will not change.
		// payout DOT succeed.
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			PoolId::Loans(BTC)
		));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 27);
		assert_eq!(TokensModule::free_balance(DOT, &VAULT::get()), 20);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(TokensModule::free_balance(DOT, &ALICE::get()), 10);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (27, 9)), (DOT, (30, 10))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, vec![(AUSD, 9), (DOT, 10)].into_iter().collect())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			vec![(AUSD, 9)].into_iter().collect()
		);

		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 0);
		assert_eq!(TokensModule::free_balance(DOT, &BOB::get()), 0);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(200, Default::default())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), BOB::get()),
			Default::default()
		);

		// BOB claimed DOT and AUSD rewards.
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(BOB::get()),
			PoolId::Loans(BTC)
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: BOB::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: AUSD,
			actual_amount: 18,
			deduction_amount: 0,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: BOB::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: DOT,
			actual_amount: 20,
			deduction_amount: 0,
		}));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 9);
		assert_eq!(TokensModule::free_balance(DOT, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 18);
		assert_eq!(TokensModule::free_balance(DOT, &BOB::get()), 20);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (27, 27)), (DOT, (30, 30))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(200, vec![(AUSD, 18), (DOT, 20)].into_iter().collect())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), BOB::get()),
			Default::default()
		);

		// accumulate 6 aUSD
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 6));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), AUSD, 6));
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 15);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (33, 27)), (DOT, (30, 30))].into_iter().collect(),
			}
		);

		// Alice claim AUSD reward
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			PoolId::Loans(BTC)
		));
		System::assert_last_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: AUSD,
			actual_amount: 11,
			deduction_amount: 0,
		}));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 4);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 11);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (33, 29)), (DOT, (30, 30))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, vec![(AUSD, 11), (DOT, 10)].into_iter().collect())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), ALICE::get()),
			Default::default()
		);

		// mock the Vault is enough for some reasons
		assert_ok!(TokensModule::withdraw(AUSD, &VAULT::get(), 3));
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 1);

		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 18);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(200, vec![(AUSD, 18), (DOT, 20)].into_iter().collect())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), BOB::get()),
			Default::default()
		);

		// Bob claim rewards, payout AUSD failed for drained vault, the pending reward record of AUSD will
		// not change.
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(BOB::get()),
			PoolId::Loans(BTC)
		));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 1);
		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 18);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 300,
				rewards: vec![(AUSD, (33, 33)), (DOT, (30, 30))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(200, vec![(AUSD, 22), (DOT, 20)].into_iter().collect())
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(BTC), BOB::get()),
			vec![(AUSD, 4)].into_iter().collect()
		);
	});
}

#[test]
fn claim_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(ACA, &VAULT::get(), 10000));
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 10000));
		assert_ok!(TokensModule::deposit(LDOT, &VAULT::get(), 10000));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(20, 100)),
				(PoolId::Loans(BTC), Rate::saturating_from_rational(20, 100)),
			]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(40, 100)),
				(PoolId::Loans(BTC), Rate::saturating_from_rational(40, 100)),
			]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(50, 100)),
				(PoolId::Loans(BTC), Rate::saturating_from_rational(60, 100)),
			]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![(PoolId::Loans(BTC), Rate::saturating_from_rational(80, 100)),]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![(PoolId::Loans(BTC), Rate::saturating_from_rational(90, 100)),]
		));

		// alice add shares before accumulate rewards
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 100));
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(BTC_AUSD_LP), 100));

		// bob add shares before accumulate rewards
		assert_ok!(RewardsModule::add_share(&BOB::get(), &PoolId::Dex(BTC_AUSD_LP), 100));

		// accumulate rewards for different pools
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), ACA, 2000));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Dex(BTC_AUSD_LP), ACA, 1000));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Dex(BTC_AUSD_LP), AUSD, 2000));

		// bob add share after accumulate rewards
		assert_ok!(RewardsModule::add_share(&BOB::get(), &PoolId::Loans(BTC), 100));

		// accumulate LDOT rewards for PoolId::Loans(BTC)
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), LDOT, 500));

		// alice claim rewards for PoolId::Loans(BTC)
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (4000, 2000)), (LDOT, (500, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 0);
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			PoolId::Loans(BTC)
		));

		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: ACA,
			actual_amount: 200,
			deduction_amount: 1800,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: LDOT,
			actual_amount: 25,
			deduction_amount: 225,
		}));

		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (5800, 4000)), (LDOT, (725, 250))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, vec![(ACA, 2000), (LDOT, 250)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 9800);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 9975);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 200);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 25);

		// bob claim rewards for PoolId::Loans(BTC)
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(100, vec![(ACA, 2000)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 0);
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(BOB::get()),
			PoolId::Loans(BTC)
		));

		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: BOB::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: ACA,
			actual_amount: 90,
			deduction_amount: 810,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: BOB::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: LDOT,
			actual_amount: 37,
			deduction_amount: 325,
		}));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (6610, 4900)), (LDOT, (1050, 612))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(100, vec![(ACA, 2900), (LDOT, 362)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 9710);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 9938);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 90);
		assert_eq!(TokensModule::free_balance(LDOT, &BOB::get()), 37);

		// alice remove share for PoolId::Dex(BTC_AUSD_LP) before claim rewards,
		// rewards will be settled and as pending rewards, will not be deducted.
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (1000, 0)), (AUSD, (2000, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 9710);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 200);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			BTreeMap::default()
		);
		assert_ok!(RewardsModule::remove_share(
			&ALICE::get(),
			&PoolId::Dex(BTC_AUSD_LP),
			50
		));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 150,
				rewards: vec![(ACA, (750, 250)), (AUSD, (1500, 500))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(50, vec![(ACA, 250), (AUSD, 500)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 9710);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 200);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			vec![(ACA, 500), (AUSD, 1000)].into_iter().collect()
		);

		// alice claim rewards for PoolId::Dex(BTC_AUSD_LP)
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			PoolId::Dex(BTC_AUSD_LP)
		));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Dex(BTC_AUSD_LP),
			reward_currency_id: ACA,
			actual_amount: 250,
			deduction_amount: 250,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Dex(BTC_AUSD_LP),
			reward_currency_id: AUSD,
			actual_amount: 500,
			deduction_amount: 500,
		}));

		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 150,
				rewards: vec![(ACA, (1000, 250)), (AUSD, (2000, 500))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			(50, vec![(ACA, 250), (AUSD, 500)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 9460);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 9500);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 450);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 500);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Dex(BTC_AUSD_LP), ALICE::get()),
			BTreeMap::default()
		);
	});
}

#[test]
fn on_initialize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(ACA, &RewardsSource::get(), 10000));
		assert_ok!(TokensModule::deposit(AUSD, &RewardsSource::get(), 10000));
		assert_ok!(TokensModule::deposit(LDOT, &RewardsSource::get(), 10000));

		assert_ok!(IncentivesModule::update_incentive_rewards(
			RuntimeOrigin::signed(ROOT::get()),
			vec![
				(PoolId::Loans(BTC), vec![(ACA, 1000), (AUSD, 500)]),
				(PoolId::Loans(DOT), vec![(ACA, 2000), (LDOT, 50)]),
				(PoolId::Dex(BTC_AUSD_LP), vec![(ACA, 100)]),
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 200)]),
				(PoolId::Earning(ACA), vec![(ACA, 100)]),
			],
		));

		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 1));
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(BTC_AUSD_LP), 1));
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(DOT_AUSD_LP), 1));
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Earning(ACA), 1));

		assert_eq!(TokensModule::free_balance(ACA, &RewardsSource::get()), 10000);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 10000);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(DOT)),
			PoolInfo {
				total_shares: 0,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);

		// per 10 blocks will accumulate rewards, nothing happened when on_initialize(9)
		IncentivesModule::on_initialize(9);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0);

		IncentivesModule::on_initialize(10);
		assert_eq!(
			TokensModule::free_balance(ACA, &RewardsSource::get()),
			10000 - (1000 + 200 + 100 + 100)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 10000 - 500);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 1000 + 200 + 100 + 100);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 500);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0);
		// 1000 ACA and 500 AUSD are incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (1000, 0)), (AUSD, (500, 0))].into_iter().collect(),
			}
		);
		// because total_shares of PoolId::Loans(DOT) is zero, will not accumulate rewards
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(DOT)),
			PoolInfo {
				total_shares: 0,
				..Default::default()
			}
		);
		// 100 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (100, 0))].into_iter().collect(),
			}
		);
		// 200 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (200, 0))].into_iter().collect(),
			}
		);
		// 100 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (100, 0))].into_iter().collect(),
			}
		);

		// add share for PoolId::Loans(DOT)
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(DOT), 1));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(DOT)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);

		IncentivesModule::on_initialize(20);
		assert_eq!(
			TokensModule::free_balance(ACA, &RewardsSource::get()),
			8600 - (1000 + 2000 + 100 + 200 + 100)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 9500 - 500);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 10000 - 50);
		assert_eq!(
			TokensModule::free_balance(ACA, &VAULT::get()),
			1400 + (1000 + 2000 + 100 + 200 + 100)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 500 + 500); // 500 from RewardsSource
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0 + 50);
		// 1000 ACA and 500 AUSD are incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (2000, 0)), (AUSD, (1000, 0))].into_iter().collect(),
			}
		);
		// 2000 ACA and 50 LDOT are incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(DOT)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (2000, 0)), (LDOT, (50, 0))].into_iter().collect(),
			}
		);
		// 100 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (200, 0))].into_iter().collect(),
			}
		);
		// 200 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (400, 0))].into_iter().collect(),
			}
		);
		// 100 ACA is incentive reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (200, 0))].into_iter().collect(),
			}
		);

		mock_shutdown();
		IncentivesModule::on_initialize(30);
		assert_eq!(
			TokensModule::free_balance(ACA, &RewardsSource::get()),
			5200 - (100 + 200 + 100)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 9000);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 9950);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 4800 + (100 + 200 + 100));
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 1000);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 50);
		// PoolId::Loans will not accumulate incentive rewards after shutdown
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (2000, 0)), (AUSD, (1000, 0))].into_iter().collect(),
			}
		);
		// PoolId::Loans will not accumulate incentive rewards after shutdown
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(DOT)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (2000, 0)), (LDOT, (50, 0))].into_iter().collect(),
			}
		);
		// after shutdown, PoolId::Dex will accumulate incentive rewards
		// reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (300, 0))].into_iter().collect(),
			}
		);
		// after shutdown, PoolId::Dex will accumulate incentive rewards
		// reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (600, 0))].into_iter().collect(),
			}
		);
		// after shutdown, PoolId::Earning will accumulate incentive rewards
		// reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (300, 0))].into_iter().collect(),
			}
		);
	});
}

#[test]
fn earning_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(OnEarningBonded::<Runtime>::handle(&(ALICE::get(), 80)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 80,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Earning(ACA), ALICE::get()),
			(80, Default::default())
		);

		assert_ok!(OnEarningUnbonded::<Runtime>::handle(&(ALICE::get(), 20)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo {
				total_shares: 60,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Earning(ACA), ALICE::get()),
			(60, Default::default())
		);

		assert_ok!(OnEarningUnbonded::<Runtime>::handle(&(ALICE::get(), 60)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Earning(ACA)),
			PoolInfo { ..Default::default() }
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Earning(ACA), ALICE::get()),
			(0, Default::default())
		);
	});
}

#[test]
fn transfer_reward_and_update_rewards_storage_atomically_when_accumulate_incentives_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(AUSD, &RewardsSource::get(), 100));
		assert_ok!(TokensModule::deposit(ACA, &RewardsSource::get(), 100));
		assert_eq!(TokensModule::free_balance(ACA, &RewardsSource::get()), 100);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 100);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(
			orml_rewards::PoolInfos::<Runtime>::contains_key(PoolId::Dex(LDOT)),
			false
		);

		assert_ok!(IncentivesModule::update_incentive_rewards(
			RuntimeOrigin::signed(ROOT::get()),
			vec![(PoolId::Loans(LDOT), vec![(ACA, 30), (AUSD, 90)]),],
		));

		// accumulate ACA and AUSD failed, because pool dosen't exist
		IncentivesModule::accumulate_incentives(PoolId::Loans(LDOT));
		assert_eq!(
			orml_rewards::PoolInfos::<Runtime>::contains_key(PoolId::Dex(LDOT)),
			false
		);
		assert_eq!(TokensModule::free_balance(ACA, &RewardsSource::get()), 100);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 100);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);

		assert_ok!(RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(LDOT), 1));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(LDOT)),
			PoolInfo {
				total_shares: 1,
				..Default::default()
			}
		);

		// accumulate ACA and AUSD rewards succeeded
		IncentivesModule::accumulate_incentives(PoolId::Loans(LDOT));
		assert_eq!(TokensModule::free_balance(ACA, &RewardsSource::get()), 70);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 10);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 30);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 90);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(LDOT)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (30, 0)), (AUSD, (90, 0))].into_iter().collect()
			}
		);

		// accumulate ACA reward succeeded， accumulate AUSD reward failed
		IncentivesModule::accumulate_incentives(PoolId::Loans(LDOT));
		assert_eq!(TokensModule::free_balance(ACA, &RewardsSource::get()), 40);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 10);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 60);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 90);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(LDOT)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (60, 0)), (AUSD, (90, 0))].into_iter().collect()
			}
		);
	});
}

#[test]
fn update_claim_reward_deduction_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_currency(
				RuntimeOrigin::signed(ALICE::get()),
				PoolId::Dex(DOT_AUSD_LP),
				Some(ACA)
			),
			BadOrigin
		);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(10, 100)),]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_currency(
			RuntimeOrigin::signed(ROOT::get()),
			PoolId::Dex(DOT_AUSD_LP),
			Some(ACA)
		),);
		System::assert_has_event(RuntimeEvent::IncentivesModule(
			crate::Event::ClaimRewardDeductionCurrencyUpdated {
				pool: PoolId::Dex(DOT_AUSD_LP),
				currency: Some(ACA),
			},
		));

		assert_eq!(
			ClaimRewardDeductionCurrency::<Runtime>::get(PoolId::Dex(DOT_AUSD_LP)),
			Some(ACA)
		);
	});
}

#[test]
fn claim_reward_deduction_currency_works() {
	ExtBuilder::default().build().execute_with(|| {
		let pool_id = PoolId::Dex(DOT_AUSD_LP);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			RuntimeOrigin::signed(ROOT::get()),
			vec![(pool_id, Rate::saturating_from_rational(10, 100)),]
		));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_currency(
			RuntimeOrigin::signed(ROOT::get()),
			pool_id,
			Some(ACA)
		));

		assert_ok!(TokensModule::deposit(ACA, &VAULT::get(), 10000));
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 10000));

		// alice add shares before accumulate rewards
		assert_ok!(RewardsModule::add_share(&ALICE::get(), &pool_id, 100));

		// bob add shares before accumulate rewards
		assert_ok!(RewardsModule::add_share(&BOB::get(), &pool_id, 100));

		// accumulate rewards
		assert_ok!(RewardsModule::accumulate_reward(&pool_id, ACA, 1000));
		assert_ok!(RewardsModule::accumulate_reward(&pool_id, AUSD, 2000));

		// alice claim rewards
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			pool_id
		));

		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: pool_id,
			reward_currency_id: ACA,
			actual_amount: 450,
			deduction_amount: 50,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: pool_id,
			reward_currency_id: AUSD,
			actual_amount: 1000,
			deduction_amount: 0,
		}));

		System::reset_events();

		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 450);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 1000);

		// apply deduction currency to all rewards
		assert_ok!(IncentivesModule::update_claim_reward_deduction_currency(
			RuntimeOrigin::signed(ROOT::get()),
			pool_id,
			None
		));

		// accumulate rewards
		assert_ok!(RewardsModule::accumulate_reward(&pool_id, ACA, 1000));
		assert_ok!(RewardsModule::accumulate_reward(&pool_id, AUSD, 2000));

		// alice claim rewards
		assert_ok!(IncentivesModule::claim_rewards(
			RuntimeOrigin::signed(ALICE::get()),
			pool_id
		));

		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: pool_id,
			reward_currency_id: ACA,
			actual_amount: 473,
			deduction_amount: 52,
		}));
		System::assert_has_event(RuntimeEvent::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: pool_id,
			reward_currency_id: AUSD,
			actual_amount: 900,
			deduction_amount: 100,
		}));

		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 923);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 1900);
	});
}

#[test]
fn nominees_election_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(OnNomineesElectionBonded::<Runtime>::handle(&(ALICE::get(), 80)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::NomineesElection),
			PoolInfo {
				total_shares: 80,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::NomineesElection, ALICE::get()),
			(80, Default::default())
		);

		assert_ok!(OnNomineesElectionUnbonded::<Runtime>::handle(&(ALICE::get(), 20)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::NomineesElection),
			PoolInfo {
				total_shares: 60,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::NomineesElection, ALICE::get()),
			(60, Default::default())
		);

		assert_ok!(OnNomineesElectionUnbonded::<Runtime>::handle(&(ALICE::get(), 60)));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::NomineesElection),
			PoolInfo { ..Default::default() }
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::NomineesElection, ALICE::get()),
			(0, Default::default())
		);
	});
}
