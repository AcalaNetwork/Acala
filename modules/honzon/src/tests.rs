//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use mock::{ExtBuilder, HonzonModule, ALICE, BOB, ALIEX, STABLE_COIN_ID, X_TOKEN_ID, Y_TOKEN_ID};
use palette_support::{assert_noop, assert_ok};

#[test]
fn authorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorization(ALICE, BOB, Y_TOKEN_ID));
		assert_eq!(HonzonModule::check_authorization(ALICE, BOB, X_TOKEN_ID), true);
	});
}

#[test]
fn unauthorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorization(ALICE, BOB, Y_TOKEN_ID));
		assert_eq!(HonzonModule::check_authorization(ALICE, BOB, X_TOKEN_ID), true);

		assert_ok!(HonzonModule::unauthorize(ALICE, BOB, X_TOKEN_ID));
		assert_eq!(HonzonModule::check_authorization(ALICE, BOB, X_TOKEN_ID), false);
	});
}

#[test]
fn unauthorize_all_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorization(ALICE, BOB, X_TOKEN_ID));
		assert_ok!(HonzonModule::authorization(ALICE, ALIEX, Y_TOKEN_ID));
		assert_ok!(HonzonModule::unauthorize_all(ALICE));
		assert_eq!(HonzonModule::check_authorization(ALICE, BOB, X_TOKEN_ID), false);
		assert_eq!(HonzonModule::check_authorization(ALICE, ALICE, Y_TOKEN_ID), false);
	});
}

#[test]
fn transfer_vaults_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorization(BOB, ALICE, X_TOKEN_ID));
		assert_eq!(HonzonModule::transfer_vaults(ALICE, BOB, X_TOKEN_ID));
	});
}

#[test]
fn transfer_unauthorization_vaults_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			assert_eq!(HonzonModule::transfer_vaults(ALICE, BOB, X_TOKEN_ID));
			Error::NoAuthorization
		);
	});
}

#[test]
fn update_vault_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::update_vault(ALICE, X_TOKEN_ID, 1, 1));
	});
}
