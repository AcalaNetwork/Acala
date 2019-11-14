//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use mock::{DebitsModule, ExtBuilder, ALICE, BOB, USD};
use srml_support::assert_ok;

#[test]
fn update_balance_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DebitsModule::update_balance(USD, &ALICE, 10));
	});
}

#[test]
fn deposit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DebitsModule::deposit(USD, &BOB, 5));
	});
}

#[test]
fn withdraw_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DebitsModule::withdraw(USD, &ALICE, 5));
	});
}
