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
use primitives::AccountId;
use sp_runtime::FixedU128;

fn build_pool_percents(list: Vec<(AccountId, u32)>) -> Vec<PoolPercent<AccountId>> {
	list.iter()
		.map(|data| PoolPercent {
			pool: data.clone().0,
			rate: FixedU128::saturating_from_rational(data.clone().1, 100),
		})
		.collect()
}

#[test]
fn set_income_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, vec![]),
			Error::<Runtime>::InvalidParams,
		);

		let pools = build_pool_percents(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 30)]);
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
			let incentives = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 0);

			assert_noop!(
				Fees::set_treasury_pool(Origin::signed(ALICE), NetworkTreasuryPool::get(), vec![]),
				Error::<Runtime>::InvalidParams,
			);

			let pools = build_pool_percents(vec![(StakingRewardPool::get(), 70), (CollatorsRewardPool::get(), 30)]);
			assert_ok!(Fees::set_treasury_pool(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get(),
				pools.clone()
			));
			let incentives = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 2);
			System::assert_last_event(Event::Fees(crate::Event::TreasuryPoolSet {
				treasury: NetworkTreasuryPool::get(),
				pools,
			}));
		});
}

#[test]
fn invalid_pool_rates_works() {
	ExtBuilder::default().build().execute_with(|| {
		let pools1 = build_pool_percents(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 20)]);
		let pools2 = build_pool_percents(vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 40)]);
		let pools3 = build_pool_percents(vec![(StakingRewardPool::get(), 70), (CollatorsRewardPool::get(), 20)]);

		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, pools1),
			Error::<Runtime>::InvalidParams
		);
		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, pools2),
			Error::<Runtime>::InvalidParams
		);
		assert_noop!(
			Fees::set_treasury_pool(Origin::signed(ALICE), NetworkTreasuryPool::get(), pools3),
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
					DealWithTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
					assert_eq!(800, Balances::free_balance(&NetworkTreasuryPool::get()));
					assert_eq!(200, Balances::free_balance(&CollatorsRewardPool::get()));
				}
				Err(_) => {}
			}

			// Update tx fee only to NetworkTreasuryPool account.
			let pools = build_pool_percents(vec![(NetworkTreasuryPool::get(), 100)]);
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
					DealWithTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
					assert_eq!(1800, Balances::free_balance(&NetworkTreasuryPool::get()));
					assert_eq!(200, Balances::free_balance(&CollatorsRewardPool::get()));
				}
				Err(_) => {}
			}

			// Update tx fee to NetworkTreasuryPool and CollatorsRewardPool both 50%.
			let pools = build_pool_percents(vec![(NetworkTreasuryPool::get(), 50), (CollatorsRewardPool::get(), 50)]);
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
					DealWithTxFees::<Runtime>::on_unbalanceds(Some(imbalance).into_iter());
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
			// Native token tests
			// FeeToTreasuryPool based on pre-configured treasury pool percentage.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::TxFee, None, ACA, 10000));

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

			// FeeToTreasuryPool direct to given account.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(
				IncomeSource::TxFee,
				Some(&TreasuryAccount::get()),
				ACA,
				10000
			));
			assert_eq!(Currencies::free_balance(ACA, &TreasuryAccount::get()), 10000);
			System::assert_has_event(Event::Balances(pallet_balances::Event::Deposit {
				who: TreasuryAccount::get(),
				amount: 10000,
			}));

			// Non native token tests
			// FeeToTreasuryPool based on pre-configured treasury pool percentage.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(IncomeSource::TxFee, None, DOT, 10000));

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

			// FeeToTreasuryPool direct to given account.
			assert_ok!(Pallet::<Runtime>::on_fee_deposit(
				IncomeSource::TxFee,
				Some(&TreasuryAccount::get()),
				DOT,
				10000
			));
			assert_eq!(Currencies::free_balance(DOT, &TreasuryAccount::get()), 10000);
			System::assert_has_event(Event::Tokens(orml_tokens::Event::Deposited {
				currency_id: DOT,
				who: TreasuryAccount::get(),
				amount: 10000,
			}));
		});
}
