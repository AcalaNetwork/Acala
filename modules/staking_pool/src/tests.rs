//! Unit tests for staking pool module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	BondingDuration, CurrenciesModule, ExtBuilder, Origin, Runtime, StakingPoolModule, Status, System, TestEvent,
	ALICE, BOB, BRIDGE_STATUS, DOT, LDOT,
};

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
fn staking_ledger_work() {
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
			StakingPoolModule::staking_ledger(),
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

		assert_eq!(StakingPoolModule::balance(), 1340 + 1000);
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

		CurrentEra::put(5);
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

		CurrentEra::put(3);
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
fn payout_nominator_work() {
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

		StakingPoolModule::payout_nominator();
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
fn get_communal_bonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));

		assert_eq!(StakingPoolModule::get_communal_bonded(), 800)
	});
}

#[test]
fn get_total_communal_balance_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));
		FreeUnbonded::put(300);
		UnbondingToFree::put(300);

		assert_eq!(StakingPoolModule::get_total_communal_balance(), 1400)
	});
}

#[test]
fn get_free_unbonded_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));
		FreeUnbonded::put(300);
		UnbondingToFree::put(300);

		assert_eq!(
			StakingPoolModule::get_free_unbonded_ratio(),
			Ratio::saturating_from_rational(300, 1400)
		);
	});
}

#[test]
fn get_unbonding_to_free_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));
		FreeUnbonded::put(300);
		UnbondingToFree::put(400);

		assert_eq!(
			StakingPoolModule::get_unbonding_to_free_ratio(),
			Ratio::saturating_from_rational(400, 1500)
		);
	});
}

#[test]
fn get_communal_bonded_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));
		FreeUnbonded::put(300);
		UnbondingToFree::put(400);

		assert_eq!(
			StakingPoolModule::get_communal_bonded_ratio(),
			Ratio::saturating_from_rational(800, 1500)
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

		TotalBonded::put(1000);
		NextEraUnbond::put((200, 200));
		FreeUnbonded::put(300);
		UnbondingToFree::put(400);

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
		ClaimedUnbond::<Runtime>::insert(ALICE, 1, 300);
		ClaimedUnbond::<Runtime>::insert(ALICE, 2, 200);
		ClaimedUnbond::<Runtime>::insert(ALICE, 3, 50);
		ClaimedUnbond::<Runtime>::insert(ALICE, 4, 500);

		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 0);

		CurrentEra::put(1);
		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 300);

		CurrentEra::put(3);
		assert_eq!(StakingPoolModule::get_available_unbonded(&ALICE), 550);
	});
}

#[test]
fn bond_to_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalBonded::put(1000);
		FreeUnbonded::put(300);
		assert_ok!(CurrenciesModule::deposit(DOT, &StakingPoolModule::account_id(), 300));
		assert_eq!(StakingPoolModule::total_bonded(), 1000);
		assert_eq!(StakingPoolModule::free_unbonded(), 300);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			300
		);

		assert_ok!(StakingPoolModule::bond_to_bridge(100));
		assert_eq!(StakingPoolModule::total_bonded(), 1100);
		assert_eq!(StakingPoolModule::free_unbonded(), 200);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			200
		);

		assert_noop!(
			StakingPoolModule::bond_to_bridge(300),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);
	});
}

#[test]
fn unbond_from_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		BRIDGE_STATUS.with(|v| {
			let mut old_map = v.borrow().clone();
			old_map.insert(
				1,
				Status {
					bonded: 1000,
					free: 0,
					unlocking: vec![],
				},
			);
			*v.borrow_mut() = old_map;
		});
		TotalBonded::put(1000);
		NextEraUnbond::put((300, 200));
		assert_eq!(StakingPoolModule::unbonding(4), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 0);

		StakingPoolModule::unbond_from_bridge(0);
		assert_eq!(
			*BRIDGE_STATUS
				.with(|v| v.borrow().clone())
				.get(&1)
				.unwrap_or(&Default::default()),
			Status {
				bonded: 700,
				free: 0,
				unlocking: vec![(4, 300)],
			}
		);
		assert_eq!(StakingPoolModule::next_era_unbond(), (0, 0));
		assert_eq!(StakingPoolModule::unbonding(4), (300, 200, 200));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 100);
		assert_eq!(StakingPoolModule::total_bonded(), 700);
	});
}

#[test]
fn mint_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::free_unbonded(), 0);
		assert_eq!(StakingPoolModule::mint(&ALICE, 500), Ok(5000));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 5000);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);

		let mint_liquid_event = TestEvent::staking_pool(RawEvent::MintLiquid(ALICE, 500, 5000));
		assert!(System::events().iter().any(|record| record.event == mint_liquid_event));
	});
}

