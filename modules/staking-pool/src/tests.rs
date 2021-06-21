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

//! Unit tests for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	BondingDuration, CurrenciesModule, Event, ExtBuilder, One, Origin, Runtime, StakingPoolModule, Status, System,
	ALICE, BOB, BRIDGE_STATUS, DOT, LDOT,
};
use sp_runtime::traits::BadOrigin;

#[test]
fn distribute_increment_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(StakingPoolModule::distribute_increment(vec![], 1000), vec![]);
		assert_eq!(
			StakingPoolModule::distribute_increment(vec![(1, 300), (2, 200), (3, 400), (4, 200)], 1000),
			vec![(1, 1000)]
		);
		assert_eq!(
			StakingPoolModule::distribute_increment(vec![(2, 200), (1, 300), (3, 400), (4, 200)], 1000),
			vec![(2, 1000)]
		);
	});
}

#[test]
fn distribute_decrement_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(StakingPoolModule::distribute_increment(vec![], 1000), vec![]);
		assert_eq!(
			StakingPoolModule::distribute_decrement(vec![(1, 300), (2, 200), (3, 400), (4, 200)], 1000),
			vec![(1, 300), (2, 200), (3, 400), (4, 100)]
		);
		assert_eq!(
			StakingPoolModule::distribute_decrement(vec![(1, 300), (2, 200), (3, 400), (4, 200)], 500),
			vec![(1, 300), (2, 200)]
		);
	});
}

#[test]
fn relaychain_staking_ledger_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![(1, 50), (3, 60), (4, 20)],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![(1, 20), (2, 40)],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![(2, 50), (4, 100)],
				},
			);
			*v.borrow_mut() = old_map;
		});

		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 1340,
				active: 1000,
				unlocking: vec![
					PolkadotUnlockChunk { value: 70, era: 1 },
					PolkadotUnlockChunk { value: 90, era: 2 },
					PolkadotUnlockChunk { value: 60, era: 3 },
					PolkadotUnlockChunk { value: 120, era: 4 },
				],
			}
		);
	});
}

#[test]
fn balance_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![(1, 50), (3, 60), (4, 20)],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![(1, 20), (2, 40)],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![(2, 50), (4, 100)],
				},
			);
			*v.borrow_mut() = old_map;
		});

		assert_eq!(StakingPoolModule::relaychain_free_balance(), 1000);
	});
}

#[test]
fn transfer_to_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 0,
				free: 0,
				unlocking: vec![],
			}
		);

		assert_ok!(StakingPoolModule::transfer_to_bridge(&ALICE, 500));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 500);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 0,
				free: 500,
				unlocking: vec![],
			}
		);
	});
}

#[test]
fn receive_from_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![],
				},
			);
			*v.borrow_mut() = old_map;
		});
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);

		assert_ok!(StakingPoolModule::receive_from_bridge(&ALICE, 600));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1600);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 300,
				free: 0,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&2)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 100,
				free: 300,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&3)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 200,
				free: 100,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&4)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 400,
				free: 0,
				unlocking: vec![],
			}
		);
	});
}

#[test]
fn bond_extra_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![],
				},
			);
			*v.borrow_mut() = old_map;
		});

		assert_ok!(StakingPoolModule::bond_extra(600));
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 300,
				free: 200,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&2)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 400,
				free: 0,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&3)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 500,
				free: 100,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&4)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 400,
				free: 100,
				unlocking: vec![],
			}
		);
	});
}

#[test]
fn unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![],
				},
			);
			*v.borrow_mut() = old_map;
		});

		CurrentEra::<Runtime>::put(5);
		assert_ok!(StakingPoolModule::unbond(600));
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 100,
				free: 200,
				unlocking: vec![(9, 200)],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&2)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 100,
				free: 300,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&3)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 200,
				free: 400,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&4)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 0,
				free: 100,
				unlocking: vec![(9, 400)],
			}
		);
	});
}

