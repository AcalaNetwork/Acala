// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo::default(),
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo::default(),
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			Default::default(),
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
			Default::default(),
		);

		assert_ok!(IncentivesModule::deposit_dex_share(
			Origin::signed(ALICE::get()),
			BTC_AUSD_LP,
			10000
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::DepositDexShare(
			ALICE::get(),
			BTC_AUSD_LP,
			10000,
		)));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 0);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			10000
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(10000, Default::default())
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
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
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 10000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(10000, Default::default())
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
			(10000, Default::default())
		);

		assert_ok!(IncentivesModule::withdraw_dex_share(
			Origin::signed(ALICE::get()),
			BTC_AUSD_LP,
			8000
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::WithdrawDexShare(
			ALICE::get(),
			BTC_AUSD_LP,
			8000,
		)));
		assert_eq!(TokensModule::free_balance(BTC_AUSD_LP, &ALICE::get()), 8000);
		assert_eq!(
			TokensModule::free_balance(BTC_AUSD_LP, &IncentivesModule::account_id()),
			2000
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 2000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 2000,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(2000, Default::default())
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
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

		assert_eq!(IncentivesModule::incentive_reward_amount(PoolId::HomaIncentive), 0);
		assert_eq!(
			IncentivesModule::incentive_reward_amount(PoolId::DexIncentive(DOT_AUSD_LP)),
			0
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amount(PoolId::LoansIncentive(DOT)),
			0
		);

		assert_ok!(IncentivesModule::update_incentive_rewards(
			Origin::signed(Root::get()),
			vec![
				(PoolId::HomaIncentive, 200),
				(PoolId::DexIncentive(DOT_AUSD_LP), 1000),
				(PoolId::LoansIncentive(DOT), 500),
			],
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated(
			PoolId::HomaIncentive,
			200,
		)));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated(
			PoolId::DexIncentive(DOT_AUSD_LP),
			1000,
		)));
		System::assert_has_event(Event::IncentivesModule(crate::Event::IncentiveRewardAmountUpdated(
			PoolId::LoansIncentive(DOT),
			500,
		)));
		assert_eq!(IncentivesModule::incentive_reward_amount(PoolId::HomaIncentive), 200);
		assert_eq!(
			IncentivesModule::incentive_reward_amount(PoolId::DexIncentive(DOT_AUSD_LP)),
			1000
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amount(PoolId::LoansIncentive(DOT)),
			500
		);

		assert_noop!(
			IncentivesModule::update_incentive_rewards(
				Origin::signed(Root::get()),
				vec![(PoolId::DexIncentive(DOT), 800)],
			),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_noop!(
			IncentivesModule::update_incentive_rewards(
				Origin::signed(Root::get()),
				vec![(PoolId::HomaValidatorAllowance(VALIDATOR::get()), 300)],
			),
			Error::<Runtime>::InvalidPoolId
		);
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
				Origin::signed(Root::get()),
				vec![(PoolId::DexIncentive(DOT_AUSD_LP), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(Root::get()),
				vec![(PoolId::DexSaving(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_eq!(
			IncentivesModule::dex_saving_reward_rate(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(Root::get()),
			vec![(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),]
		));
		System::assert_has_event(Event::IncentivesModule(crate::Event::SavingRewardRateUpdated(
			PoolId::DexSaving(DOT_AUSD_LP),
			Rate::saturating_from_rational(1, 100),
		)));
		assert_eq!(
			IncentivesModule::dex_saving_reward_rate(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
	});
}

#[test]
fn update_payout_deduction_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			IncentivesModule::update_payout_deduction_rates(Origin::signed(ALICE::get()), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_payout_deduction_rates(
				Origin::signed(Root::get()),
				vec![(PoolId::DexIncentive(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			IncentivesModule::update_payout_deduction_rates(
				Origin::signed(Root::get()),
				vec![(PoolId::DexSaving(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			IncentivesModule::update_payout_deduction_rates(
				Origin::signed(Root::get()),
				vec![(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(101, 100)),]
			),
			Error::<Runtime>::InvalidRate,
		);

		assert_eq!(
			IncentivesModule::payout_deduction_rates(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_ok!(IncentivesModule::update_payout_deduction_rates(
			Origin::signed(Root::get()),
			vec![(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),]
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::PayoutDeductionRateUpdated(
			PoolId::DexSaving(DOT_AUSD_LP),
			Rate::saturating_from_rational(1, 100),
		)));
		assert_eq!(
			IncentivesModule::payout_deduction_rates(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
	});
}

#[test]
fn add_allowance_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::add_allowance(Origin::signed(ALICE::get()), PoolId::HomaIncentive, 200),
			Error::<Runtime>::InvalidPoolId
		);

		assert_ok!(TokensModule::deposit(LDOT, &ALICE::get(), 10000));
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 10000);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())).rewards,
			Default::default()
		);

		assert_noop!(
			IncentivesModule::add_allowance(
				Origin::signed(ALICE::get()),
				PoolId::HomaValidatorAllowance(VALIDATOR::get()),
				1000
			),
			orml_rewards::Error::<Runtime>::PoolDoesNotExist
		);

		RewardsModule::add_share(&ALICE::get(), &PoolId::HomaValidatorAllowance(VALIDATOR::get()), 100);

		assert_ok!(IncentivesModule::add_allowance(
			Origin::signed(ALICE::get()),
			PoolId::HomaValidatorAllowance(VALIDATOR::get()),
			1000
		));
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 1000);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 9000);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get()))
				.rewards
				.get(&LDOT),
			Some(&(1000, 0))
		);
	});
}

#[test]
fn on_update_loan_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)), PoolInfo::default(),);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE::get()),
			Default::default(),
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB::get()),
			Default::default(),
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), BTC, 100, 0));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 100,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE::get()),
			(100, Default::default())
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB::get(), BTC, 100, 500));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 700,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB::get()),
			(600, Default::default())
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE::get(), BTC, -50, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 650,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE::get()),
			(50, Default::default())
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB::get(), BTC, -650, 600));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 50,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB::get()),
			Default::default(),
		);
	});
}

