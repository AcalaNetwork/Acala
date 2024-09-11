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

//! Unit tests for the Homa Module

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeEvent, *};
use orml_traits::MultiCurrency;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn mint_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, STAKING_CURRENCY_ID, 1_000_000),
			(BOB, STAKING_CURRENCY_ID, 1_000_000),
		])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				Some(1_000_000),
				None,
				None,
				None,
				None,
			));
			MintThreshold::set(100_000);

			assert_noop!(
				Homa::mint(RuntimeOrigin::signed(ALICE), 99_999),
				Error::<Runtime>::BelowMintThreshold
			);
			assert_noop!(
				Homa::mint(RuntimeOrigin::signed(ALICE), 3_000_001),
				Error::<Runtime>::ExceededStakingCurrencySoftCap
			);
			assert_noop!(
				Homa::mint(RuntimeOrigin::signed(ALICE), 3_000_000),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);

			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 0);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 0);
			assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &ALICE), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 1_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 500_000));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::Minted {
				minter: ALICE,
				staking_currency_amount: 500_000,
				liquid_amount_received: 5_000_000,
				liquid_amount_added_to_void: 0,
			}));

			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 5_000_000);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::to_bond_pool(), 500_000);
			assert_eq!(Homa::get_total_staking_currency(), 500_000);
			assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &ALICE), 5_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 500_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				500_000
			);

			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				None,
				Some(Rate::saturating_from_rational(10, 100)),
				None,
				None,
				None,
			));
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &BOB), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &BOB), 1_000_000);

			assert_ok!(Homa::mint(RuntimeOrigin::signed(BOB), 100_000));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::Minted {
				minter: BOB,
				staking_currency_amount: 100_000,
				liquid_amount_received: 909_090,
				liquid_amount_added_to_void: 90910,
			}));

			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 5_909_090);
			assert_eq!(Homa::total_void_liquid(), 90910);
			assert_eq!(Homa::to_bond_pool(), 600_000);
			assert_eq!(Homa::get_total_staking_currency(), 600_000);
			assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &BOB), 909_090);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &BOB), 900_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				600_000
			);
		});
}

#[test]
fn request_redeem_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LIQUID_CURRENCY_ID, 10_000_000),
			(BOB, LIQUID_CURRENCY_ID, 10_000_000),
		])
		.build()
		.execute_with(|| {
			RedeemThreshold::set(1_000_000);

			assert_noop!(
				Homa::request_redeem(RuntimeOrigin::signed(ALICE), 999_999, false),
				Error::<Runtime>::BelowRedeemThreshold
			);

			assert_eq!(Homa::redeem_requests(&ALICE), None);
			assert_eq!(Homa::redeem_requests(&BOB), None);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &ALICE), 10_000_000);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &BOB), 10_000_000);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);

			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 1_000_000, false));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RequestedRedeem {
				redeemer: ALICE,
				liquid_amount: 1_000_000,
				allow_fast_match: false,
			}));
			assert_eq!(Homa::redeem_requests(&ALICE), Some((1_000_000, false)));
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &ALICE), 9_000_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				1_000_000
			);

			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(BOB), 10_000_000, true));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RequestedRedeem {
				redeemer: BOB,
				liquid_amount: 10_000_000,
				allow_fast_match: true,
			}));
			assert_eq!(Homa::redeem_requests(&BOB), Some((10_000_000, true)));
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &BOB), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				11_000_000
			);

			// Alice overwrite the redeem_request
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 2_000_000, true));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RequestedRedeem {
				redeemer: ALICE,
				liquid_amount: 2_000_000,
				allow_fast_match: true,
			}));
			assert_eq!(Homa::redeem_requests(&ALICE), Some((2_000_000, true)));
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &ALICE), 8_000_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				12_000_000
			);

			// Bob cancel the redeem_request
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(BOB), 0, false));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RedeemRequestCancelled {
				redeemer: BOB,
				cancelled_liquid_amount: 10_000_000,
			}));
			assert_eq!(Homa::redeem_requests(&BOB), None);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &BOB), 10_000_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				2_000_000
			);
		});
}