#[test]
fn withdraw_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![(1, 100), (4, 300)],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![(1, 50), (2, 30)],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 400,
					free: 100,
					unlocking: vec![(3, 100), (5, 300)],
				},
			);
			*v.borrow_mut() = old_map;
		});

		CurrentEra::<Runtime>::put(3);
		StakingPoolModule::withdraw_unbonded();
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 300,
				free: 300,
				unlocking: vec![(4, 300)],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&2)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 100,
				free: 380,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&3)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 200,
				free: 400,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&4)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 400,
				free: 200,
				unlocking: vec![(5, 300)],
			}
		);
	});
}

#[test]
fn payout_stakers_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 300,
					free: 200,
					unlocking: vec![],
				},
			);
			old_map.insert(
				2,
				Status {
					bonded: 100,
					free: 300,
					unlocking: vec![],
				},
			);
			old_map.insert(
				3,
				Status {
					bonded: 200,
					free: 400,
					unlocking: vec![],
				},
			);
			old_map.insert(
				4,
				Status {
					bonded: 0,
					free: 100,
					unlocking: vec![],
				},
			);
			*v.borrow_mut() = old_map;
		});

		StakingPoolModule::payout_stakers(0);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 303,
				free: 200,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&2)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 101,
				free: 300,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&3)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 202,
				free: 400,
				unlocking: vec![],
			}
		);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&4)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 0,
				free: 100,
				unlocking: vec![],
			}
		);
	});
}

#[test]
fn staking_pool_ledger_work() {
	ExtBuilder::default().build().execute_with(|| {
		let ledger = Ledger {
			bonded: 1000,
			free_pool: 200,
			unbonding_to_free: 300,
			to_unbond_next_era: (300, 200),
		};

		assert_eq!(ledger.total(), 1500);
		assert_eq!(ledger.total_belong_to_liquid_holders(), 1300);
		assert_eq!(ledger.bonded_belong_to_liquid_holders(), 800);
		assert_eq!(ledger.free_pool_ratio(), Ratio::saturating_from_rational(200, 1300));
		assert_eq!(
			ledger.unbonding_to_free_ratio(),
			Ratio::saturating_from_rational(300, 1300)
		);
	});
}

#[test]
fn liquid_exchange_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			StakingPoolModule::liquid_exchange_rate(),
			ExchangeRate::saturating_from_rational(10, 100)
		);

		StakingPoolLedger::<Runtime>::put(Ledger {
			bonded: 1000,
			free_pool: 300,
			unbonding_to_free: 400,
			to_unbond_next_era: (200, 200),
		});

		assert_eq!(
			StakingPoolModule::liquid_exchange_rate(),
			ExchangeRate::saturating_from_rational(10, 100)
		);

		assert_ok!(CurrenciesModule::deposit(LDOT, &ALICE, 500));
		assert_eq!(
			StakingPoolModule::liquid_exchange_rate(),
			ExchangeRate::saturating_from_rational(1500, 500)
		);

		assert_ok!(CurrenciesModule::deposit(LDOT, &BOB, 300));
		assert_eq!(
			StakingPoolModule::liquid_exchange_rate(),
			ExchangeRate::saturating_from_rational(1500, 800)
		);
	});
}

#[test]
fn get_available_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		Unbondings::<Runtime>::insert(ALICE, 1, 300);
		Unbondings::<Runtime>::insert(ALICE, 2, 200);
		Unbondings::<Runtime>::insert(ALICE, 3, 50);
		Unbondings::<Runtime>::insert(ALICE, 4, 500);

		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 0);

		CurrentEra::<Runtime>::put(1);
		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 300);

		CurrentEra::<Runtime>::put(3);
		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 550);
	});
}

