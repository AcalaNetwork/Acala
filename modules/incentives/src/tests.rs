// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
use mock::{Event, *};
use orml_rewards::PoolInfo;
use orml_traits::MultiCurrency;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn deposit_dex_share_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
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
			Origin::signed(ALICE::get()),
			BTC_AUSD_LP,
			10000
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::DepositDexShare {
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
		System::set_block_number(1);
		assert_ok!(TokensModule::deposit(BTC_AUSD_LP, &ALICE::get(), 10000));

		assert_noop!(
			IncentivesModule::withdraw_dex_share(Origin::signed(BOB::get()), BTC_AUSD_LP, 10000),
			Error::<Runtime>::NotEnough,
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			Origin::signed(ALICE::get()),
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
			Origin::signed(ALICE::get()),
			BTC_AUSD_LP,
			8000
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::WithdrawDexShare {
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
		System::set_block_number(1);
		assert_noop!(
			IncentivesModule::update_incentive_rewards(Origin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_incentive_rewards(Origin::signed(ROOT::get()), vec![(PoolId::Dex(DOT), vec![])]),
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
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 1000), (DOT, 100)]),
				(PoolId::Loans(DOT), vec![(ACA, 500)]),
			],
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_currency_id: ACA,
			reward_amount_per_period: 1000,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_currency_id: DOT,
			reward_amount_per_period: 100,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated {
			pool: PoolId::Loans(DOT),
			reward_currency_id: ACA,
			reward_amount_per_period: 500,
		}));
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
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 200), (DOT, 0)]),
				(PoolId::Loans(DOT), vec![(ACA, 500)]),
			],
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_currency_id: ACA,
			reward_amount_per_period: 200,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_currency_id: DOT,
			reward_amount_per_period: 0,
		}));
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
fn update_dex_saving_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(Origin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(ROOT::get()),
				vec![(PoolId::Loans(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(101, 100))]
			),
			Error::<Runtime>::InvalidRate
		);

		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);

		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(2, 100))
			]
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::SavingRewardRateUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_rate_per_period: Rate::saturating_from_rational(1, 100),
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::SavingRewardRateUpdated {
			pool: PoolId::Dex(BTC_AUSD_LP),
			reward_rate_per_period: Rate::saturating_from_rational(2, 100),
		}));
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			true
		);
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::saturating_from_rational(2, 100)
		);

		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(5, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::zero())
			]
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::SavingRewardRateUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			reward_rate_per_period: Rate::saturating_from_rational(5, 100),
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::SavingRewardRateUpdated {
			pool: PoolId::Dex(BTC_AUSD_LP),
			reward_rate_per_period: Rate::zero(),
		}));
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(5, 100)
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);
	});
}

#[test]
fn update_claim_reward_deduction_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(Origin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(
				Origin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_claim_reward_deduction_rates(
				Origin::signed(ROOT::get()),
				vec![(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(101, 100)),]
			),
			Error::<Runtime>::InvalidRate,
		);

		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(2, 100))
			]
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewardDeductionRateUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			deduction_rate: Rate::saturating_from_rational(1, 100),
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewardDeductionRateUpdated {
			pool: PoolId::Dex(BTC_AUSD_LP),
			deduction_rate: Rate::saturating_from_rational(2, 100),
		}));
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			true
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::saturating_from_rational(2, 100)
		);

		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(5, 100)),
				(PoolId::Dex(BTC_AUSD_LP), Rate::zero())
			]
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewardDeductionRateUpdated {
			pool: PoolId::Dex(DOT_AUSD_LP),
			deduction_rate: Rate::saturating_from_rational(5, 100),
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewardDeductionRateUpdated {
			pool: PoolId::Dex(BTC_AUSD_LP),
			deduction_rate: Rate::zero(),
		}));
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(5, 100)
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(BTC_AUSD_LP)),
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

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), BTC, 100, 0));
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

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), BTC, 100, 100));
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

		OnUpdateLoan::<Runtime>::happened(&(BOB::get(), BTC, 600, 0));
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

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), BTC, -50, 200));
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

		OnUpdateLoan::<Runtime>::happened(&(BOB::get(), BTC, -600, 600));
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
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 100));
		RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 100);
		RewardsModule::add_share(&BOB::get(), &PoolId::Loans(BTC), 100);
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), AUSD, 18));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 100);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(AUSD, (18, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, Default::default())
		);

		// Alice claim rewards, but the rewards are put back to pool because transfer rewards failed.
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(ALICE::get()),
			PoolId::Loans(BTC)
		));

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 100);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(AUSD, (27, 9))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), ALICE::get()),
			(100, vec![(AUSD, 9)].into_iter().collect())
		);

		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 0);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(100, Default::default())
		);

		// BOB claim reward and receive the reward.
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(BOB::get()),
			PoolId::Loans(BTC)
		));
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 87);
		assert_eq!(TokensModule::free_balance(AUSD, &BOB::get()), 13);
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(AUSD, (27, 22))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(BTC), BOB::get()),
			(100, vec![(AUSD, 13)].into_iter().collect())
		);
	});
}

