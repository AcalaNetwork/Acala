//! Unit tests for the accounts module.

#![cfg(test)]

use super::*;
use frame_support::{
	assert_noop, assert_ok,
	weights::{DispatchClass, DispatchInfo, Pays},
};
use mock::{
	Accounts, Call, Currencies, DEXModule, ExtBuilder, NewAccountDeposit, Origin, Runtime, System, TimeModule, ACA,
	ALICE, AUSD, BOB, BTC, CAROL,
};
use orml_traits::MultiCurrency;

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
fn try_record_free_transfer_when_no_lock() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::free_transfer_enabled_accounts(ALICE), None);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), false);
	});
}

#[test]
fn try_record_free_transfer_over_cap() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0]);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0]);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), false);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
	});
}

#[test]
fn remove_expired_entry() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(TimeModule::now(), 0);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![]);
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![0, 0, 0]);
		TimeModule::set_timestamp(100);
		assert_eq!(TimeModule::now(), 100);
		assert_eq!(Accounts::try_record_free_transfer(&ALICE), true);
		assert_eq!(Accounts::last_free_transfers(ALICE), vec![100]);
	});
}

const CALL: &<Runtime as system::Trait>::Call = &Call::Currencies(orml_currencies::Call::transfer(BOB, AUSD, 12));

const CALL2: &<Runtime as system::Trait>::Call =
	&Call::Currencies(orml_currencies::Call::transfer_native_currency(BOB, 12));

const INFO: DispatchInfo = DispatchInfo {
	weight: 1000,
	class: DispatchClass::Normal,
	pays_fee: Pays::Yes,
};

const POST_INFO: PostDispatchInfo = PostDispatchInfo {
	actual_weight: Some(800),
	pays_fee: Pays::Yes,
};

#[test]
fn charges_fee() {
	ExtBuilder::default().build().execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL, &INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), (100000 - fee).into());

		let fee2 = 18 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL2, &INFO, 18)
				.unwrap()
				.priority,
			fee2
		);
		assert_eq!(
			Currencies::free_balance(ACA, &ALICE),
			(100000 - fee - fee2).unique_saturated_into()
		);
	});
}

#[test]
fn enabled_free_transaction_not_charges_fee() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL, &INFO, 23)
				.unwrap()
				.priority,
			0
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000.unique_saturated_into());
	});
}

#[test]
fn enabled_free_transaction_charges_tip() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(100)
				.validate(&ALICE, CALL, &INFO, 23)
				.unwrap()
				.priority,
			100
		);
		assert_eq!(
			Currencies::free_balance(ACA, &ALICE),
			(100000 - 100).unique_saturated_into()
		);
	});
}

#[test]
fn enabled_free_transaction_charges_other_call() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		let fee = 23 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, CALL2, &INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(
			Currencies::free_balance(ACA, &ALICE),
			(100000 - fee).unique_saturated_into()
		);
	});
}

#[test]
fn enabled_free_transaction_charges_other_call_with_tip_and_native_currency_is_enough() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Accounts::enable_free_transfer(Origin::signed(ALICE)));

		let fee = 23 * 2 + 1000 + 100; // len * byte + weight + tip
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(100)
				.validate(&ALICE, CALL2, &INFO, 23)
				.unwrap()
				.priority,
			fee
		);
		assert_eq!(
			Currencies::free_balance(ACA, &ALICE),
			(100000 - fee).unique_saturated_into()
		);
	});
}

#[test]
fn charges_fee_when_pre_dispatch_and_native_currency_is_enough() {
	ExtBuilder::default().build().execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		assert!(ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, CALL, &INFO, 23)
			.is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);
	});
}

#[test]
fn refund_fee_according_to_actual_when_post_dispatch_and_native_currency_is_enough() {
	ExtBuilder::default().build().execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		let pre = ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

		let refund = 200; // 1000 - 800
		assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(pre, &INFO, &POST_INFO, 23, &Ok(())).is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);
	});
}

#[test]
fn open_account_successfully_when_transfer_native() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 200));
		assert_eq!(Accounts::is_explicit(&BOB), true);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 200 - 100);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			100
		);
	});
}

#[test]
fn open_account_failed_when_transfer_native() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::is_explicit(&Accounts::treasury_account_id()), false);
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 50));
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_eq!(Accounts::is_explicit(&Accounts::treasury_account_id()), true);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &Accounts::treasury_account_id()),
			50
		);
	});
}

#[test]
fn open_account_successfully_when_transfer_non_native() {
	ExtBuilder::default().build().execute_with(|| {
		// add liquidity to dex
		assert_ok!(DEXModule::add_liquidity(Origin::signed(ALICE), ACA, 10000, 100));
		assert_ok!(DEXModule::add_liquidity(Origin::signed(ALICE), BTC, 10, 200));
		assert_eq!(DEXModule::liquidity_pool(ACA).0, 10000);

		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(BTC, &ALICE, &BOB, 10));
		assert_eq!(Accounts::is_explicit(&BOB), true);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 1497);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &BOB), 9);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			100
		);
		assert_eq!(DEXModule::liquidity_pool(ACA).0, 8403);
	});
}

