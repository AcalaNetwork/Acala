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
		System::assert_last_event(Event::IncentivesModule(crate::Event::DepositDexShare(
			ALICE,
			BTC_AUSD_LP,
			10000,
		)));
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
		System::assert_last_event(Event::IncentivesModule(crate::Event::WithdrawDexShare(
			ALICE,
			BTC_AUSD_LP,
			8000,
		)));
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
fn update_incentive_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_incentive_rewards(Origin::signed(ALICE), vec![]),
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
			Origin::signed(4),
			vec![
				(PoolId::HomaIncentive, 200),
				(PoolId::DexIncentive(DOT_AUSD_LP), 1000),
				(PoolId::LoansIncentive(DOT), 500),
			],
		));
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
			IncentivesModule::update_incentive_rewards(Origin::signed(4), vec![(PoolId::DexIncentive(DOT), 800)],),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_noop!(
			IncentivesModule::update_incentive_rewards(
				Origin::signed(4),
				vec![(PoolId::HomaValidatorAllowance(VALIDATOR), 300)],
			),
			Error::<Runtime>::InvalidPoolId
		);
	});
}

#[test]
fn update_dex_saving_rewards_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(Origin::signed(ALICE), vec![]),
			BadOrigin
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(4),
				vec![(PoolId::DexIncentive(DOT_AUSD_LP), Rate::zero())]
			),
			Error::<Runtime>::InvalidPoolId
		);
		assert_noop!(
			IncentivesModule::update_dex_saving_rewards(
				Origin::signed(4),
				vec![(PoolId::DexSaving(DOT), Rate::zero())]
			),
			Error::<Runtime>::InvalidCurrencyId
		);

		assert_eq!(
			IncentivesModule::dex_saving_reward_rate(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::zero()
		);
		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(4),
			vec![(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),]
		));
		assert_eq!(
			IncentivesModule::dex_saving_reward_rate(PoolId::DexSaving(DOT_AUSD_LP)),
			Rate::saturating_from_rational(1, 100)
		);
	});
}

#[test]
fn add_allowance_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			IncentivesModule::add_allowance(Origin::signed(ALICE), PoolId::HomaIncentive, 200),
			Error::<Runtime>::InvalidPoolId
		);

		assert_ok!(TokensModule::deposit(LDOT, &ALICE, 10000));
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT), 0);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE), 10000);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR)).total_rewards,
			0
		);

		assert_ok!(IncentivesModule::add_allowance(
			Origin::signed(ALICE),
			PoolId::HomaValidatorAllowance(VALIDATOR),
			1000
		));
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT), 1000);
		assert_eq!(TokensModule::free_balance(LDOT, &ALICE), 9000);
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR)).total_rewards,
			1000
		);
	});
}

#[test]
fn on_update_loan_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE),
			(0, 0)
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB),
			(0, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE, BTC, 100, 0));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 100,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE),
			(100, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB, BTC, 100, 500));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 700,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB),
			(600, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(ALICE, BTC, -50, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 650,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), ALICE),
			(50, 0)
		);

		OnUpdateLoan::<Runtime>::happened(&(BOB, BTC, -650, 600));
		assert_eq!(
			RewardsModule::pools(PoolId::LoansIncentive(BTC)),
			PoolInfo {
				total_shares: 50,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::LoansIncentive(BTC), BOB),
			(0, 0)
		);
	});
}

#[test]
fn guarantee_hooks_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR)),
			PoolInfo {
				total_shares: 0,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR), ALICE),
			(0, 0)
		);

		OnIncreaseGuarantee::<Runtime>::happened(&(ALICE, VALIDATOR, 100));
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR)),
			PoolInfo {
				total_shares: 100,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR), ALICE),
			(100, 0)
		);

		OnDecreaseGuarantee::<Runtime>::happened(&(ALICE, VALIDATOR, 10));
		assert_eq!(
			RewardsModule::pools(PoolId::HomaValidatorAllowance(VALIDATOR)),
			PoolInfo {
				total_shares: 90,
				total_rewards: 0,
				total_withdrawn_rewards: 0
			}
		);
		assert_eq!(
			RewardsModule::share_and_withdrawn_reward(PoolId::HomaValidatorAllowance(VALIDATOR), ALICE),
			(90, 0)
		);
	});
}