#[test]
fn claim_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(TokensModule::deposit(ACA, &VAULT::get(), 10000));
		assert_ok!(TokensModule::deposit(AUSD, &VAULT::get(), 10000));
		assert_ok!(TokensModule::deposit(LDOT, &VAULT::get(), 10000));
		assert_ok!(IncentivesModule::update_claim_reward_deduction_rates(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(50, 100)),
				(PoolId::Loans(BTC), Rate::saturating_from_rational(90, 100)),
			]
		));

		// alice add shares before accumulate rewards
		RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 100);
		RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(BTC_AUSD_LP), 100);

		// bob add shares before accumulate rewards
		RewardsModule::add_share(&BOB::get(), &PoolId::Dex(BTC_AUSD_LP), 100);

		// accumulate rewards for different pools
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Loans(BTC), ACA, 2000));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Dex(BTC_AUSD_LP), ACA, 1000));
		assert_ok!(RewardsModule::accumulate_reward(&PoolId::Dex(BTC_AUSD_LP), AUSD, 2000));

		// bob add share after accumulate rewards
		RewardsModule::add_share(&BOB::get(), &PoolId::Loans(BTC), 100);

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
			Origin::signed(ALICE::get()),
			PoolId::Loans(BTC)
		));

		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: ACA,
			actual_amount: 200,
			deduction_amount: 1800,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
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
			Origin::signed(BOB::get()),
			PoolId::Loans(BTC)
		));

		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
			who: BOB::get(),
			pool: PoolId::Loans(BTC),
			reward_currency_id: ACA,
			actual_amount: 90,
			deduction_amount: 810,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
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
		RewardsModule::remove_share(&ALICE::get(), &PoolId::Dex(BTC_AUSD_LP), 50);
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
			Origin::signed(ALICE::get()),
			PoolId::Dex(BTC_AUSD_LP)
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
			who: ALICE::get(),
			pool: PoolId::Dex(BTC_AUSD_LP),
			reward_currency_id: ACA,
			actual_amount: 250,
			deduction_amount: 250,
		}));
		System::assert_has_event(Event::IncentivesModule(crate::Event::ClaimRewards {
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
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Loans(BTC), vec![(ACA, 1000), (AUSD, 500)]),
				(PoolId::Loans(DOT), vec![(ACA, 2000), (LDOT, 50)]),
				(PoolId::Dex(BTC_AUSD_LP), vec![(ACA, 100)]),
				(PoolId::Dex(DOT_AUSD_LP), vec![(ACA, 200)]),
			],
		));
		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(ROOT::get()),
			vec![
				(PoolId::Dex(BTC_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::Dex(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
			],
		));

		RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(BTC), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::Dex(DOT_AUSD_LP), 1);

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

		// per 10 blocks will accumulate rewards, nothing happened when on_initialize(9)
		IncentivesModule::on_initialize(9);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0);

		IncentivesModule::on_initialize(10);
		assert_eq!(
			TokensModule::free_balance(ACA, &RewardsSource::get()),
			10000 - (1000 + 200 + 100)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 10000 - 500);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 1000 + 200 + 100);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 500 + (5 + 4)); // (5 + 4) from debit_issue,  500 from RewardsSource
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
		// 100 ACA is incentive reward, 5 AUSD is dex saving reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (100, 0)), (AUSD, (5, 0))].into_iter().collect(),
			}
		);
		// 200 ACA is incentive reward, 4 AUSD is dex saving reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (200, 0)), (AUSD, (4, 0))].into_iter().collect(),
			}
		);

		// add share for PoolId::Loans(DOT)
		RewardsModule::add_share(&ALICE::get(), &PoolId::Loans(DOT), 1);
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
			8700 - (1000 + 2000 + 100 + 200)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 9500 - 500);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 10000 - 50);
		assert_eq!(
			TokensModule::free_balance(ACA, &VAULT::get()),
			1300 + (1000 + 2000 + 100 + 200)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 509 + (500 + 9)); // 9 from debit_issue,  500 from RewardsSource
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
		// 100 ACA is incentive reward, 5 AUSD is dex saving reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (200, 0)), (AUSD, (10, 0))].into_iter().collect(),
			}
		);
		// 200 ACA is incentive reward, 4 AUSD is dex saving reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (400, 0)), (AUSD, (8, 0))].into_iter().collect(),
			}
		);

		mock_shutdown();
		IncentivesModule::on_initialize(30);
		assert_eq!(
			TokensModule::free_balance(ACA, &RewardsSource::get()),
			5400 - (100 + 200)
		);
		assert_eq!(TokensModule::free_balance(AUSD, &RewardsSource::get()), 9000);
		assert_eq!(TokensModule::free_balance(LDOT, &RewardsSource::get()), 9950);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 4600 + (100 + 200));
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 1018);
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
		// after shutdown, PoolId::Dex will accumulate incentive rewards, but will not accumulate dex saving
		// reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (300, 0)), (AUSD, (10, 0))].into_iter().collect(),
			}
		);
		// after shutdown, PoolId::Dex will accumulate incentive rewards, but will not accumulate dex saving
		// reward
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Dex(DOT_AUSD_LP)),
			PoolInfo {
				total_shares: 1,
				rewards: vec![(ACA, (600, 0)), (AUSD, (8, 0))].into_iter().collect(),
			}
		);
	});
}