#[test]
fn set_staking_pool_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			StakingPoolModule::set_staking_pool_params(
				Origin::signed(5),
				ChangeRatio::NoChange,
				ChangeRatio::NoChange,
				ChangeRatio::NoChange,
				ChangeRate::NoChange,
				ChangeRate::NoChange
			),
			BadOrigin
		);

		assert_eq!(
			StakingPoolModule::staking_pool_params().target_max_free_unbonded_ratio,
			Ratio::saturating_from_rational(10, 100)
		);
		assert_ok!(StakingPoolModule::set_staking_pool_params(
			Origin::signed(One::get()),
			ChangeRatio::NewValue(Ratio::saturating_from_rational(15, 100)),
			ChangeRatio::NoChange,
			ChangeRatio::NoChange,
			ChangeRate::NoChange,
			ChangeRate::NoChange
		));
		assert_eq!(
			StakingPoolModule::staking_pool_params().target_max_free_unbonded_ratio,
			Ratio::saturating_from_rational(15, 100)
		);

		assert_noop!(
			StakingPoolModule::set_staking_pool_params(
				Origin::signed(One::get()),
				ChangeRatio::NoChange,
				ChangeRatio::NewValue(Ratio::saturating_from_rational(16, 100)),
				ChangeRatio::NoChange,
				ChangeRate::NoChange,
				ChangeRate::NoChange
			),
			Error::<Runtime>::InvalidConfig
		);
	});
}

#[test]
fn mint_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 0,
				free_pool: 0,
				unbonding_to_free: 0,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(StakingPoolModule::mint(&ALICE, 500), Ok(5000));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 5000);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 0,
				free_pool: 500,
				unbonding_to_free: 0,
				to_unbond_next_era: (0, 0)
			}
		);
		System::assert_last_event(Event::StakingPoolModule(crate::Event::MintLiquid(ALICE, 500, 5000)));

		RebalancePhase::<Runtime>::put(Phase::Started);
		assert_noop!(
			StakingPoolModule::mint(&ALICE, 500),
			Error::<Runtime>::RebalanceUnfinished
		);
	});
}

#[test]
fn withdraw_redemption_work() {
	ExtBuilder::default().build().execute_with(|| {
		Unbondings::<Runtime>::insert(ALICE, StakingPoolModule::current_era(), 200);
		assert_ok!(CurrenciesModule::deposit(DOT, &StakingPoolModule::account_id(), 500));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(
			StakingPoolModule::unbondings(&ALICE, StakingPoolModule::current_era()),
			200
		);

		assert_eq!(StakingPoolModule::withdraw_redemption(&ALICE), Ok(200));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1200);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			300
		);
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 0), 0);
	});
}

#[test]
fn redeem_by_unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::transfer_to_bridge(
			&StakingPoolModule::account_id(),
			500
		));
		assert_ok!(StakingPoolModule::bond_extra(500));
		StakingPoolLedger::<Runtime>::mutate(|ledger| {
			ledger.free_pool = ledger.free_pool.saturating_sub(500);
			ledger.bonded = ledger.bonded.saturating_add(500);
		});
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_unbond(&ALICE, 5000),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);

		assert_eq!(CurrenciesModule::total_issuance(LDOT), 10000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 0,
				free_pool: 500,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(StakingPoolModule::next_era_unbonds(&ALICE), 0);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&ALICE, 1000));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByUnbond(ALICE, 1000, 100)));
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 9000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 0,
				free_pool: 500,
				to_unbond_next_era: (100, 100)
			}
		);
		assert_eq!(StakingPoolModule::next_era_unbonds(&ALICE), 100);

		// over the communal_bonded
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 9000);
		assert_eq!(StakingPoolModule::next_era_unbonds(&BOB), 0);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&BOB, 9000));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByUnbond(BOB, 4000, 400)));
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 5000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 5000);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 0,
				free_pool: 500,
				to_unbond_next_era: (500, 500)
			}
		);
		assert_eq!(StakingPoolModule::next_era_unbonds(&BOB), 400);

		RebalancePhase::<Runtime>::put(Phase::Started);
		assert_noop!(
			StakingPoolModule::redeem_by_unbond(&BOB, 9000),
			Error::<Runtime>::RebalanceUnfinished
		);
	});
}

