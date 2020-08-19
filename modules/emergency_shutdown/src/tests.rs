//! Unit tests for the emergency shutdown module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::traits::BadOrigin;

#[test]
fn emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(EmergencyShutdownModule::is_shutdown(), false);
		assert_noop!(
			EmergencyShutdownModule::emergency_shutdown(Origin::signed(5)),
			BadOrigin,
		);
		assert_ok!(EmergencyShutdownModule::emergency_shutdown(Origin::signed(1)));

		let shutdown_event = TestEvent::emergency_shutdown(RawEvent::Shutdown(1));
		assert!(System::events().iter().any(|record| record.event == shutdown_event));

		assert_eq!(EmergencyShutdownModule::is_shutdown(), true);
		assert_noop!(
			EmergencyShutdownModule::emergency_shutdown(Origin::signed(1)),
			Error::<Runtime>::AlreadyShutdown,
		);
	});
}

#[test]
fn open_collateral_refund_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(EmergencyShutdownModule::can_refund(), false);
		assert_noop!(
			EmergencyShutdownModule::open_collateral_refund(Origin::signed(1)),
			Error::<Runtime>::MustAfterShutdown,
		);
	});
}

#[test]
fn open_collateral_refund_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(EmergencyShutdownModule::can_refund(), false);
		assert_ok!(EmergencyShutdownModule::emergency_shutdown(Origin::signed(1)));
		assert_noop!(
			EmergencyShutdownModule::open_collateral_refund(Origin::signed(5)),
			BadOrigin,
		);
		assert_ok!(EmergencyShutdownModule::open_collateral_refund(Origin::signed(1)));

		let open_refund_event = TestEvent::emergency_shutdown(RawEvent::OpenRefund(1));
		assert!(System::events().iter().any(|record| record.event == open_refund_event));

		assert_eq!(EmergencyShutdownModule::can_refund(), true);
	});
}

#[test]
fn refund_collaterals_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EmergencyShutdownModule::refund_collaterals(Origin::signed(ALICE), 10),
			Error::<Runtime>::CanNotRefund,
		);
	});
}
