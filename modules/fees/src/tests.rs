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

//! Unit tests for fee distribution module.

#![cfg(test)]

use super::*;
use crate::mock::*;
use frame_support::traits::{ExistenceRequirement, WithdrawReasons};
use frame_support::{assert_noop, assert_ok};
use mock::{Event, ExtBuilder, Origin, Runtime, System};
use primitives::{AccountId, PoolPercent};
use support::Rate;

#[test]
fn set_income_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, vec![]),
			Error::<Runtime>::InvalidParams,
		);

		let pools =
			build_pool_percents::<AccountId>(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 30)]);
		assert_ok!(Fees::set_income_fee(
			Origin::signed(ALICE),
			IncomeSource::TxFee,
			pools.clone()
		));
		let incomes = IncomeToTreasuries::<Runtime>::get(IncomeSource::TxFee);
		assert_eq!(incomes.len(), 2);
		System::assert_last_event(Event::Fees(crate::Event::IncomeFeeSet {
			income: IncomeSource::TxFee,
			pools,
		}));
	});
}

#[test]
fn set_treasury_pool_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, ACA, 10000)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Fees::set_treasury_pool(Origin::signed(ALICE), NetworkTreasuryPool::get(), 100, vec![]),
				Error::<Runtime>::InvalidParams,
			);

			let pools = build_pool_percents::<AccountId>(vec![
				(StakingRewardPool::get(), 70),
				(CollatorsRewardPool::get(), 30),
			]);
			assert_ok!(Fees::set_treasury_pool(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get(),
				100,
				pools.clone()
			));
			let (threshold, incentives) = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 2);
			assert_eq!(threshold, 100);
			System::assert_last_event(Event::Fees(crate::Event::TreasuryPoolSet {
				treasury: NetworkTreasuryPool::get(),
				pools: pools.clone(),
			}));

			assert_ok!(Fees::set_treasury_pool(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get(),
				10,
				pools.clone()
			));
			let (threshold, incentives) = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 2);
			assert_eq!(threshold, 10);
			System::assert_last_event(Event::Fees(crate::Event::TreasuryPoolSet {
				treasury: NetworkTreasuryPool::get(),
				pools,
			}));
		});
}

#[test]
fn invalid_pool_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		let pools1 =
			build_pool_percents::<AccountId>(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 20)]);
		let pools2 =
			build_pool_percents::<AccountId>(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 40)]);
		let pools3 =
			build_pool_percents::<AccountId>(vec![(StakingRewardPool::get(), 70), (CollatorsRewardPool::get(), 20)]);

		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, pools1),
			Error::<Runtime>::InvalidParams
		);
		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, pools2),
			Error::<Runtime>::InvalidParams
		);
		assert_noop!(
			Fees::set_treasury_pool(Origin::signed(ALICE), NetworkTreasuryPool::get(), 100, pools3),
			Error::<Runtime>::InvalidParams
		);
	});
}

#[test]
fn tx_fee_allocation_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, ACA, 10000)])
		.build()
		.execute_with(|| {
			let pool_rates: BoundedVec<PoolPercent<AccountId>, MaxPoolSize> =
				IncomeToTreasuries::<Runtime>::get(IncomeSource::TxFee);
			assert_eq!(2, pool_rates.len());

			assert_eq!(0, Balances::free_balance(&NetworkTreasuryPool::get()));
			assert_eq!(0, Balances::free_balance(&CollatorsRewardPool::get()));

			// Tx fee has two configuration in mock.rs setup.
			let negative_balance = Balances::withdraw(
				&ALICE,
				1000,
				WithdrawReasons::TRANSACTION_PAYMENT,
				ExistenceRequirement::KeepAlive,
			);
			match negative_balance {
				Ok(imbalance) => {
					DistributeTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
					assert_eq!(800, Balances::free_balance(&NetworkTreasuryPool::get()));
					assert_eq!(200, Balances::free_balance(&CollatorsRewardPool::get()));
				}
				Err(_) => {}
			}

			// Update tx fee only to NetworkTreasuryPool account.
			let pools = build_pool_percents::<AccountId>(vec![(NetworkTreasuryPool::get(), 100)]);
			assert_ok!(Fees::set_income_fee(
				Origin::signed(ALICE),
				IncomeSource::TxFee,
				pools.clone()
			));
			let negative_balance = Balances::withdraw(
				&ALICE,
				1000,
				WithdrawReasons::TRANSACTION_PAYMENT,
				ExistenceRequirement::KeepAlive,
			);
			match negative_balance {
				Ok(imbalance) => {
					DistributeTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
					assert_eq!(1800, Balances::free_balance(&NetworkTreasuryPool::get()));
					assert_eq!(200, Balances::free_balance(&CollatorsRewardPool::get()));
				}
				Err(_) => {}
			}

			// Update tx fee to NetworkTreasuryPool and CollatorsRewardPool both 50%.
			let pools = build_pool_percents::<AccountId>(vec![
				(NetworkTreasuryPool::get(), 50),
				(CollatorsRewardPool::get(), 50),
			]);
			assert_ok!(Fees::set_income_fee(
				Origin::signed(ALICE),
				IncomeSource::TxFee,
				pools.clone()
			));
			let negative_balance = Balances::withdraw(
				&ALICE,
				1000,
				WithdrawReasons::TRANSACTION_PAYMENT,
				ExistenceRequirement::KeepAlive,
			);
			match negative_balance {
				Ok(imbalance) => {
					DistributeTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
					assert_eq!(2300, Balances::free_balance(&NetworkTreasuryPool::get()));
					assert_eq!(700, Balances::free_balance(&CollatorsRewardPool::get()));
				}
				Err(_) => {}
			}

			// emit deposit event, just validate for last on_unbalanced() action
			System::assert_has_event(Event::Balances(pallet_balances::Event::Deposit {
				who: NetworkTreasuryPool::get(),
				amount: 500,
			}));
			System::assert_has_event(Event::Balances(pallet_balances::Event::Deposit {
				who: CollatorsRewardPool::get(),
				amount: 500,
			}));
		});
}