#[test]
fn claim_redemption_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LIQUID_CURRENCY_ID, 10_000_000),
			(BOB, LIQUID_CURRENCY_ID, 10_000_000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(Homa::relay_chain_current_era(), 0);
			Unbondings::<Runtime>::insert(&ALICE, 1, 1_000_000);
			Unbondings::<Runtime>::insert(&ALICE, 2, 2_000_000);
			Unbondings::<Runtime>::insert(&ALICE, 3, 3_000_000);
			assert_eq!(Homa::unbondings(&ALICE, 1), 1_000_000);
			assert_eq!(Homa::unbondings(&ALICE, 2), 2_000_000);
			assert_eq!(Homa::unbondings(&ALICE, 3), 3_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

			// no available expired redemption, nothing happened.
			assert_ok!(Homa::claim_redemption(RuntimeOrigin::signed(BOB), ALICE));
			assert_eq!(Homa::unbondings(&ALICE, 1), 1_000_000);
			assert_eq!(Homa::unbondings(&ALICE, 2), 2_000_000);
			assert_eq!(Homa::unbondings(&ALICE, 3), 3_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 0);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

			// there is available expired redemption, but UnclaimedRedemption is not enough.
			RelayChainCurrentEra::<Runtime>::put(2);
			assert_noop!(
				Homa::claim_redemption(RuntimeOrigin::signed(BOB), ALICE),
				Error::<Runtime>::InsufficientUnclaimedRedemption
			);

			assert_ok!(Currencies::deposit(STAKING_CURRENCY_ID, &Homa::account_id(), 3_000_000));
			UnclaimedRedemption::<Runtime>::put(3_000_000);
			assert_eq!(Homa::unclaimed_redemption(), 3_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				3_000_000
			);

			assert_ok!(Homa::claim_redemption(RuntimeOrigin::signed(BOB), ALICE));
			assert_eq!(Homa::unbondings(&ALICE, 1), 0);
			assert_eq!(Homa::unbondings(&ALICE, 2), 0);
			assert_eq!(Homa::unbondings(&ALICE, 3), 3_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 3_000_000);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);
		});
}

#[test]
fn update_homa_params_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Homa::update_homa_params(RuntimeOrigin::signed(ALICE), None, None, None, None, None),
			BadOrigin
		);

		assert_eq!(Homa::soft_bonded_cap_per_sub_account(), 0);
		assert_eq!(Homa::estimated_reward_rate_per_era(), Rate::zero());
		assert_eq!(Homa::commission_rate(), Rate::zero());
		assert_eq!(Homa::fast_match_fee_rate(), Rate::zero());
		assert_eq!(Homa::nominate_interval_era(), 0);

		assert_ok!(Homa::update_homa_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(1_000_000_000),
			Some(Rate::saturating_from_rational(1, 10000)),
			Some(Rate::saturating_from_rational(5, 100)),
			Some(Rate::saturating_from_rational(1, 100)),
			Some(1),
		));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::SoftBondedCapPerSubAccountUpdated {
			cap_amount: 1_000_000_000,
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::EstimatedRewardRatePerEraUpdated {
			reward_rate: Rate::saturating_from_rational(1, 10000),
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::CommissionRateUpdated {
			commission_rate: Rate::saturating_from_rational(5, 100),
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::FastMatchFeeRateUpdated {
			fast_match_fee_rate: Rate::saturating_from_rational(1, 100),
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::NominateIntervalEraUpdated { eras: 1 }));
		assert_eq!(Homa::soft_bonded_cap_per_sub_account(), 1_000_000_000);
		assert_eq!(
			Homa::estimated_reward_rate_per_era(),
			Rate::saturating_from_rational(1, 10000)
		);
		assert_eq!(Homa::commission_rate(), Rate::saturating_from_rational(5, 100));
		assert_eq!(Homa::fast_match_fee_rate(), Rate::saturating_from_rational(1, 100));
		assert_eq!(Homa::nominate_interval_era(), 1);
	});
}

#[test]
fn update_bump_era_params_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Homa::update_bump_era_params(RuntimeOrigin::signed(ALICE), None, None),
			BadOrigin
		);
		assert_eq!(Homa::last_era_bumped_block(), 0);
		assert_eq!(Homa::bump_era_frequency(), 0);

		MockRelayBlockNumberProvider::set(10);

		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(10),
			Some(7200),
		));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LastEraBumpedBlockUpdated {
			last_era_bumped_block: 10,
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::BumpEraFrequencyUpdated {
			frequency: 7200,
		}));
		assert_eq!(Homa::last_era_bumped_block(), 10);
		assert_eq!(Homa::bump_era_frequency(), 7200);
	});
}

