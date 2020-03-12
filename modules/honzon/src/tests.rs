//! Unit tests for the honzon module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CdpEngineModule, Currencies, ExtBuilder, HonzonModule, LoansModule, Origin, Runtime, System, TestEvent, ALICE,
	AUSD, BOB, BTC, CAROL, DOT,
};
use support::{Rate, Ratio};

#[test]
fn liquidate_unsafe_cdp_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 50);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			HonzonModule::liquidate(Origin::signed(CAROL), ALICE, BTC),
			Error::<Runtime>::LiquidateFailed,
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_ok!(HonzonModule::liquidate(Origin::signed(CAROL), ALICE, BTC));
		assert_eq!(Currencies::balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 0);
	});
}

#[test]
fn authorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));

		let authorization_event = TestEvent::honzon(RawEvent::Authorization(ALICE, BOB, BTC));
		assert!(System::events()
			.iter()
			.any(|record| record.event == authorization_event));

		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));
	});
}

#[test]
fn unauthorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));
		assert_ok!(HonzonModule::unauthorize(Origin::signed(ALICE), BTC, BOB));

		let unauthorization_event = TestEvent::honzon(RawEvent::UnAuthorization(ALICE, BOB, BTC));
		assert!(System::events()
			.iter()
			.any(|record| record.event == unauthorization_event));

		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::<Runtime>::NoAuthorization
		);
	});
}

#[test]
fn unauthorize_all_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), DOT, CAROL));
		assert_ok!(HonzonModule::unauthorize_all(Origin::signed(ALICE)));

		let unauthorization_all_event = TestEvent::honzon(RawEvent::UnAuthorizationAll(ALICE));
		assert!(System::events()
			.iter()
			.any(|record| record.event == unauthorization_all_event));

		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::<Runtime>::NoAuthorization
		);
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, DOT),
			Error::<Runtime>::NoAuthorization
		);
	});
}

#[test]
fn transfer_loan_from_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(HonzonModule::update_loan(Origin::signed(ALICE), BTC, 100, 50));
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::transfer_loan_from(Origin::signed(BOB), BTC, ALICE));
		assert_eq!(LoansModule::collaterals(BOB, BTC), 100);
		assert_eq!(LoansModule::debits(BTC, BOB).0, 50);
	});
}

#[test]
fn transfer_unauthorization_loans_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HonzonModule::transfer_loan_from(Origin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::NoAuthorization,
		);
	});
}

#[test]
fn update_loan_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(HonzonModule::update_loan(Origin::signed(ALICE), BTC, 100, 50));
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 50);
	});
}

#[test]
fn emergency_shutdown_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(HonzonModule::is_shutdown(), false);
		HonzonModule::emergency_shutdown();
		assert_eq!(HonzonModule::is_shutdown(), true);
		assert_noop!(
			HonzonModule::liquidate(Origin::signed(CAROL), ALICE, BTC),
			Error::<Runtime>::AlreadyShutdown,
		);
		assert_noop!(
			HonzonModule::update_loan(Origin::signed(ALICE), BTC, 100, 50),
			Error::<Runtime>::AlreadyShutdown,
		);
		assert_noop!(
			HonzonModule::transfer_loan_from(Origin::signed(ALICE), BTC, BOB),
			Error::<Runtime>::AlreadyShutdown,
		);
	});
}

#[test]
fn settle_cdp_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_noop!(
			HonzonModule::settle_cdp(Origin::signed(CAROL), ALICE, BTC),
			Error::<Runtime>::MustAfterShutdown,
		);
		HonzonModule::emergency_shutdown();
		assert_ok!(HonzonModule::settle_cdp(Origin::signed(CAROL), ALICE, BTC));
	});
}

#[test]
fn withdraw_collateral_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::update_loan(Origin::signed(ALICE), BTC, 100, 0));
		assert_noop!(
			HonzonModule::withdraw_collateral(Origin::signed(ALICE), BTC, 100),
			Error::<Runtime>::MustAfterShutdown,
		);
		HonzonModule::emergency_shutdown();
		assert_ok!(HonzonModule::withdraw_collateral(Origin::signed(ALICE), BTC, 100));
	});
}