#[test]
fn guarantee_hooks_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo::default(),
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			Default::default(),
		);

		OnIncreaseGuarantee::<Runtime>::happened(&(ALICE::get(), VALIDATOR::get(), 100));
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo {
				total_shares: 100,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			(100, Default::default())
		);

		OnDecreaseGuarantee::<Runtime>::happened(&(ALICE::get(), VALIDATOR::get(), 10));
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo {
				total_shares: 90,
				..Default::default()
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			(90, Default::default())
		);
	});
}

#[test]
fn payout_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::LoansIncentive(BTC), ALICE::get()),
			BTreeMap::default()
		);

		IncentivesModule::payout(&ALICE::get(), &PoolId::LoansIncentive(BTC), ACA, 1000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::LoansIncentive(BTC), ALICE::get()),
			vec![(ACA, 1000)].into_iter().collect()
		);

		IncentivesModule::payout(&ALICE::get(), &PoolId::LoansIncentive(BTC), ACA, 1000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::LoansIncentive(BTC), ALICE::get()),
			vec![(ACA, 2000)].into_iter().collect()
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
		assert_ok!(IncentivesModule::update_payout_deduction_rates(
			Origin::signed(Root::get()),
			vec![
				(
					PoolId::DexIncentive(BTC_AUSD_LP),
					Rate::saturating_from_rational(50, 100)
				),
				(PoolId::DexSaving(BTC_AUSD_LP), Rate::saturating_from_rational(20, 100)),
				(
					PoolId::HomaValidatorAllowance(VALIDATOR::get()),
					Rate::saturating_from_rational(90, 100)
				),
			]
		));

		// alice add shares before accumulate rewards
		RewardsModule::add_share(&ALICE::get(), &PoolId::LoansIncentive(BTC), 100);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexIncentive(BTC_AUSD_LP), 100);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexSaving(BTC_AUSD_LP), 100);
		RewardsModule::add_share(&ALICE::get(), &PoolId::HomaValidatorAllowance(VALIDATOR::get()), 100);

		// bob add shares before accumulate rewards
		RewardsModule::add_share(&BOB::get(), &PoolId::DexSaving(BTC_AUSD_LP), 100);
		RewardsModule::add_share(&BOB::get(), &PoolId::DexIncentive(BTC_AUSD_LP), 100);

		// accumulate rewards for different pools
		assert_ok!(RewardsModule::accumulate_reward(
			&PoolId::LoansIncentive(BTC),
			ACA,
			2000
		));
		assert_ok!(RewardsModule::accumulate_reward(
			&PoolId::DexIncentive(BTC_AUSD_LP),
			ACA,
			1000
		));
		assert_ok!(RewardsModule::accumulate_reward(
			&PoolId::DexSaving(BTC_AUSD_LP),
			AUSD,
			2000
		));
		assert_ok!(RewardsModule::accumulate_reward(
			&PoolId::HomaValidatorAllowance(VALIDATOR::get()),
			LDOT,
			5000
		));

		// bob add share after accumulate rewards
		RewardsModule::add_share(&BOB::get(), &PoolId::LoansIncentive(BTC), 100);

		// alice claim rewards for PoolId::LoansIncentive(BTC)
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (4000, 2000))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 0);
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(ALICE::get()),
			PoolId::LoansIncentive(BTC)
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::ClaimRewards(
			ALICE::get(),
			PoolId::LoansIncentive(BTC),
			ACA,
			2000,
			0,
		)));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (4000, 4000))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE::get()),
			(100, vec![(ACA, 2000)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 8000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 2000);

		// bob claim rewards for PoolId::LoansIncentive(BTC)
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB::get()),
			(100, vec![(ACA, 2000)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 0);
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(BOB::get()),
			PoolId::LoansIncentive(BTC)
		));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (4000, 4000))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB::get()),
			(100, vec![(ACA, 2000)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 8000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 0);

		// alice remove share for PoolId::DexIncentive(BTC_AUSD_LP) before claim rewards
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(ACA, (1000, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 2000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			BTreeMap::default()
		);
		RewardsModule::remove_share(&ALICE::get(), &PoolId::DexIncentive(BTC_AUSD_LP), 50);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 150,
				rewards: vec![(ACA, (750, 250))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(50, vec![(ACA, 250)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 8000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 2000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			vec![(ACA, 500)].into_iter().collect()
		);

		// bob claim rewards for PoolId::DexIncentive(BTC_AUSD_LP)
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), BOB::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 0);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), BOB::get()),
			BTreeMap::default()
		);
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(BOB::get()),
			PoolId::DexIncentive(BTC_AUSD_LP)
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::ClaimRewards(
			BOB::get(),
			PoolId::DexIncentive(BTC_AUSD_LP),
			ACA,
			250,
			250,
		)));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 150,
				rewards: vec![(ACA, (1000, 750))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), BOB::get()),
			(100, vec![(ACA, 500)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 7750);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 250);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 2000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), BOB::get()),
			BTreeMap::default()
		);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			vec![(ACA, 500)].into_iter().collect()
		);

		// alice claim rewards for PoolId::DexIncentive(BTC_AUSD_LP)
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(ALICE::get()),
			PoolId::DexIncentive(BTC_AUSD_LP)
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::ClaimRewards(
			ALICE::get(),
			PoolId::DexIncentive(BTC_AUSD_LP),
			ACA,
			292,
			291,
		)));
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 150,
				rewards: vec![(ACA, (1291, 833))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			(50, vec![(ACA, 333)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 7458);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE::get()), 2292);
		assert_eq!(TokensModule::free_balance(ACA, &BOB::get()), 250);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), ALICE::get()),
			BTreeMap::default()
		);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::DexIncentive(BTC_AUSD_LP), BOB::get()),
			BTreeMap::default()
		);

		// alice claim rewards for PoolId::DexSaving(BTC_AUSD_LP)
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(AUSD, (2000, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 0);
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(ALICE::get()),
			PoolId::DexSaving(BTC_AUSD_LP)
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::ClaimRewards(
			ALICE::get(),
			PoolId::DexSaving(BTC_AUSD_LP),
			AUSD,
			800,
			200,
		)));
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)),
			PoolInfo {
				total_shares: 200,
				rewards: vec![(AUSD, (2200, 1000))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::DexSaving(BTC_AUSD_LP), ALICE::get()),
			(100, vec![(AUSD, 1000)].into_iter().collect())
		);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 9200);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE::get()), 800);

		// alice remove all share for PoolId::HomaValidatorAllowance(VALIDATOR::get())
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo {
				total_shares: 100,
				rewards: vec![(LDOT, (5000, 0))].into_iter().collect(),
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			(100, Default::default())
		);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			BTreeMap::default()
		);
		RewardsModule::remove_share(&ALICE::get(), &PoolId::HomaValidatorAllowance(VALIDATOR::get()), 100);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo::default()
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			Default::default()
		);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			vec![(LDOT, 5000)].into_iter().collect()
		);

		// alice claim rewards for PoolId::HomaValidatorAllowance(VALIDATOR::get())
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 10000);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 0);

		// cannot claim reward becuase deduction will try to accumulate reward back to pool but pool is
		// removed becuase there is no share
		assert_noop!(
			IncentivesModule::claim_rewards(
				Origin::signed(ALICE::get()),
				PoolId::HomaValidatorAllowance(VALIDATOR::get())
			),
			orml_rewards::Error::<Runtime>::PoolDoesNotExist
		);

		// making deducation rate zero will allow claiming reward
		assert_ok!(IncentivesModule::update_payout_deduction_rates(
			Origin::signed(Root::get()),
			vec![(PoolId::HomaValidatorAllowance(VALIDATOR::get()), Rate::zero())]
		));

		// alice claim all reward, no deduction
		assert_ok!(IncentivesModule::claim_rewards(
			Origin::signed(ALICE::get()),
			PoolId::HomaValidatorAllowance(VALIDATOR::get())
		));
		System::assert_last_event(Event::IncentivesModule(crate::Event::ClaimRewards(
			ALICE::get(),
			PoolId::HomaValidatorAllowance(VALIDATOR::get()),
			LDOT,
			5000,
			0,
		)));
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR::get())),
			PoolInfo::default()
		);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT::get()), 5000);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE::get()), 5000);
		assert_eq!(
			IncentivesModule::pending_rewards(PoolId::HomaValidatorAllowance(VALIDATOR::get()), ALICE::get()),
			BTreeMap::default()
		);
	});
}