#[test]
fn pay_out_works_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(TokensModule::deposit(ACA, &VAULT, 10000));
		assert_ok!(TokensModule::deposit(AUSD, &VAULT, 10000));
		assert_ok!(TokensModule::deposit(LDOT, &VAULT, 10000));

		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 10000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE), 0);
		IncentivesModule::payout(&ALICE, &PoolId::LoansIncentive(BTC), 1000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 9000);
		assert_eq!(TokensModule::free_balance(ACA, &ALICE), 1000);

		assert_eq!(TokensModule::free_balance(ACA, &BOB), 0);
		IncentivesModule::payout(&BOB, &PoolId::DexIncentive(DOT_AUSD_LP), 1000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 8000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 1000);

		IncentivesModule::payout(&BOB, &PoolId::HomaIncentive, 2000);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 6000);
		assert_eq!(TokensModule::free_balance(ACA, &BOB), 3000);

		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 10000);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE), 0);
		IncentivesModule::payout(&ALICE, &PoolId::DexSaving(DOT_AUSD_LP), 1000);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 9000);
		assert_eq!(TokensModule::free_balance(AUSD, &ALICE), 1000);

		assert_eq!(TokensModule::free_balance(LDOT, &VAULT), 10000);
		assert_eq!(TokensModule::free_balance(LDOT, &BOB), 0);
		IncentivesModule::payout(&BOB, &PoolId::HomaValidatorAllowance(VALIDATOR), 3000);
		assert_eq!(TokensModule::free_balance(LDOT, &VAULT), 7000);
		assert_eq!(TokensModule::free_balance(LDOT, &BOB), 3000);
	});
}

#[test]
fn on_initialize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(IncentivesModule::update_incentive_rewards(
			Origin::signed(4),
			vec![
				(PoolId::LoansIncentive(BTC), 1000),
				(PoolId::LoansIncentive(DOT), 2000),
				(PoolId::DexIncentive(BTC_AUSD_LP), 100),
				(PoolId::DexIncentive(DOT_AUSD_LP), 200),
				(PoolId::HomaIncentive, 30),
			],
		));
		assert_ok!(IncentivesModule::update_dex_saving_rewards(
			Origin::signed(4),
			vec![
				(PoolId::DexSaving(BTC_AUSD_LP), Rate::saturating_from_rational(1, 100)),
				(PoolId::DexSaving(DOT_AUSD_LP), Rate::saturating_from_rational(1, 100)),
			],
		));

		RewardsModule::add_share(&ALICE, &PoolId::LoansIncentive(BTC), 1);
		RewardsModule::add_share(&ALICE, &PoolId::DexIncentive(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE, &PoolId::DexIncentive(DOT_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE, &PoolId::DexSaving(BTC_AUSD_LP), 1);
		RewardsModule::add_share(&ALICE, &PoolId::DexSaving(DOT_AUSD_LP), 1);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 0);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(DOT)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).total_rewards, 0);

		IncentivesModule::on_initialize(9);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 0);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 0);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(DOT)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).total_rewards, 0);

		IncentivesModule::on_initialize(10);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 1300);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 9);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)).total_rewards, 1000);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(DOT)).total_rewards, 0);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)).total_rewards,
			100
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP)).total_rewards,
			200
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).total_rewards, 0);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).total_rewards, 5);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).total_rewards, 4);

		RewardsModule::add_share(&ALICE, &PoolId::LoansIncentive(DOT), 1);
		RewardsModule::add_share(&ALICE, &PoolId::HomaIncentive, 1);
		IncentivesModule::on_initialize(20);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 4630);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 18);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)).total_rewards, 2000);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(DOT)).total_rewards, 2000);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)).total_rewards,
			200
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP)).total_rewards,
			400
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).total_rewards, 30);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).total_rewards, 10);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).total_rewards, 8);

		mock_shutdown();
		IncentivesModule::on_initialize(30);
		assert_eq!(TokensModule::free_balance(ACA, &VAULT), 4630);
		assert_eq!(TokensModule::free_balance(AUSD, &VAULT), 18);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(BTC)).total_rewards, 2000);
		assert_eq!(RewardsModule::pools(PoolId::LoansIncentive(DOT)).total_rewards, 2000);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(BTC_AUSD_LP)).total_rewards,
			200
		);
		assert_eq!(
			RewardsModule::pools(PoolId::DexIncentive(DOT_AUSD_LP)).total_rewards,
			400
		);
		assert_eq!(RewardsModule::pools(PoolId::HomaIncentive).total_rewards, 30);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(BTC_AUSD_LP)).total_rewards, 10);
		assert_eq!(RewardsModule::pools(PoolId::DexSaving(DOT_AUSD_LP)).total_rewards, 8);
	});
}
