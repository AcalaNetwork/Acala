//! Unit tests for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{alice, bob, eth, sig, EvmAccounts, ExtBuilder, Origin, Runtime, System, TestEvent, ALICE, BOB};

#[test]
fn claim_account_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EvmAccounts::claim_account(
			Origin::signed(ALICE::get()),
			eth(&alice()),
			sig::<Runtime>(&alice(), &ALICE::get().encode(), &[][..])
		));
		let event = TestEvent::evm_accounts(RawEvent::ClaimAccount(ALICE::get(), eth(&alice())));
		assert!(System::events().iter().any(|record| record.event == event));
	});
}

#[test]
fn claim_account_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EvmAccounts::claim_account(
				Origin::signed(ALICE::get()),
				eth(&alice()),
				sig::<Runtime>(&alice(), &ALICE::get().encode(), &vec![1][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccounts::claim_account(
				Origin::signed(ALICE::get()),
				eth(&alice()),
				sig::<Runtime>(&alice(), &BOB::get().encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccounts::claim_account(
				Origin::signed(ALICE::get()),
				eth(&bob()),
				sig::<Runtime>(&alice(), &ALICE::get().encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
	});
}