#[test]
fn earning_booster_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), ACA, 100, 0));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(ACA)),
			PoolInfo {
				total_shares: 100,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(ACA), ALICE::get()),
			(100, Default::default())
		);

		OnEarningBonded::<Runtime>::happened(&(ALICE::get(), 80));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(ACA)),
			PoolInfo {
				total_shares: 100 + 80 + 40,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(ACA), ALICE::get()),
			(100 + 80 + 40, Default::default())
		);

		OnEarningUnbonded::<Runtime>::happened(&(ALICE::get(), 20));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(ACA)),
			PoolInfo {
				total_shares: 100 + 60 + 30,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(ACA), ALICE::get()),
			(100 + 60 + 30, Default::default())
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), ACA, -100, 100));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(ACA)),
			PoolInfo {
				total_shares: 60 + 30,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(ACA), ALICE::get()),
			(60 + 30, Default::default())
		);

		OnEarningUnbonded::<Runtime>::happened(&(ALICE::get(), 60));
		assert_eq!(
			RewardsModule::pool_infos(PoolId::Loans(ACA)),
			PoolInfo { ..Default::default() }
		);
		assert_eq!(
			RewardsModule::shares_and_withdrawn_rewards(PoolId::Loans(ACA), ALICE::get()),
			(0, Default::default())
		);
	});
}
