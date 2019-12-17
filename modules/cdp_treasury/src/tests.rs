//! Unit tests for the cdp treasury module.

#![cfg(test)]

use super::*;
use mock::{CdpTreasuryModule, Currencies, ExtBuilder, AUSD};
use sp_runtime::traits::OnFinalize;

#[test]
fn on_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_debit(1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 1000);
	});
}

#[test]
fn on_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		CdpTreasuryModule::on_surplus(1000);
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
		CdpTreasuryModule::on_surplus(1000);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		CdpTreasuryModule::on_finalize(1);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 1000);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_debit(300);
		assert_eq!(CdpTreasuryModule::debit_pool(), 300);
		CdpTreasuryModule::on_finalize(2);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 700);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 700);
		assert_eq!(CdpTreasuryModule::debit_pool(), 0);
		CdpTreasuryModule::on_debit(800);
		assert_eq!(CdpTreasuryModule::debit_pool(), 800);
		CdpTreasuryModule::on_finalize(3);
		assert_eq!(Currencies::balance(AUSD, &CdpTreasuryModule::account_id()), 0);
		assert_eq!(CdpTreasuryModule::surplus_pool(), 0);
		assert_eq!(CdpTreasuryModule::debit_pool(), 100);
	});
}