#[test]
fn on_initialize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(IncentivesModule::update_incentive_rewards(
			Origin::signed(Root::get()),
			vec![
				(PoolId::LoansIncentive(BTC), 1000),
				(PoolId::LoansIncentive(DOT), 2000),
				(PoolId::DexIncentive(BTC_AUSD_LP), 100),
				(PoolId::DexIncentive(DOT_AUSD_LP), 200),
				(PoolId::HomaIncentive, 30),
			],
		));
		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(Root::get()),
			vec![
				(PoolId::DexSaving(BTC_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
			],
		));

		RewardsModule::add_share(&ALICE::get(), &PoolId::LoansIncentive(BTC), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexIncentive(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexIncentive(DOT_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexSaving(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::DexSaving(DOT_AUSD_LP), 1);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)).rewards.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(DOT)).rewards.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP))
				.rewards
				.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP))
				.rewards
				.get(&ACA),
			None
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).rewards.get(&ACA), None);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).rewards.get(&AUSD),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).rewards.get(&AUSD),
			None
		);

		IncentivesModule::on_initialize(9);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 0);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)).rewards.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(DOT)).rewards.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP))
				.rewards
				.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP))
				.rewards
				.get(&ACA),
			None
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).rewards.get(&ACA), None);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).rewards.get(&AUSD),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).rewards.get(&AUSD),
			None
		);

		IncentivesModule::on_initialize(10);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 1300);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 9);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)).rewards.get(&ACA),
			Some(&(1000, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(DOT)).rewards.get(&ACA),
			None
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(100, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(200, 0))
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).rewards.get(&ACA), None);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).rewards.get(&AUSD),
			Some(&(5, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).rewards.get(&AUSD),
			Some(&(4, 0))
		);

		RewardsModule::add_share(&ALICE::get(), &PoolId::LoansIncentive(DOT), 1);
		RewardsModule::add_share(&ALICE::get(), &PoolId::HomaIncentive, 1);
		IncentivesModule::on_initialize(20);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 4630);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 18);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)).rewards.get(&ACA),
			Some(&(2000, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(DOT)).rewards.get(&ACA),
			Some(&(2000, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(200, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(400, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaIncentive).rewards.get(&ACA),
			Some(&(30, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).rewards.get(&AUSD),
			Some(&(10, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).rewards.get(&AUSD),
			Some(&(8, 0))
		);

		mock_shutdown();
		IncentivesModule::on_initialize(30);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT::get()), 4630);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT::get()), 18);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)).rewards.get(&ACA),
			Some(&(2000, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(DOT)).rewards.get(&ACA),
			Some(&(2000, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(200, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP))
				.rewards
				.get(&ACA),
			Some(&(400, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaIncentive).rewards.get(&ACA),
			Some(&(30, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).rewards.get(&AUSD),
			Some(&(10, 0))
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).rewards.get(&AUSD),
			Some(&(8, 0))
		);
	});
}
