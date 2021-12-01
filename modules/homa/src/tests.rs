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

//! Unit tests for the Homa Module

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use orml_traits::MultiCurrency;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn update_ledgers_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(Homa::update_ledgers(Origin::signed(ALICE), vec![]), BadOrigin);

		assert_eq!(Homa::staking_ledgers(0), None);
		assert_eq!(Homa::staking_ledgers(1), None);

		assert_ok!(Homa::update_ledgers(
			Origin::signed(HomaAdmin::get()),
			vec![
				(
					0,
					Some(1_000_000),
					Some(vec![
						UnlockChunk { value: 1000, era: 5 },
						UnlockChunk { value: 20_000, era: 6 },
					])
				),
				(1, None, Some(vec![UnlockChunk { value: 2000, era: 10 },])),
			]
		));
		System::assert_has_event(Event::Homa(crate::Event::LedgerBondedUpdated(0, 1_000_000)));
		System::assert_has_event(Event::Homa(crate::Event::LedgerUnlockingUpdated(
			0,
			vec![
				UnlockChunk { value: 1000, era: 5 },
				UnlockChunk { value: 20_000, era: 6 },
			],
		)));
		System::assert_has_event(Event::Homa(crate::Event::LedgerUnlockingUpdated(
			1,
			vec![UnlockChunk { value: 2000, era: 10 }],
		)));
		assert_eq!(
			Homa::staking_ledgers(0),
			Some(StakingLedger {
				bonded: 1_000_000,
				unlocking: vec![
					UnlockChunk { value: 1000, era: 5 },
					UnlockChunk { value: 20_000, era: 6 },
				]
			})
		);
		assert_eq!(
			Homa::staking_ledgers(1),
			Some(StakingLedger {
				bonded: 0,
				unlocking: vec![UnlockChunk { value: 2000, era: 10 },]
			})
		);

		assert_ok!(Homa::update_ledgers(
			Origin::signed(HomaAdmin::get()),
			vec![
				(0, None, Some(vec![UnlockChunk { value: 20_000, era: 6 },])),
				(1, Some(0), Some(vec![])),
			]
		));
		System::assert_has_event(Event::Homa(crate::Event::LedgerUnlockingUpdated(
			0,
			vec![UnlockChunk { value: 20_000, era: 6 }],
		)));
		System::assert_has_event(Event::Homa(crate::Event::LedgerUnlockingUpdated(1, vec![])));
		assert_eq!(
			Homa::staking_ledgers(0),
			Some(StakingLedger {
				bonded: 1_000_000,
				unlocking: vec![UnlockChunk { value: 20_000, era: 6 },]
			})
		);
		assert_eq!(Homa::staking_ledgers(1), None);
	});
}

#[test]
fn update_homa_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Homa::update_homa_params(Origin::signed(ALICE), None, None, None, None, None, None),
			BadOrigin
		);

		assert_eq!(Homa::soft_bonded_cap_per_sub_account(), 0);
		assert_eq!(Homa::estimated_reward_rate_per_era(), Rate::zero());
		assert_eq!(Homa::mint_threshold(), 0);
		assert_eq!(Homa::redeem_threshold(), 0);
		assert_eq!(Homa::commission_rate(), Rate::zero());
		assert_eq!(Homa::fast_match_fee_rate(), Rate::zero());

		assert_ok!(Homa::update_homa_params(
			Origin::signed(HomaAdmin::get()),
			Some(dollar(10000)),
			Some(Rate::saturating_from_rational(1, 10000)),
			Some(dollar(1)),
			Some(dollar(10)),
			Some(Rate::saturating_from_rational(5, 100)),
			Some(Rate::saturating_from_rational(1, 100)),
		));
		System::assert_has_event(Event::Homa(crate::Event::SoftBondedCapPerSubAccountUpdated(dollar(
			10000,
		))));
		System::assert_has_event(Event::Homa(crate::Event::EstimatedRewardRatePerEraUpdated(
			Rate::saturating_from_rational(1, 10000),
		)));
		System::assert_has_event(Event::Homa(crate::Event::MintThresholdUpdated(dollar(1))));
		System::assert_has_event(Event::Homa(crate::Event::RedeemThresholdUpdated(dollar(10))));
		System::assert_has_event(Event::Homa(crate::Event::CommissionRateUpdated(
			Rate::saturating_from_rational(5, 100),
		)));
		System::assert_has_event(Event::Homa(crate::Event::FastMatchFeeRateUpdated(
			Rate::saturating_from_rational(1, 100),
		)));
		assert_eq!(Homa::soft_bonded_cap_per_sub_account(), dollar(10000));
		assert_eq!(
			Homa::estimated_reward_rate_per_era(),
			Rate::saturating_from_rational(1, 10000)
		);
		assert_eq!(Homa::mint_threshold(), dollar(1));
		assert_eq!(Homa::redeem_threshold(), dollar(10));
		assert_eq!(Homa::commission_rate(), Rate::saturating_from_rational(5, 100));
		assert_eq!(Homa::fast_match_fee_rate(), Rate::saturating_from_rational(1, 100));
	});
}