#[test]
fn on_fee_deposit_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, ACA, 10000), (ALICE, DOT, 10000)])
		.build()
		.execute_with(|| {
			assert_ok!(Fees::do_set_treasury_rate(
				IncomeSource::TxFee,
				vec![
					PoolPercent {
						pool: NetworkTreasuryPool::get(),
						rate: Rate::saturating_from_rational(8, 10)
					},
					PoolPercent {
						pool: CollatorsRewardPool::get(),
						rate: Rate::saturating_from_rational(2, 10)
					},
				]
			));
			// Native token tests
			// FeeToTreasuryPool based on pre-configured treasury pool percentage.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::TxFee, ACA, 10000));

			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 8000);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 2000);
			System::assert_has_event(Event::Balances(pallet_balances::Event::Deposit {
				who: NetworkTreasuryPool::get(),
				amount: 8000,
			}));
			System::assert_has_event(Event::Balances(pallet_balances::Event::Deposit {
				who: CollatorsRewardPool::get(),
				amount: 2000,
			}));
			assert_eq!(
				crate::TreasuryTokens::<Runtime>::get(&NetworkTreasuryPool::get()).to_vec(),
				vec![ACA]
			);
			assert_eq!(
				crate::TreasuryTokens::<Runtime>::get(&CollatorsRewardPool::get()).to_vec(),
				vec![ACA]
			);

			// Non native token tests
			// FeeToTreasuryPool based on pre-configured treasury pool percentage.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::TxFee, DOT, 10000));

			assert_eq!(Currencies::free_balance(DOT, &NetworkTreasuryPool::get()), 8000);
			assert_eq!(Currencies::free_balance(DOT, &CollatorsRewardPool::get()), 2000);
			System::assert_has_event(Event::Tokens(orml_tokens::Event::Deposited {
				currency_id: DOT,
				who: NetworkTreasuryPool::get(),
				amount: 8000,
			}));
			System::assert_has_event(Event::Tokens(orml_tokens::Event::Deposited {
				currency_id: DOT,
				who: CollatorsRewardPool::get(),
				amount: 2000,
			}));
			assert_eq!(
				crate::TreasuryTokens::<Runtime>::get(&NetworkTreasuryPool::get()).to_vec(),
				vec![ACA, DOT]
			);
			assert_eq!(
				crate::TreasuryTokens::<Runtime>::get(&CollatorsRewardPool::get()).to_vec(),
				vec![ACA, DOT]
			);
		});
}

#[test]
fn force_transfer_to_incentive_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, ACA, 100000), (ALICE, AUSD, 10000)])
		.build()
		.execute_with(|| {
			let pool_rates = IncomeToTreasuries::<Runtime>::get(IncomeSource::TxFee);

			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates.clone(), ACA, 1000,));
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 800);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 200);

			assert_ok!(Pallet::<Runtime>::force_transfer_to_incentive(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get()
			));
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 0);
			assert_eq!(Currencies::free_balance(ACA, &StakingRewardPool::get()), 640);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 280);
			assert_eq!(Currencies::free_balance(ACA, &EcosystemRewardPool::get()), 80);
			System::assert_has_event(Event::Fees(crate::Event::IncentiveDistribution {
				treasury: NetworkTreasuryPool::get(),
				amount: 800,
			}));

			assert_ok!(DEX::add_liquidity(
				Origin::signed(ALICE),
				ACA,
				AUSD,
				10000,
				1000,
				0,
				false
			));

			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates, AUSD, 100));
			assert_eq!(Currencies::free_balance(AUSD, &NetworkTreasuryPool::get()), 80);
			assert_eq!(Currencies::free_balance(AUSD, &CollatorsRewardPool::get()), 20);

			assert_ok!(Pallet::<Runtime>::force_transfer_to_incentive(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get()
			));
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 0);
			assert_eq!(Currencies::free_balance(AUSD, &NetworkTreasuryPool::get()), 0);
			assert_eq!(Currencies::free_balance(ACA, &StakingRewardPool::get()), 640 + 592);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 280 + 74);
			assert_eq!(Currencies::free_balance(ACA, &EcosystemRewardPool::get()), 80 + 74);
			System::assert_has_event(Event::DEX(module_dex::Event::Swap {
				trader: NetworkTreasuryPool::get(),
				path: vec![AUSD, ACA],
				liquidity_changes: vec![80, 740],
			}));
			System::assert_has_event(Event::Fees(crate::Event::IncentiveDistribution {
				treasury: NetworkTreasuryPool::get(),
				amount: 740,
			}));
		});
}

