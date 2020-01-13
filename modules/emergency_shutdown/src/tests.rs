//! Unit tests for the emergency shutdown module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CdpEngineModule, CdpTreasury, EmergencyShutdownModule, ExtBuilder, HonzonModule, Origin, Runtime, System,
	TestEvent, ALICE,
};

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(EmergencyShutdownModule::is_shutdown(), false);
		assert_eq!(HonzonModule::is_shutdown(), false);
		assert_eq!(CdpEngineModule::is_shutdown(), false);
		assert_eq!(CdpTreasury::is_shutdown(), false);
		assert_ok!(EmergencyShutdownModule::emergency_shutdown(Origin::ROOT));

		let shutdown_event = TestEvent::emergency_shutdown(RawEvent::Shutdown(1));
		assert!(System::events().iter().any(|record| record.event == shutdown_event));

		assert_eq!(EmergencyShutdownModule::is_shutdown(), true);
		assert_eq!(HonzonModule::is_shutdown(), true);
		assert_eq!(CdpEngineModule::is_shutdown(), true);
		assert_eq!(CdpTreasury::is_shutdown(), true);
		assert_noop!(
			EmergencyShutdownModule::emergency_shutdown(Origin::ROOT),
			Error::<Runtime>::AlreadyShutdown,
		);
	});
}

#[test]
fn open_collateral_refund_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(EmergencyShutdownModule::can_refund(), false);
		assert_noop!(
			EmergencyShutdownModule::open_collateral_refund(Origin::ROOT),
			Error::<Runtime>::MustAfterShutdown,
		);
		assert_ok!(EmergencyShutdownModule::emergency_shutdown(Origin::ROOT));
		CdpTreasury::on_system_surplus(100);
		assert_eq!(CdpTreasury::get_surplus_pool(), 100);
		assert_noop!(
			EmergencyShutdownModule::open_collateral_refund(Origin::ROOT),
			Error::<Runtime>::ExistSurplus,
		);
	});
}

#[test]
fn open_collateral_refund_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(EmergencyShutdownModule::can_refund(), false);
		assert_ok!(EmergencyShutdownModule::emergency_shutdown(Origin::ROOT));
		assert_ok!(EmergencyShutdownModule::open_collateral_refund(Origin::ROOT));

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

#[test]
fn cancel_auction_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EmergencyShutdownModule::cancel_auction(Origin::signed(ALICE), 0),
			Error::<Runtime>::MustAfterShutdown,
		);
	});
}
