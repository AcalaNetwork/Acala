//! Unit tests for the accounts module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Accounts, Currencies, ExtBuilder, Origin, Runtime, TimeModule, ALICE, AUSD, BOB, BTC};
use sp_runtime::traits::OnFinalize;

#[test]
fn try_free_transfer_over_cap() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
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
