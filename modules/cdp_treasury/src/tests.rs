//! Unit tests for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{CdpTreasuryModule, Currencies, ExtBuilder, Runtime, ALICE, AUSD};
use sp_runtime::traits::OnFinalize;

#[test]
fn on_system_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 1000);
	});
}

#[test]
fn on_system_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		CdpTreasuryModule::on_system_surplus(1000);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
	});
}

#[test]
fn on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_surplus(1000);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(300);
		assert_eq!(CdpTreasuryModule::debit_pool(), 300);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 700);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 700);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_system_debit(800);
		assert_eq!(CdpTreasuryModule::debit_pool(), 800);
		CdpTreasuryModule::on_finalize(3);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 100);
	});
}

#[test]
fn add_backed_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(CdpTreasuryModule::add_backed_debit(&ALICE, 1000));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 2000);
	});
}

#[test]
fn sub_backed_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &ALICE), 1000);
		assert_ok!(CdpTreasuryModule::sub_backed_debit(&ALICE, 1000));
		assert_eq!(Currencies::balance(AUSD, &ALICE), 0);
		assert_noop!(
			CdpTreasuryModule::sub_backed_debit(&ALICE, 1000),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);
	});
}
