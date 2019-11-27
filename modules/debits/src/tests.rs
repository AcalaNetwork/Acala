//! Unit tests for the debits module.

#![cfg(test)]

use super::*;
use frame_support::assert_ok;
use mock::{Currencies, DebitsModule, ExtBuilder, ALICE, AUSD};

#[test]
fn update_balance_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(DebitsModule::update_balance(AUSD, &ALICE, 100));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1050);
	});
}

#[test]
fn deposit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(DebitsModule::deposit(AUSD, &ALICE, 100));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1050);
	});
}

#[test]
fn withdraw_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(DebitsModule::withdraw(AUSD, &ALICE, 100));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 950);
	});
}