#[test]
fn reset_ledgers_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(Homa::reset_ledgers(RuntimeOrigin::signed(ALICE), vec![]), BadOrigin);

		assert_eq!(Homa::staking_ledgers(0), None);
		assert_eq!(Homa::staking_ledgers(1), None);

		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
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
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LedgerBondedReset {
			sub_account_index: 0,
			new_bonded_amount: 1_000_000,
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LedgerUnlockingReset {
			sub_account_index: 0,
			new_unlocking: vec![
				UnlockChunk { value: 1000, era: 5 },
				UnlockChunk { value: 20_000, era: 6 },
			],
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LedgerUnlockingReset {
			sub_account_index: 1,
			new_unlocking: vec![UnlockChunk { value: 2000, era: 10 }],
		}));
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

		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
			vec![
				(0, None, Some(vec![UnlockChunk { value: 20_000, era: 6 },])),
				(1, Some(0), Some(vec![])),
			]
		));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LedgerUnlockingReset {
			sub_account_index: 0,
			new_unlocking: vec![UnlockChunk { value: 20_000, era: 6 }],
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::LedgerUnlockingReset {
			sub_account_index: 1,
			new_unlocking: vec![],
		}));
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
fn reset_current_era_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(Homa::reset_current_era(RuntimeOrigin::signed(ALICE), 1), BadOrigin);
		assert_eq!(Homa::relay_chain_current_era(), 0);

		assert_ok!(Homa::reset_current_era(RuntimeOrigin::signed(HomaAdmin::get()), 1));
		System::assert_last_event(RuntimeEvent::Homa(crate::Event::CurrentEraReset { new_era_index: 1 }));
		assert_eq!(Homa::relay_chain_current_era(), 1);
	});
}

#[test]
fn get_staking_currency_soft_cap_works() {
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
fn get_total_bonded_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
			vec![
				(0, Some(1_000_000), None),
				(1, Some(2_000_000), None),
				(2, Some(1_000_000), None),
				(3, None, Some(vec![UnlockChunk { value: 1_000, era: 1 }]))
			]
		));
		assert_eq!(Homa::get_total_bonded(), 4_000_000);
	});
}

#[test]
fn get_total_staking_currency_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
			vec![(0, Some(1_000_000), None), (1, Some(2_000_000), None)]
		));
		ToBondPool::<Runtime>::put(2_000_000);
		assert_eq!(Homa::get_total_staking_currency(), 5_000_000);
	});
}

#[test]
fn get_total_liquid_currency_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, LIQUID_CURRENCY_ID, 20_000_000)])
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::total_issuance(LiquidCurrencyId::get()), 20_000_000);
			assert_eq!(Homa::get_total_liquid_currency(), 20_000_000);
			TotalVoidLiquid::<Runtime>::put(10_000_000);
			assert_eq!(Currencies::total_issuance(LiquidCurrencyId::get()), 20_000_000);
			assert_eq!(Homa::get_total_liquid_currency(), 30_000_000);
		});
}

#[test]
fn current_exchange_rate_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::current_exchange_rate(), DefaultExchangeRate::get());
		assert_eq!(Homa::convert_liquid_to_staking(10_000_000), Ok(1_000_000));
		assert_eq!(Homa::convert_staking_to_liquid(1_000_000), Ok(10_000_000));

		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
			vec![(0, Some(1_000_000), None)]
		));

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
fn distribution_helpers_works() {
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
			distribute_decrement(bonded_list, 3_000_000, Some(1_000_000), Some(1_000_001)),
			(vec![(2, 2_000_000)], 1_000_000)
		);
	});
}

