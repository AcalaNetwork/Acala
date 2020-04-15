//! Unit tests for the accounts module.

#![cfg(test)]

use super::*;
use frame_support::{
	assert_noop, assert_ok,
	weights::{DispatchClass, DispatchInfo},
};
use mock::{Accounts, Call, Currencies, ExtBuilder, Origin, Runtime, TimeModule, ACA, ALICE, AUSD, BOB};

#[test]
fn enable_free_transfer_require_deposit() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Accounts::enable_free_transfer(Origin::signed(BOB)),
			Error::<Runtime>::NotEnoughBalance
		);
	});
}

#[test]
fn enable_free_transfer_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), None);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), Some(true));
	});
}

#[test]
fn disable_free_transfers_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), Some(true));
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

const CALL: &<Runtime as system::Trait>::Call = &Call::Currencies(orml_currencies::Call::transfer(BOB, AUSD, 12));

const CALL2: &<Runtime as system::Trait>::Call =
	&Call::Currencies(orml_currencies::Call::transfer_native_currency(BOB, 12));

const INFO: DispatchInfo = DispatchInfo {
	weight: 1000,
	class: DispatchClass::Normal,
	pays_fee: true,
};

#[test]
fn charges_fee() {
	ExtBuilder::default().build().execute_with(|| {
		let fee = 42 + 23 * 2 + 1000; // base + len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL, INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000 - fee);

		let fee2 = 42 + 18 * 2 + 1000; // base + len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL2, INFO, 18)
				.unwrap()
				.priority,
			fee2
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000 - fee - fee2);
	});
}

#[test]
fn enabled_free_transaction_not_charges_fee() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL, INFO, 23)
				.unwrap()
				.priority,
			0
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000);
	});
}

#[test]
fn enabled_free_transaction_charges_tip() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(100)
				.validate(&ALICE, CALL, INFO, 23)
				.unwrap()
				.priority,
			100
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000 - 100);
	});
}

#[test]
fn enabled_free_transaction_charges_other_call() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		let fee = 42 + 23 * 2 + 1000; // base + len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL2, INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000 - fee);
	});
}

#[test]
fn enabled_free_transaction_charges_other_call_with_tip() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		let fee = 42 + 23 * 2 + 1000 + 100; // base + len * byte + weight + tip
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(100)
				.validate(&ALICE, CALL2, INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE,), 100000 - fee);
	});
}