#[test]
fn get_staking_currency_soft_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::get_staking_currency_soft_cap(), 0);
		SoftBondedCapPerSubAccount::<Runtime>::put(1_000_000);
		assert_eq!(
			Homa::get_staking_currency_soft_cap(),
			1_000_000 * (ActiveSubAccountsIndexList::get().len() as Balance)
		);
	});
}

#[test]
fn get_total_bonded_soft_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		StakingLedgers::<Runtime>::insert(
			0,
			StakingLedger {
				bonded: 1_000_000,
				..Default::default()
			},
		);
		StakingLedgers::<Runtime>::insert(
			1,
			StakingLedger {
				bonded: 2_000_000,
				..Default::default()
			},
		);
		StakingLedgers::<Runtime>::insert(
			3,
			StakingLedger {
				bonded: 1_000_000,
				..Default::default()
			},
		);
		assert_eq!(Homa::get_total_bonded(), 4_000_000);
	});
}

#[test]
fn get_total_staking_currency_work() {
	ExtBuilder::default().build().execute_with(|| {
		StakingLedgers::<Runtime>::insert(
			0,
			StakingLedger {
				bonded: 1_000_000,
				..Default::default()
			},
		);
		StakingLedgers::<Runtime>::insert(
			1,
			StakingLedger {
				bonded: 2_000_000,
				..Default::default()
			},
		);
		ToBondPool::<Runtime>::put(2_000_000);
		assert_eq!(Homa::get_total_staking_currency(), 5_000_000);
	});
}

#[test]
fn current_exchange_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
		assert_eq!(Homa::convert_liquid_to_staking(10_000_000), Ok(1_000_000));
		assert_eq!(Homa::convert_staking_to_liquid(1_000_000), Ok(10_000_000));

		StakingLedgers::<Runtime>::insert(
			0,
			StakingLedger {
				bonded: 1_000_000,
				..Default::default()
			},
		);
		assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
		assert_eq!(Homa::convert_liquid_to_staking(10_000_000), Ok(1_000_000));
		assert_eq!(Homa::convert_staking_to_liquid(1_000_000), Ok(10_000_000));

		assert_ok!(Currencies::deposit(LiquidCurrencyId::get(), &ALICE, 5_000_000));
		assert_eq!(
			Homa::current_exchange_rate(),
			ExchangeRate::saturating_from_rational(1_000_000, 5_000_000)
		);
		assert_eq!(Homa::convert_liquid_to_staking(10_000_000), Ok(2_000_000));
		assert_eq!(Homa::convert_staking_to_liquid(1_000_000), Ok(5_000_000));

		TotalVoidLiquid::<Runtime>::put(3_000_000);
		assert_eq!(
			Homa::current_exchange_rate(),
			ExchangeRate::saturating_from_rational(1_000_000, 8_000_000)
		);
		assert_eq!(Homa::convert_liquid_to_staking(10_000_000), Ok(1_250_000));
		assert_eq!(Homa::convert_staking_to_liquid(1_000_000), Ok(8_000_000));
	});
}

#[test]
fn distribution_helpers_work() {
	ExtBuilder::default().build().execute_with(|| {
		let bonded_list = vec![(0, 1_000_000), (1, 2_000_000), (2, 3_000_000), (3, 100_000)];

		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, None, None),
			(vec![(3, 2_000_000)], 0)
		);
		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, Some(1_100_000), None),
			(vec![(3, 1_000_000), (0, 100_000)], 900_000)
		);
		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, Some(100_000), None),
			(vec![], 2_000_000)
		);
		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, None, Some(2_000_001)),
			(vec![], 2_000_000)
		);
		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, Some(1_000_000), Some(900_001)),
			(vec![], 2_000_000)
		);
		assert_eq!(
			distribute_increment(bonded_list.clone(), 2_000_000, Some(1_200_000), Some(1_000_000)),
			(vec![(3, 1_100_000)], 900_000)
		);

		assert_eq!(
			distribute_decrement(bonded_list.clone(), 7_000_000, None, None),
			(
				vec![(2, 3_000_000), (1, 2_000_000), (0, 1_000_000), (3, 100_000)],
				900_000
			)
		);
		assert_eq!(
			distribute_decrement(bonded_list.clone(), 2_000_000, Some(1_800_000), None),
			(vec![(2, 1_200_000), (1, 200_000)], 600_000)
		);
		assert_eq!(
			distribute_decrement(bonded_list.clone(), 2_000_000, Some(3_000_000), None),
			(vec![], 2_000_000)
		);
		assert_eq!(
			distribute_decrement(bonded_list.clone(), 6_000_000, None, Some(2_000_000)),
			(vec![(2, 3_000_000), (1, 2_000_000)], 1_000_000)
		);
		assert_eq!(
			distribute_decrement(bonded_list.clone(), 2_000_000, None, Some(3_000_001)),
			(vec![], 2_000_000)
		);
		assert_eq!(
			distribute_decrement(bonded_list.clone(), 3_000_000, Some(1_000_000), Some(1_000_001)),
			(vec![(2, 2_000_000)], 1_000_000)
		);
	});
}
