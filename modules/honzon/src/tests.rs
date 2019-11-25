//! Unit tests for the tokens module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{CdpEngineModule, ExtBuilder, HonzonModule, Origin, VaultsModule, ALICE, ALIEX, BOB, BTC, DOT};
use sr_primitives::Permill;
use support::Ratio;

#[test]
fn authorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));
	});
}

#[test]
fn unauthorize_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::check_authorization(&ALICE, &BOB, BTC));

		assert_ok!(HonzonModule::unauthorize(Origin::signed(ALICE), BTC, BOB));
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::NoAuthorization
		);
	});
}

#[test]
fn unauthorize_all_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), BTC, BOB));
		assert_ok!(HonzonModule::authorize(Origin::signed(ALICE), DOT, ALIEX));
		assert_ok!(HonzonModule::unauthorize_all(Origin::signed(ALICE)));
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, BTC),
			Error::NoAuthorization
		);
		assert_noop!(
			HonzonModule::check_authorization(&ALICE, &BOB, DOT),
			Error::NoAuthorization
		);
	});
}

#[test]
fn transfer_vault_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_ok!(HonzonModule::update_vault(Origin::signed(ALICE), BTC, 100, 50));
		assert_ok!(HonzonModule::authorize(Origin::signed(BOB), BTC, ALICE));
		assert_ok!(HonzonModule::transfer_vault(Origin::signed(ALICE), BTC, BOB));
		assert_eq!(VaultsModule::collaterals(BOB, BTC), 100);
		assert_eq!(VaultsModule::debits(BOB, BTC), 50);
	});
}

#[test]
fn transfer_unauthorization_vaults_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HonzonModule::transfer_vault(Origin::signed(ALICE), BTC, BOB),
			"NoAuthorization"
		);
	});
}

#[test]
fn update_vault_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		CdpEngineModule::set_collateral_params(
			BTC,
			Some(Some(Permill::from_parts(1))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Permill::from_percent(20))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		);
		assert_ok!(HonzonModule::update_vault(Origin::signed(ALICE), BTC, 100, 50));
		assert_eq!(VaultsModule::collaterals(ALICE, BTC), 100);
		assert_eq!(VaultsModule::debits(ALICE, BTC), 50);
	});
}
