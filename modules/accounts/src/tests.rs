//! Unit tests for the accounts module.

#![cfg(test)]

use super::*;
use frame_support::assert_ok;
use mock::{Accounts, ExtBuilder, Origin, TimeModule, ALICE};

#[test]
fn enable_free_transfer_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), None);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), Some(()));
	});
}

#[test]
fn disable_free_transfers_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), Some(()));
		assert_ok!(Accounts::disable_free_transfers(Origin::signed(ALICE)));
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), None);
	});
}

#[test]
fn try_free_transfer_when_no_lock() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), None);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_eq!(Accounts::try_free_transfer(&ALICE), false);
	});
}

#[test]
fn try_free_transfer_over_cap() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0]);
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0]);
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
		assert_eq!(Accounts::try_free_transfer(&ALICE), false);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
	});
}

#[test]
fn remove_expired_entry() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
		assert_ok!(TimeModule::dispatch(pallet_timestamp::Call::set(100), Origin::NONE));
		assert_eq!(TimeModule::now(), 100);
		assert_eq!(Accounts::try_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![100]);
	});
}