#[test]
fn do_fast_match_redeem_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LIQUID_CURRENCY_ID, 20_000_000),
			(BOB, LIQUID_CURRENCY_ID, 20_000_000),
			(CHARLIE, STAKING_CURRENCY_ID, 1_000_000),
		])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::signed(HomaAdmin::get()),
				vec![(0, Some(4_000_000), None)]
			));
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				Some(5_000_000),
				None,
				None,
				Some(Rate::saturating_from_rational(1, 10)),
				None,
			));
			RedeemThreshold::set(1_000_000);
			assert_ok!(Homa::mint(RuntimeOrigin::signed(CHARLIE), 1_000_000));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 5_000_000, true));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(BOB), 6_500_000, true));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(CHARLIE), 5_000_000, false));
			assert_eq!(Homa::redeem_requests(&ALICE), Some((5_000_000, true)));
			assert_eq!(Homa::redeem_requests(&BOB), Some((6_500_000, true)));
			assert_eq!(Homa::redeem_requests(&CHARLIE), Some((5_000_000, false)));
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &BOB), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &CHARLIE), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				16_500_000
			);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				1_000_000
			);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 50_000_000);
			assert_eq!(Homa::to_bond_pool(), 1_000_000);
			assert_eq!(Homa::get_total_staking_currency(), 5_000_000);
			assert_eq!(
				Homa::current_exchange_rate(),
				ExchangeRate::saturating_from_rational(5_000_000, 50_000_000)
			);

			// Charlie's redeem request is not allowed to be fast matched.
			assert_noop!(
				Homa::do_fast_match_redeem(&CHARLIE, true),
				Error::<Runtime>::FastMatchIsNotAllowed
			);

			// Alice's redeem request is able to be fast matched fully.
			assert_ok!(Homa::do_fast_match_redeem(&ALICE, false));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RedeemedByFastMatch {
				redeemer: ALICE,
				matched_liquid_amount: 5_000_000,
				fee_in_liquid: 500_000,
				redeemed_staking_amount: 450_000,
			}));
			assert_eq!(Homa::redeem_requests(&ALICE), None);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &ALICE), 450_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				11_500_000
			);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				550_000
			);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 45_000_000);
			assert_eq!(Homa::to_bond_pool(), 550_000);
			assert_eq!(Homa::get_total_staking_currency(), 4_550_000);
			assert_eq!(
				Homa::current_exchange_rate(),
				ExchangeRate::saturating_from_rational(4_550_000, 45_000_000)
			);

			// Bob's redeem request is able to be fast matched partially,
			// because must remain `RedeemThreshold` even if `ToBondPool` is enough.
			assert_noop!(
				Homa::do_fast_match_redeem(&BOB, false),
				Error::<Runtime>::CannotCompletelyFastMatch,
			);

			assert_ok!(Homa::do_fast_match_redeem(&BOB, true));
			System::assert_last_event(RuntimeEvent::Homa(crate::Event::RedeemedByFastMatch {
				redeemer: BOB,
				matched_liquid_amount: 5_500_000,
				fee_in_liquid: 550_000,
				redeemed_staking_amount: 500_499,
			}));
			assert_eq!(Homa::redeem_requests(&BOB), Some((1_000_000, true)));
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &BOB), 500_499);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				6_000_000
			);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				49_501
			);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 39_500_000);
			assert_eq!(Homa::to_bond_pool(), 49_501);
			assert_eq!(Homa::get_total_staking_currency(), 4_049_501);
			assert_eq!(
				Homa::current_exchange_rate(),
				ExchangeRate::saturating_from_rational(4_049_501, 39_500_000)
			);
		});
}

#[test]
fn process_staking_rewards_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, LIQUID_CURRENCY_ID, 40_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::signed(HomaAdmin::get()),
				vec![(0, Some(3_000_000), None), (1, Some(1_000_000), None),]
			));
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				None,
				Some(Rate::saturating_from_rational(20, 100)),
				None,
				None,
				None,
			));
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 3_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::get_total_bonded(), 4_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 40_000_000);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()), 0);

			// accumulate staking rewards, no commission
			assert_ok!(Homa::process_staking_rewards(1, 0));
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 3_600_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_200_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::get_total_bonded(), 4_800_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 40_000_000);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()), 0);

			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				None,
				None,
				Some(Rate::saturating_from_rational(10, 100)),
				None,
				None,
			));

			// accumulate staking rewards, will draw commission to TreasuryAccount
			assert_ok!(Homa::process_staking_rewards(2, 1));
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 4_320_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_440_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::get_total_bonded(), 5_760_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 40_677_966);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()),
				677_966
			);
		});
}

#[test]
fn process_scheduled_unbond_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Homa::reset_ledgers(
			RuntimeOrigin::signed(HomaAdmin::get()),
			vec![
				(
					0,
					None,
					Some(vec![
						UnlockChunk {
							value: 1_000_000,
							era: 11
						},
						UnlockChunk {
							value: 2_000_000,
							era: 14
						},
					])
				),
				(
					1,
					None,
					Some(vec![
						UnlockChunk {
							value: 100_000,
							era: 12
						},
						UnlockChunk {
							value: 200_000,
							era: 13
						},
					])
				),
			]
		));
		assert_eq!(
			Homa::staking_ledgers(0),
			Some(StakingLedger {
				bonded: 0,
				unlocking: vec![
					UnlockChunk {
						value: 1_000_000,
						era: 11
					},
					UnlockChunk {
						value: 2_000_000,
						era: 14
					},
				]
			})
		);
		assert_eq!(
			Homa::staking_ledgers(1),
			Some(StakingLedger {
				bonded: 0,
				unlocking: vec![
					UnlockChunk {
						value: 100_000,
						era: 12
					},
					UnlockChunk {
						value: 200_000,
						era: 13
					},
				]
			})
		);
		assert_eq!(Homa::unclaimed_redemption(), 0);
		assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 0);
		assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

		assert_ok!(Homa::process_scheduled_unbond(13));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaWithdrawUnbonded {
			sub_account_index: 0,
			amount: 1_000_000,
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaWithdrawUnbonded {
			sub_account_index: 1,
			amount: 300_000,
		}));
		assert_eq!(
			Homa::staking_ledgers(0),
			Some(StakingLedger {
				bonded: 0,
				unlocking: vec![UnlockChunk {
					value: 2_000_000,
					era: 14
				},]
			})
		);
		assert_eq!(Homa::staking_ledgers(1), None);
		assert_eq!(Homa::unclaimed_redemption(), 1_300_000);
		assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 1_300_000);
		assert_eq!(
			Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
			1_300_000
		);
	});
}