#[test]
fn withdraw_redemption_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalClaimedUnbonded::put(500);
		ClaimedUnbond::<Runtime>::insert(ALICE, StakingPoolModule::current_era(), 200);
		assert_ok!(CurrenciesModule::deposit(DOT, &StakingPoolModule::account_id(), 500));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 0), 200);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 500);

		assert_eq!(StakingPoolModule::withdraw_redemption(&ALICE), Ok(200));
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1200);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			300
		);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 0), 0);
		assert_eq!(StakingPoolModule::total_claimed_unbonded(), 300);
	});
}

#[test]
fn redeem_by_unbond_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::bond_to_bridge(500));
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_unbond(&ALICE, 5000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (0, 0));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 500);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 1000);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			0
		);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&ALICE, 1000));
		let redeem_by_unbond_event_1 = TestEvent::staking_pool(RawEvent::RedeemByUnbond(ALICE, 1000, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_unbond_event_1));

		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (100, 100));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 400);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 900);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&ALICE, 0 + 1 + BondingDuration::get()),
			100
		);

		// over the communal_bonded
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 9000);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&BOB, 0 + 1 + BondingDuration::get()),
			0
		);

		assert_ok!(StakingPoolModule::redeem_by_unbond(&BOB, 9000));
		let redeem_by_unbond_event_2 = TestEvent::staking_pool(RawEvent::RedeemByUnbond(BOB, 4000, 400));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_unbond_event_2));

		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 5000);
		assert_eq!(StakingPoolModule::total_bonded(), 500);
		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(StakingPoolModule::next_era_unbond(), (500, 500));
		assert_eq!(StakingPoolModule::get_communal_bonded(), 0);
		assert_eq!(StakingPoolModule::get_total_communal_balance(), 500);
		assert_eq!(
			StakingPoolModule::claimed_unbond(&BOB, 0 + 1 + BondingDuration::get()),
			400
		);
	});
}

#[test]
fn redeem_by_free_unbonded_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(StakingPoolModule::mint(&BOB, 1000), Ok(10000));
		assert_ok!(StakingPoolModule::bond_to_bridge(500));
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		assert_noop!(
			StakingPoolModule::redeem_by_free_unbonded(&ALICE, 5000),
			Error::<Runtime>::LiquidCurrencyNotEnough,
		);

		assert_eq!(StakingPoolModule::free_unbonded(), 500);
		assert_eq!(CurrenciesModule::free_balance(DOT, &ALICE), 1000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			500
		);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 10000);

		assert_ok!(StakingPoolModule::redeem_by_free_unbonded(&ALICE, 1000));
		let redeem_by_free_unbonded_event_1 =
			TestEvent::staking_pool(RawEvent::RedeemByFreeUnbonded(ALICE, 1000, 80, 20));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_free_unbonded_event_1));

		assert_eq!(StakingPoolModule::free_unbonded(), 420);
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
		let redeem_by_free_unbonded_event_2 =
			TestEvent::staking_pool(RawEvent::RedeemByFreeUnbonded(BOB, 3662, 300, 74));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_free_unbonded_event_2));

		assert_eq!(StakingPoolModule::free_unbonded(), 120);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			120
		);
		assert_eq!(CurrenciesModule::free_balance(DOT, &BOB), 300);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 5338);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 5338);
	});
}

#[test]
fn redeem_by_claim_unbonding_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(CurrenciesModule::transfer(Origin::signed(ALICE), BOB, DOT, 1000));
		assert_eq!(StakingPoolModule::mint(&BOB, 2000), Ok(20000));
		assert_ok!(StakingPoolModule::bond_to_bridge(1000));
		assert_ok!(CurrenciesModule::transfer(Origin::signed(BOB), ALICE, LDOT, 1000));

		TotalBonded::mutate(|bonded| *bonded -= 500);
		Unbonding::insert(4, (500, 0, 0));
		UnbondingToFree::put(500);

		assert_eq!(StakingPoolModule::unbonding(4), (500, 0, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 500);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 1000);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 4), 0);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 20000);

		assert_eq!(StakingPoolModule::current_era(), 0);
		assert_noop!(
			StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 0, BondingDuration::get() + 1),
			Error::<Runtime>::InvalidEra,
		);

		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&ALICE, 1000, 4));
		let redeem_by_claimed_unbonding_event_1 =
			TestEvent::staking_pool(RawEvent::RedeemByClaimUnbonding(ALICE, 4, 1000, 80, 20));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_claimed_unbonding_event_1));

		assert_eq!(StakingPoolModule::unbonding(4), (500, 80, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 420);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &ALICE), 0);
		assert_eq!(StakingPoolModule::claimed_unbond(&ALICE, 4), 80);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 19000);

		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 19000);
		assert_eq!(StakingPoolModule::claimed_unbond(&BOB, 4), 0);

		// when overflow available
		assert_ok!(StakingPoolModule::redeem_by_claim_unbonding(&BOB, 10000, 4));
		let redeem_by_claimed_unbonding_event_2 =
			TestEvent::staking_pool(RawEvent::RedeemByClaimUnbonding(BOB, 4, 3910, 316, 79));
		assert!(System::events()
			.iter()
			.any(|record| record.event == redeem_by_claimed_unbonding_event_2));

		assert_eq!(StakingPoolModule::unbonding(4), (500, 396, 0));
		assert_eq!(StakingPoolModule::unbonding_to_free(), 104);
		assert_eq!(CurrenciesModule::free_balance(LDOT, &BOB), 15090);
		assert_eq!(StakingPoolModule::claimed_unbond(&BOB, 4), 316);
		assert_eq!(CurrenciesModule::total_issuance(LDOT), 15090);
	});
}