#[test]
fn open_account_failed_when_transfer_non_native() {
	ExtBuilder::default().build().execute_with(|| {
		// inject liquidity to dex
		assert_ok!(DEXModule::add_liquidity(Origin::signed(ALICE), ACA, 200, 100));
		assert_eq!(DEXModule::liquidity_pool(ACA).0, 200);

		assert_eq!(Accounts::is_explicit(&Accounts::treasury_account_id()), false);
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 99));
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_eq!(Accounts::is_explicit(&Accounts::treasury_account_id()), false);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(AUSD, &Accounts::treasury_account_id()),
			99
		);
	});
}

#[test]
fn close_account_failed_when_not_allowed_death() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 200));
		System::inc_ref(&BOB);
		assert_noop!(
			Accounts::close_account(Origin::signed(BOB), None),
			Error::<Runtime>::NonZeroRefCount,
		);
	});
}

#[test]
fn close_account_failed_when_still_has_active_reserved() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 200));
		assert_eq!(System::allow_death(&BOB), true);
		assert_ok!(<Currencies as MultiReservableCurrency<_>>::reserve(ACA, &BOB, 10));
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			10 + NewAccountDeposit::get(),
		);
		assert_noop!(
			Accounts::close_account(Origin::signed(BOB), None),
			Error::<Runtime>::StillHasActiveReserved,
		);

		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &CAROL, 200));
		assert_ok!(<Currencies as MultiCurrency<_>>::deposit(BTC, &CAROL, 10));
		assert_ok!(<Currencies as MultiReservableCurrency<_>>::reserve(BTC, &CAROL, 1));
		assert_eq!(System::allow_death(&CAROL), true);
		assert_noop!(
			Accounts::close_account(Origin::signed(CAROL), None),
			Error::<Runtime>::StillHasActiveReserved,
		);
	});
}

#[test]
fn close_account_and_does_not_specific_receiver() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 500));
		assert_eq!(Accounts::is_explicit(&BOB), true);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(BTC, &ALICE, &BOB, 300));
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 400);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			100
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &BOB), 300);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(AUSD, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(BTC, &Accounts::treasury_account_id()),
			0
		);

		assert_ok!(Accounts::close_account(Origin::signed(BOB), None));
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			0
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &BOB), 0);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &Accounts::treasury_account_id()),
			400
		);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &Accounts::treasury_account_id()),
			100
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(AUSD, &Accounts::treasury_account_id()),
			1000
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(BTC, &Accounts::treasury_account_id()),
			300
		);
	});
}

#[test]
fn close_account_and_specific_receiver() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 500));
		assert_eq!(Accounts::is_explicit(&BOB), true);
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(BTC, &ALICE, &BOB, 300));

		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 400);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			NewAccountDeposit::get()
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &BOB), 300);
		assert_eq!(Accounts::is_explicit(&CAROL), false);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &CAROL), 0);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &CAROL),
			0
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &CAROL), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &CAROL), 0);
		assert_eq!(Accounts::is_explicit(&Accounts::treasury_account_id()), false);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(ACA, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(AUSD, &Accounts::treasury_account_id()),
			0
		);
		assert_eq!(
			<Currencies as MultiCurrency<_>>::free_balance(BTC, &Accounts::treasury_account_id()),
			0
		);

		assert_ok!(Accounts::close_account(Origin::signed(BOB), Some(CAROL)));

		assert_eq!(Accounts::is_explicit(&BOB), false);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			0
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &BOB), 0);
		assert_eq!(Accounts::is_explicit(&CAROL), true);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &CAROL), 400);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &CAROL),
			100
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &CAROL), 1000);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(BTC, &CAROL), 300);
	});
}

#[test]
fn charges_fee_when_validate_and_native_is_not_enough() {
	ExtBuilder::default().build().execute_with(|| {
		// open account for BOB
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(ACA, &ALICE, &BOB, 100));
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));
		assert_eq!(Accounts::is_explicit(&BOB), true);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(
			<Currencies as MultiReservableCurrency<_>>::reserved_balance(ACA, &BOB),
			100
		);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);

		// add liquidity to DEX
		assert_ok!(DEXModule::add_liquidity(Origin::signed(ALICE), ACA, 10000, 1000));
		assert_eq!(DEXModule::liquidity_pool(ACA), (10000, 1000));

		let fee = 500 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&BOB, CALL2, &INFO, 500)
				.unwrap()
				.priority,
			fee
		);

		assert_eq!(Currencies::free_balance(ACA, &BOB), 7);
		assert_eq!(Currencies::free_balance(AUSD, &BOB), 749);
		assert_eq!(DEXModule::liquidity_pool(ACA), (10000 - 7 - 2000, 1251));
	});
}