#[test]
fn redeem_by_free_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::transfer_to_bridge(
			&StakingPoolModule::account_id(),
			500
		));
		assert_ok!(StakingPoolModule::bond_extra(500));
		StakingPoolLedger::<Runtime>::mutate(|ledger| {
			ledger.free_pool = ledger.free_pool.saturating_sub(500);
			ledger.bonded = ledger.bonded.saturating_add(500);
		});
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_free_unbonded(&ALICE, 5000),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);

		assert_eq!(StakingPoolModule::staking_pool_ledger().free_pool, 500);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 10000);

		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&ALICE, 1000));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByFreeUnbonded(
			ALICE, 1000, 80, 20,
		)));
		assert_eq!(StakingPoolModule::staking_pool_ledger().free_pool, 420);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			420
		);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1080);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 9000);
		assert_eq!(CurrenciesModule::free_balance(DOT, &BOB), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 9000);

		// when overflow available
		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&BOB, 9000));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByFreeUnbonded(
			BOB, 3662, 300, 74,
		)));
		assert_eq!(StakingPoolModule::staking_pool_ledger().free_pool, 120);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			120
		);
		assert_eq!(CurrenciesModule::free_balance(DOT, &BOB), 300);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 5338);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 5338);

		RebalancePhase::<Runtime>::put(Phase::Started);
		assert_noop!(
			StakingPoolModule::redeem_by_free_unbonded(&BOB, 9000),
			Error::<Runtime>::RebalanceUnfinished
		);
	});
}

#[test]
fn redeem_by_claim_unbonding_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CurrenciesModule::transfer(Origin::signed(ALICE), BOB, DOT, 1000));
		assert_eq!(StakingPoolModule::mint(&BOB, 2000), Ok(20000));
		assert_ok!(StakingPoolModule::transfer_to_bridge(
			&StakingPoolModule::account_id(),
			1000
		));
		assert_ok!(StakingPoolModule::bond_extra(1000));
		Unbonding::<Runtime>::insert(4, (500, 0, 0));
		StakingPoolLedger::<Runtime>::mutate(|ledger| {
			ledger.free_pool = ledger.free_pool.saturating_sub(1000);
			ledger.bonded = ledger.bonded.saturating_add(1000).saturating_sub(500);
			ledger.unbonding_to_free = ledger.free_pool.saturating_sub(500);
		});
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 500,
				free_pool: 1000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(StakingPoolModule::unbonding(4), (500, 0, 0));
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 4), 0);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 20000);

		assert_eq!(StakingPoolModule::current_era(), 0);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1000, BondingDuration::get() + 1),
			Error::<Runtime>::InvalidEra,
		);

		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1000, 4));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByClaimUnbonding(
			ALICE, 4, 1000, 80, 20,
		)));
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 420,
				free_pool: 1000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(StakingPoolModule::unbonding(4), (500, 80, 0));
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 4), 80);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 19000);

		// when overflow available
		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&BOB, 10000, 4));
		System::assert_last_event(Event::StakingPoolModule(crate::Event::RedeemByClaimUnbonding(
			BOB, 4, 3910, 316, 79,
		)));
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 500,
				unbonding_to_free: 104,
				free_pool: 1000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(StakingPoolModule::unbonding(4), (500, 396, 0));
		assert_eq!(StakingPoolModule::unbondings(&BOB, 4), 316);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 15090);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 15090);

		RebalancePhase::<Runtime>::put(Phase::Started);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&BOB, 10000, 4),
			Error::<Runtime>::RebalanceUnfinished
		);
	});
}

fn mock_rebalance_process(era: EraIndex) {
	StakingPoolModule::on_new_era(era);
	StakingPoolModule::on_initialize((era * 3).into()); // Started
	StakingPoolModule::on_initialize((era * 3 + 1).into()); // RelaychainUpdated
	StakingPoolModule::on_initialize((era * 3 + 2).into()); // LedgerUpdated
}