#[test]
fn process_to_bond_pool_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, STAKING_CURRENCY_ID, 20_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				Some(3_000_000),
				None,
				None,
				None,
				None,
			));
			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::signed(HomaAdmin::get()),
				vec![(0, Some(1_000_000), None)]
			));
			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 900_000));
			assert_eq!(MockHomaSubAccountXcm::get_xcm_transfer_fee(), 1_000_000);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(1), None);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 900_000);
			assert_eq!(Homa::get_total_bonded(), 1_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 20_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				900_000
			);

			// ToBondPool is unable to afford xcm_transfer_fee
			assert_ok!(Homa::process_to_bond_pool());
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(1), None);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 900_000);
			assert_eq!(Homa::get_total_bonded(), 1_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 20_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				900_000
			);

			// ToBondPool is able to afford xcm_transfer_fee, but no bonded added
			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 100_000));
			assert_eq!(Homa::to_bond_pool(), 1_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 20_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				1_000_000
			);
			assert_ok!(Homa::process_to_bond_pool());
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(1), None);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::get_total_bonded(), 1_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 19_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

			// ToBondPool is able to afford xcm_transfer_fee, and bonded added
			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 6_000_000));
			assert_eq!(Homa::to_bond_pool(), 6_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 19_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				6_000_000
			);
			assert_ok!(Homa::process_to_bond_pool());
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaBondExtra {
				sub_account_index: 1,
				amount: 3_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaBondExtra {
				sub_account_index: 2,
				amount: 1_000_000,
			}));
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 3_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(2),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::get_total_bonded(), 5_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 13_000_000);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);

			// ToBondPool is able to afford xcm_transfer_fee, and below the mint_threshold, no bonded added.
			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 2_000_000));
			MintThreshold::set(3_000_000);
			assert_eq!(Homa::to_bond_pool(), 2_000_000);
			assert_eq!(Homa::get_total_bonded(), 5_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 13_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				2_000_000
			);
			assert_ok!(Homa::process_to_bond_pool());
			assert_eq!(Homa::to_bond_pool(), 2_000_000);
			assert_eq!(Homa::get_total_bonded(), 5_000_000);
			assert_eq!(Currencies::total_issuance(STAKING_CURRENCY_ID), 13_000_000);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				2_000_000
			);
		});
}

