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
use frame_support::{assert_noop, assert_ok};
use mock::{Event, ExtBuilder, Origin, Runtime, System};

#[test]
fn set_income_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Fees::set_income_fee(Origin::signed(ALICE), IncomeSource::TxFee, vec![]),
			Error::<Runtime>::InvalidParams,
		);

		assert_ok!(Fees::set_income_fee(
			Origin::signed(ALICE),
			IncomeSource::TxFee,
			vec![(NetworkTreasuryPool::get(), 70), (HonzonTreasuryPool::get(), 30)]
		));
		let incomes = IncomeToTreasuries::<Runtime>::get(IncomeSource::TxFee);
		assert_eq!(incomes.len(), 2);
		System::assert_last_event(Event::Fees(crate::Event::IncomeFeeSet {
			income: IncomeSource::TxFee,
			pools: vec![
				PoolPercent {
					pool: NetworkTreasuryPool::get(),
					rate: Rate::saturating_from_rational(70, 100),
				},
				PoolPercent {
					pool: HonzonTreasuryPool::get(),
					rate: Rate::saturating_from_rational(30, 100),
				},
			],
		}));
	});
}

#[test]
fn set_treasury_pool_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, 10000)])
		.build()
		.execute_with(|| {
			let incentives = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 0);

			assert_noop!(
				Fees::set_treasury_pool(Origin::signed(ALICE), NetworkTreasuryPool::get(), vec![]),
				Error::<Runtime>::InvalidParams,
			);

			assert_ok!(Fees::set_treasury_pool(
				Origin::signed(ALICE),
				NetworkTreasuryPool::get(),
				vec![(StakingRewardPool::get(), 70), (CollatorsRewardPool::get(), 30)]
			));
			let incentives = TreasuryToIncentives::<Runtime>::get(NetworkTreasuryPool::get());
			assert_eq!(incentives.len(), 2);
			System::assert_last_event(Event::Fees(crate::Event::TreasuryPoolSet {
				treasury: NetworkTreasuryPool::get(),
				pools: vec![
					PoolPercent {
						pool: StakingRewardPool::get(),
						rate: Rate::saturating_from_rational(70, 100),
					},
					PoolPercent {
						pool: CollatorsRewardPool::get(),
						rate: Rate::saturating_from_rational(30, 100),
					},
				],
			}));
		});
}

#[test]
fn on_fee_change_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, 10000)])
		.build()
		.execute_with(|| {
			assert_ok!(Pallet::<Runtime>::on_fee_changed(IncomeSource::TxFee, None, ACA, 10000));

			assert_eq!(Currencies::free_balance(ACA, &NetworkTreasuryPool::get()), 8000);
			assert_eq!(Currencies::free_balance(ACA, &CollatorsRewardPool::get()), 2000);

			assert_ok!(Pallet::<Runtime>::on_fee_changed(
				IncomeSource::TxFee,
				Some(&TreasuryAccount::get()),
				ACA,
				10000
			));
			assert_eq!(Currencies::free_balance(ACA, &TreasuryAccount::get()), 10000);
		});
}