#[test]
fn distribution_incentive_threshold_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, ACA, 100000), (ALICE, AUSD, 10000)])
		.build()
		.execute_with(|| {
			let pool_rates = IncomeToTreasuries::<Runtime>::get(IncomeSource::TxFee);

			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates.clone(), ACA, 100));
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 80);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 20);

			assert_ok!(Pallet::<Runtime>::distribution_incentive(NetworkTreasuryPool::get()));
			// due to native token less than threshold, not distribute to incentive pools.
			// but swap still happened, so treasury account got native token.
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 80);
			assert_eq!(Currencies::free_balance(ACA, &StakingRewardPool::get()), 0);
			assert_eq!(Currencies::free_balance(ACA, &EcosystemRewardPool::get()), 0);

			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates.clone(), ACA, 25));
			// now treasury account native token large than threshold
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 100);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 25);

			assert_ok!(Pallet::<Runtime>::distribution_incentive(NetworkTreasuryPool::get()));
			// then distribution to incentive pools
			assert_eq!(Currencies::free_balance(ACA, &StakingRewardPool::get()), 80);
			assert_eq!(Currencies::free_balance(ACA, &EcosystemRewardPool::get()), 10);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 25 + 10);
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 0);
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 0);
			System::assert_has_event(Event::Fees(crate::Event::IncentiveDistribution {
				treasury: NetworkTreasuryPool::get(),
				amount: 100,
			}));

			assert_ok!(DEX::add_liquidity(
				Origin::signed(ALICE),
				ACA,
				AUSD,
				10000,
				1000,
				0,
				false
			));

			// swapped out native token less then threshold
			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates.clone(), AUSD, 10));
			assert_eq!(Currencies::free_balance(AUSD, &NetworkTreasuryPool::get()), 8);
			assert_eq!(Currencies::free_balance(AUSD, &CollatorsRewardPool::get()), 2);

			assert_ok!(Pallet::<Runtime>::distribution_incentive(NetworkTreasuryPool::get()));
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 79);
			System::assert_has_event(Event::DEX(module_dex::Event::Swap {
				trader: NetworkTreasuryPool::get(),
				path: vec![AUSD, ACA],
				liquidity_changes: vec![8, 79],
			}));

			assert_ok!(Pallet::<Runtime>::distribution_fees(pool_rates, AUSD, 10));
			assert_eq!(Currencies::free_balance(AUSD, &NetworkTreasuryPool::get()), 8);
			assert_eq!(Currencies::free_balance(AUSD, &CollatorsRewardPool::get()), 2 + 2);

			assert_ok!(Pallet::<Runtime>::distribution_incentive(NetworkTreasuryPool::get()));
			assert_eq!(Currencies::free_balance(ACA, &StakingRewardPool::get()), 80 + 125);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 35 + 15);
			assert_eq!(Currencies::free_balance(ACA, &EcosystemRewardPool::get()), 10 + 15);
			// due to percent round, there are some native token left in treasury account.
			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 2);
			assert_eq!(Currencies::free_balance(AUSD, &NetworkTreasuryPool::get()), 0);
			System::assert_has_event(Event::DEX(module_dex::Event::Swap {
				trader: NetworkTreasuryPool::get(),
				path: vec![AUSD, ACA],
				liquidity_changes: vec![8, 78],
			}));
			System::assert_has_event(Event::Fees(crate::Event::IncentiveDistribution {
				treasury: NetworkTreasuryPool::get(),
				amount: 79 + 78,
			}));
		});
}

#[test]
fn independent_pools_on_fee_deposit_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Register payout destination for multiple pools
		assert_ok!(Fees::do_set_treasury_rate(
			IncomeSource::TxFee,
			vec![PoolPercent {
				pool: ALICE,
				rate: Rate::one()
			},]
		));
		assert_ok!(Fees::do_set_treasury_rate(
			IncomeSource::XcmFee,
			vec![PoolPercent {
				pool: BOB,
				rate: Rate::one()
			},]
		));
		assert_ok!(Fees::do_set_treasury_rate(
			IncomeSource::HonzonStabilityFee,
			vec![PoolPercent {
				pool: CHARLIE,
				rate: Rate::one()
			},]
		));

		assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::TxFee, ACA, 1000));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(ACA, &BOB), 0);
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 0);

		assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::XcmFee, ACA, 1000));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(ACA, &BOB), 1000);
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 0);

		assert_ok!(Pallet::<Runtime>::on_fee_deposit(
			IncomeSource::HonzonStabilityFee,
			ACA,
			1000
		));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(ACA, &BOB), 1000);
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 1000);
	});
}