#[test]
fn process_redeem_requests_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LIQUID_CURRENCY_ID, 20_000_000),
			(BOB, LIQUID_CURRENCY_ID, 20_000_000),
			(CHARLIE, LIQUID_CURRENCY_ID, 10_000_000),
			(DAVE, LIQUID_CURRENCY_ID, 10_000_000),
		])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::signed(HomaAdmin::get()),
				vec![(0, Some(2_000_000), None), (1, Some(3_000_000), None),]
			));
			ToBondPool::<Runtime>::put(1_000_000);
			assert_eq!(Homa::relay_chain_current_era(), 0);

			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 20_000_000, false));
			assert_eq!(Homa::redeem_requests(&ALICE), Some((20_000_000, false)));
			assert_eq!(Homa::unbondings(&ALICE, 1 + BondingDuration::get()), 0);
			assert_eq!(Homa::get_total_bonded(), 5_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 60_000_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				20_000_000
			);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 2_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 3_000_000,
					unlocking: vec![]
				})
			);

			// total_bonded is enough to process all redeem requests
			assert_eq!(Homa::process_redeem_requests(1), Ok(1));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: ALICE,
				era_index_when_unbond: 1,
				liquid_amount: 20_000_000,
				unbonding_staking_amount: 2_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaUnbond {
				sub_account_index: 1,
				amount: 2_000_000,
			}));
			assert_eq!(Homa::redeem_requests(&ALICE), None);
			assert_eq!(Homa::unbondings(&ALICE, 1 + BondingDuration::get()), 2_000_000);
			assert_eq!(Homa::get_total_bonded(), 3_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 40_000_000);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 2_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_000_000,
					unlocking: vec![UnlockChunk {
						value: 2_000_000,
						era: 1 + BondingDuration::get()
					}]
				})
			);

			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(BOB), 20_000_000, false));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(CHARLIE), 10_000_000, false));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(DAVE), 10_000_000, false));
			assert_eq!(Homa::redeem_requests(&BOB), Some((20_000_000, false)));
			assert_eq!(Homa::redeem_requests(&CHARLIE), Some((10_000_000, false)));
			assert_eq!(Homa::redeem_requests(&DAVE), Some((10_000_000, false)));
			assert_eq!(Homa::unbondings(&BOB, 2 + BondingDuration::get()), 0);
			assert_eq!(Homa::unbondings(&CHARLIE, 2 + BondingDuration::get()), 0);
			assert_eq!(Homa::unbondings(&DAVE, 2 + BondingDuration::get()), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				40_000_000
			);

			// total_bonded is not enough to process all redeem requests
			assert_eq!(Homa::process_redeem_requests(2), Ok(2));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: BOB,
				era_index_when_unbond: 2,
				liquid_amount: 20_000_000,
				unbonding_staking_amount: 2_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: CHARLIE,
				era_index_when_unbond: 2,
				liquid_amount: 10_000_000,
				unbonding_staking_amount: 1_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaUnbond {
				sub_account_index: 0,
				amount: 2_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaUnbond {
				sub_account_index: 1,
				amount: 1_000_000,
			}));
			assert_eq!(Homa::redeem_requests(&BOB), None);
			assert_eq!(Homa::redeem_requests(&CHARLIE), None);
			assert_eq!(Homa::redeem_requests(&DAVE), Some((10_000_000, false)));
			assert_eq!(Homa::unbondings(&BOB, 2 + BondingDuration::get()), 2_000_000);
			assert_eq!(Homa::unbondings(&CHARLIE, 2 + BondingDuration::get()), 1_000_000);
			assert_eq!(Homa::unbondings(&DAVE, 2 + BondingDuration::get()), 0);
			assert_eq!(Homa::get_total_bonded(), 0);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 10_000_000);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				10_000_000
			);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 0,
					unlocking: vec![UnlockChunk {
						value: 2_000_000,
						era: 2 + BondingDuration::get()
					}]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 0,
					unlocking: vec![
						UnlockChunk {
							value: 2_000_000,
							era: 1 + BondingDuration::get()
						},
						UnlockChunk {
							value: 1_000_000,
							era: 2 + BondingDuration::get()
						}
					]
				})
			);
		});
}

#[test]
fn process_nominate_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::nominate_interval_era(), 0);

		// will not nominate
		assert_ok!(Homa::process_nominate(1));
		assert_eq!(System::events(), vec![]);

		NominateIntervalEra::<Runtime>::put(4);

		// will not nominate
		assert_ok!(Homa::process_nominate(2));
		assert_ok!(Homa::process_nominate(3));
		assert_eq!(System::events(), vec![]);

		assert_ok!(Homa::process_nominate(4));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaNominate {
			sub_account_index: 0,
			nominations: vec![VALIDATOR_A, VALIDATOR_B],
		}));
		System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaNominate {
			sub_account_index: 2,
			nominations: vec![VALIDATOR_A, VALIDATOR_C],
		}));
		// will not nominate for subaccount#1 because doesn't get nominations
	});
}

#[test]
fn era_amount_should_to_bump_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::last_era_bumped_block(), 0);
		assert_eq!(Homa::bump_era_frequency(), 0);
		assert_eq!(Homa::era_amount_should_to_bump(9), 0);
		assert_eq!(Homa::era_amount_should_to_bump(10), 0);
		assert_eq!(Homa::era_amount_should_to_bump(11), 0);
		assert_eq!(Homa::era_amount_should_to_bump(30), 0);

		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			None,
			Some(10)
		));
		assert_eq!(Homa::bump_era_frequency(), 10);
		assert_eq!(Homa::era_amount_should_to_bump(9), 0);
		assert_eq!(Homa::era_amount_should_to_bump(10), 1);
		assert_eq!(Homa::era_amount_should_to_bump(11), 1);
		assert_eq!(Homa::era_amount_should_to_bump(30), 3);

		MockRelayBlockNumberProvider::set(10);
		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(1),
			None
		));
		assert_eq!(Homa::last_era_bumped_block(), 1);
		assert_eq!(Homa::era_amount_should_to_bump(9), 0);
		assert_eq!(Homa::era_amount_should_to_bump(10), 0);
		assert_eq!(Homa::era_amount_should_to_bump(11), 1);
		assert_eq!(Homa::era_amount_should_to_bump(30), 2);
	});
}