#[test]
fn rebalance_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CurrenciesModule::deposit(DOT, &ALICE, 100000));
		assert_eq!(StakingPoolModule::mint(&ALICE, 100000), Ok(1000000));

		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 0,
				active: 0,
				unlocking: vec![],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 100000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			100000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 0);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 0);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));

		CurrentEra::put(1);
		StakingPoolModule::rebalance(1);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 90000,
				active: 90000,
				unlocking: vec![],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 90000);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 0);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (0, 0, 0));

		CurrentEra::put(2);
		StakingPoolModule::rebalance(2);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 90900,
				active: 89891,
				unlocking: vec![PolkadotUnlockChunk { value: 1009, era: 6 },],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 89891);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 1009);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (0, 0, 0));

		CurrentEra::put(3);
		StakingPoolModule::rebalance(3);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 91798,
				active: 89772,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 89772);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 2026);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (0, 0, 0));

		CurrentEra::put(4);
		StakingPoolModule::rebalance(4);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 92695,
				active: 89643,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 89643);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 3052);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (0, 0, 0));

		CurrentEra::put(5);
		StakingPoolModule::rebalance(5);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 93591,
				active: 90484,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1009, era: 6 },
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10000);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10000
		);
		assert_eq!(StakingPoolModule::total_bonded(), 90484);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 3107);
		assert_eq!(StakingPoolModule::unbonding(5), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(6), (1009, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (0, 0, 0));

		CurrentEra::put(6);
		StakingPoolModule::rebalance(6);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 94045,
				active: 90911,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1017, era: 7 },
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10450);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10450
		);
		assert_eq!(StakingPoolModule::total_bonded(), 90911);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 3134);
		assert_eq!(StakingPoolModule::unbonding(6), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(7), (1017, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (0, 0, 0));

		CurrentEra::put(7);
		StakingPoolModule::rebalance(7);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 94862,
				active: 91700,
				unlocking: vec![
					PolkadotUnlockChunk { value: 1026, era: 8 },
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 },
					PolkadotUnlockChunk { value: 1045, era: 11 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10541);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10541
		);
		assert_eq!(StakingPoolModule::total_bonded(), 91700);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 3162);
		assert_eq!(StakingPoolModule::unbonding(7), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(8), (1026, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (1045, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(12), (0, 0, 0));

		CurrentEra::put(8);
		StakingPoolModule::rebalance(8);
		assert_eq!(
			StakingPoolModule::staking_ledger(),
			PolkadotStakingLedger {
				total: 95687,
				active: 92498,
				unlocking: vec![
					PolkadotUnlockChunk { value: 55, era: 9 },
					PolkadotUnlockChunk { value: 1036, era: 10 },
					PolkadotUnlockChunk { value: 1045, era: 11 },
					PolkadotUnlockChunk { value: 1053, era: 12 },
				],
			}
		);
		assert_eq!(StakingPoolModule::free_unbonded(), 10632);
		assert_eq!(
			CurrenciesModule::free_balance(DOT, &StakingPoolModule::account_id()),
			10632
		);
		assert_eq!(StakingPoolModule::total_bonded(), 92498);
		assert_eq!(StakingPoolModule::unbonding_to_free(), 3189);
		assert_eq!(StakingPoolModule::unbonding(8), (0, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(9), (55, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(10), (1036, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(11), (1045, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(12), (1053, 0, 0));
		assert_eq!(StakingPoolModule::unbonding(13), (0, 0, 0));
	});
}