#[test]
fn rebalance_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CurrenciesModule::deposit(DOT, &ALICE, 100000));
		assert_eq!(StakingPoolModule::mint(&ALICE, 100000), Ok(1000000));

		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 0,
				active: 0,
				unlocking: vec![],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 0,
				unbonding_to_free: 0,
				free_pool: 100000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			100000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));

		mock_rebalance_process(1);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 90000,
				active: 90000,
				unlocking: vec![],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 90000,
				unbonding_to_free: 0,
				free_pool: 10000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (0, 0, 0));

		mock_rebalance_process(2);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 90900,
				active: 89891,
				unlocking: vec![PolkadotUnlockChunk { value: 1009, era: 6 }],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 89891,
				unbonding_to_free: 1009,
				free_pool: 10000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));

		mock_rebalance_process(3);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 91798,
				active: 89772,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 89772,
				unbonding_to_free: 2026,
				free_pool: 10000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (0, 0, 0));

		mock_rebalance_process(4);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 92695,
				active: 89643,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 89643,
				unbonding_to_free: 3052,
				free_pool: 10000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (0, 0, 0));

		mock_rebalance_process(5);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 93591,
				active: 90484,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 90484,
				unbonding_to_free: 3107,
				free_pool: 10000,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (0, 0, 0));

		mock_rebalance_process(6);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 94045,
				active: 90911,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 90911,
				unbonding_to_free: 3134,
				free_pool: 10450,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10450
		);
		assert_eq!(StakingPoolModule::unbonding(6), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (0, 0, 0));

		mock_rebalance_process(7);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 94862,
				active: 91700,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 },
					PolkadotUnlockChunk { value: 1045, era: 11 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 91700,
				unbonding_to_free: 3162,
				free_pool: 10541,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10541
		);
		assert_eq!(StakingPoolModule::unbonding(7), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (1045, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(12), (0, 0, 0));

		mock_rebalance_process(8);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 95687,
				active: 92498,
				unlocking: vec![
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 },
					PolkadotUnlockChunk { value: 1045, era: 11 },
					PolkadotUnlockChunk { value: 1053, era: 12 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 92498,
				unbonding_to_free: 3189,
				free_pool: 10632,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10632
		);
		assert_eq!(StakingPoolModule::unbonding(8), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (1045, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(12), (1053, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(13), (0, 0, 0));

		assert_ok!(StakingPoolModule::redeem_by_unbond(&ALICE, 2000));
		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1000, 11));
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 92498,
				unbonding_to_free: 3104,
				free_pool: 10632,
				to_unbond_next_era: (212, 212)
			}
		);
		assert_eq!(StakingPoolModule::next_era_unbonds(&ALICE), 212);
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 11), 85);
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 13), 0);

		mock_rebalance_process(9);
		assert_eq!(
			StakingPoolModule::relaychain_staking_ledger(),
			PolkadotStakingLedger {
				total: 96555,
				active: 93050,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1036, era: 10 },
					PolkadotUnlockChunk { value: 1045, era: 11 },
					PolkadotUnlockChunk { value: 1053, era: 12 },
					PolkadotUnlockChunk { value: 371, era: 13 }
				],
			}
		);
		assert_eq!(
			StakingPoolModule::staking_pool_ledger(),
			Ledger {
				bonded: 93050,
				unbonding_to_free: 3208,
				free_pool: 10687,
				to_unbond_next_era: (0, 0)
			}
		);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10687
		);
		assert_eq!(StakingPoolModule::unbonding(9), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (1045, 85, 0));
		assert_eq!(StakingPoolModule::unbonding(12), (1053, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(13), (371, 212, 212));
		assert_eq!(StakingPoolModule::next_era_unbonds(&ALICE), 0);
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 11), 85);
		assert_eq!(StakingPoolModule::unbondings(&ALICE, 13), 212);
	});
}