#[test]
fn bump_current_era_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, STAKING_CURRENCY_ID, 100_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				Some(20_000_000),
				Some(Rate::saturating_from_rational(1, 100)),
				Some(Rate::saturating_from_rational(20, 100)),
				None,
				None,
			));
			MintThreshold::set(2_000_000);

			// initial states at era #0
			assert_eq!(Homa::last_era_bumped_block(), 0);
			assert_eq!(Homa::relay_chain_current_era(), 0);
			assert_eq!(Homa::staking_ledgers(0), None);
			assert_eq!(Homa::staking_ledgers(1), None);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 0);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 0);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()), 0);

			assert_ok!(Homa::mint(RuntimeOrigin::signed(ALICE), 30_000_000));
			assert_eq!(Homa::to_bond_pool(), 30_000_000);
			assert_eq!(Homa::total_void_liquid(), 2_970_298);
			assert_eq!(Homa::get_total_staking_currency(), 30_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 297_029_702);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				30_000_000
			);

			// bump era to #1,
			// will process to_bond_pool.
			MockRelayBlockNumberProvider::set(100);
			assert_eq!(Homa::bump_current_era(1), Ok(0));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::CurrentEraBumped { new_era_index: 1 }));
			assert_eq!(Homa::last_era_bumped_block(), 100);
			assert_eq!(Homa::relay_chain_current_era(), 1);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 20_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 8_000_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 28_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 297_029_702);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()), 0);

			// bump era to #2,
			// accumulate staking reward and draw commission
			MockRelayBlockNumberProvider::set(200);
			assert_eq!(Homa::bump_current_era(1), Ok(0));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::CurrentEraBumped { new_era_index: 2 }));
			assert_eq!(Homa::last_era_bumped_block(), 200);
			assert_eq!(Homa::relay_chain_current_era(), 2);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 20_200_000,
					unlocking: vec![]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 8_080_000,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 28_280_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 297_619_046);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()),
				589_344
			);

			// assuming now staking has no rewards any more.
			assert_ok!(Homa::update_homa_params(
				RuntimeOrigin::signed(HomaAdmin::get()),
				None,
				Some(Rate::zero()),
				None,
				None,
				None,
			));

			// and there's redeem request
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 280_000_000, false));
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()),
				280_000_000
			);

			// bump era to #3,
			// will process redeem requests
			MockRelayBlockNumberProvider::set(300);
			assert_eq!(Homa::bump_current_era(1), Ok(1));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::CurrentEraBumped { new_era_index: 3 }));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: ALICE,
				era_index_when_unbond: 3,
				liquid_amount: 280_000_000,
				unbonding_staking_amount: 26_605_824,
			}));
			assert_eq!(Homa::last_era_bumped_block(), 300);
			assert_eq!(Homa::relay_chain_current_era(), 3);
			assert_eq!(
				Homa::staking_ledgers(0),
				Some(StakingLedger {
					bonded: 0,
					unlocking: vec![UnlockChunk {
						value: 20_200_000,
						era: 3 + BondingDuration::get()
					}]
				})
			);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_674_176,
					unlocking: vec![UnlockChunk {
						value: 6_405_824,
						era: 3 + BondingDuration::get()
					}]
				})
			);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::unclaimed_redemption(), 0);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 1_674_176);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 17_619_046);
			assert_eq!(Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()),
				589_344
			);

			// bump era to #31,
			// will process scheduled unbonded
			MockRelayBlockNumberProvider::set(3100);
			assert_eq!(Homa::bump_current_era(28), Ok(0));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::CurrentEraBumped { new_era_index: 31 }));
			assert_eq!(Homa::last_era_bumped_block(), 3100);
			assert_eq!(Homa::relay_chain_current_era(), 31);
			assert_eq!(Homa::staking_ledgers(0), None);
			assert_eq!(
				Homa::staking_ledgers(1),
				Some(StakingLedger {
					bonded: 1_674_176,
					unlocking: vec![]
				})
			);
			assert_eq!(Homa::staking_ledgers(2), None);
			assert_eq!(Homa::to_bond_pool(), 0);
			assert_eq!(Homa::unclaimed_redemption(), 26_605_824);
			assert_eq!(Homa::total_void_liquid(), 0);
			assert_eq!(Homa::get_total_staking_currency(), 1_674_176);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 17_619_046);
			assert_eq!(
				Currencies::free_balance(STAKING_CURRENCY_ID, &Homa::account_id()),
				26_605_824
			);
			assert_eq!(Currencies::free_balance(LIQUID_CURRENCY_ID, &Homa::account_id()), 0);
			assert_eq!(
				Currencies::free_balance(LIQUID_CURRENCY_ID, &TreasuryAccount::get()),
				589_344
			);
		});
}

#[test]
fn last_era_bumped_block_config_check_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Homa::last_era_bumped_block(), 0);
		assert_eq!(Homa::bump_era_frequency(), 0);
		assert_eq!(MockRelayBlockNumberProvider::current_block_number(), 0);

		MockRelayBlockNumberProvider::set(100);

		// it's ok, nothing happen because bump_era_frequency is zero
		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(100),
			None,
		));
		assert_eq!(Homa::last_era_bumped_block(), 0);
		assert_eq!(Homa::bump_era_frequency(), 0);

		// 50 will trigger bump era
		assert_noop!(
			Homa::update_bump_era_params(RuntimeOrigin::signed(HomaAdmin::get()), Some(50), Some(50),),
			Error::<Runtime>::InvalidLastEraBumpedBlock
		);

		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(51),
			Some(50),
		));
		assert_eq!(Homa::last_era_bumped_block(), 51);
		assert_eq!(Homa::bump_era_frequency(), 50);
		assert_eq!(MockRelayBlockNumberProvider::current_block_number(), 100);

		// 101 is great than current relaychain block
		assert_noop!(
			Homa::update_bump_era_params(RuntimeOrigin::signed(HomaAdmin::get()), Some(101), None,),
			Error::<Runtime>::InvalidLastEraBumpedBlock
		);

		assert_ok!(Homa::update_bump_era_params(
			RuntimeOrigin::signed(HomaAdmin::get()),
			Some(100),
			None,
		));
		assert_eq!(Homa::last_era_bumped_block(), 100);
		assert_eq!(Homa::bump_era_frequency(), 50);
		assert_eq!(MockRelayBlockNumberProvider::current_block_number(), 100);
	});
}

#[test]
fn process_redeem_requests_under_limit_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, LIQUID_CURRENCY_ID, 10_000_000),
			(BOB, LIQUID_CURRENCY_ID, 10_000_000),
			(CHARLIE, LIQUID_CURRENCY_ID, 10_000_000),
			(DAVE, LIQUID_CURRENCY_ID, 10_000_000),
		])
		.build()
		.execute_with(|| {
			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::signed(HomaAdmin::get()),
				vec![(0, Some(4_000_000), None)]
			));
			ToBondPool::<Runtime>::put(4_000_000);

			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(ALICE), 5_000_000, false));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(BOB), 5_000_000, false));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(CHARLIE), 5_000_000, false));
			assert_ok!(Homa::request_redeem(RuntimeOrigin::signed(DAVE), 5_000_000, false));
			assert_eq!(Homa::redeem_requests(&ALICE), Some((5_000_000, false)));
			assert_eq!(Homa::redeem_requests(&BOB), Some((5_000_000, false)));
			assert_eq!(Homa::redeem_requests(&CHARLIE), Some((5_000_000, false)));
			assert_eq!(Homa::redeem_requests(&DAVE), Some((5_000_000, false)));
			assert_eq!(Homa::unbondings(&ALICE, 1 + BondingDuration::get()), 0);
			assert_eq!(Homa::unbondings(&BOB, 1 + BondingDuration::get()), 0);
			assert_eq!(Homa::unbondings(&CHARLIE, 1 + BondingDuration::get()), 0);
			assert_eq!(Homa::unbondings(&DAVE, 1 + BondingDuration::get()), 0);
			assert_eq!(Homa::get_total_bonded(), 4_000_000);
			assert_eq!(Currencies::total_issuance(LIQUID_CURRENCY_ID), 40_000_000);

			// total_bonded is enough to process all redeem requests, but excceed limit
			assert_eq!(Homa::process_redeem_requests(1), Ok(3));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: ALICE,
				era_index_when_unbond: 1,
				liquid_amount: 5_000_000,
				unbonding_staking_amount: 1_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: BOB,
				era_index_when_unbond: 1,
				liquid_amount: 5_000_000,
				unbonding_staking_amount: 1_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::RedeemedByUnbond {
				redeemer: CHARLIE,
				era_index_when_unbond: 1,
				liquid_amount: 5_000_000,
				unbonding_staking_amount: 1_000_000,
			}));
			System::assert_has_event(RuntimeEvent::Homa(crate::Event::HomaUnbond {
				sub_account_index: 0,
				amount: 3_000_000,
			}));
			assert_eq!(Homa::redeem_requests(&ALICE), None);
			assert_eq!(Homa::redeem_requests(&BOB), None);
			assert_eq!(Homa::redeem_requests(&CHARLIE), None);
			assert_eq!(Homa::redeem_requests(&DAVE), Some((5_000_000, false)));
			assert_eq!(Homa::unbondings(&ALICE, 1 + BondingDuration::get()), 1_000_000);
			assert_eq!(Homa::unbondings(&BOB, 1 + BondingDuration::get()), 1_000_000);
			assert_eq!(Homa::unbondings(&CHARLIE, 1 + BondingDuration::get()), 1_000_000);
			assert_eq!(Homa::unbondings(&DAVE, 1 + BondingDuration::get()), 0);
		});
}
