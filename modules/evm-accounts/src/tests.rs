//! Unit tests for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{alice, bob, EvmAccountsModule, ExtBuilder, Origin, Runtime, System, TestEvent, ALICE, BOB};

#[test]
fn claim_account_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE::get()),
			EvmAccountsModule::eth_address(&alice()),
			EvmAccountsModule::eth_sign(&alice(), &ALICE::get().encode(), &[][..])
		));
		let event = TestEvent::evm_accounts(RawEvent::ClaimAccount(
			ALICE::get(),
			EvmAccountsModule::eth_address(&alice()),
		));
		assert!(System::events().iter().any(|record| record.event == event));
		assert!(
			Accounts::<Runtime>::contains_key(EvmAccountsModule::eth_address(&alice()))
				&& EvmAddresses::<Runtime>::contains_key(ALICE::get())
		);
	});
}

#[test]
fn claim_account_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE::get()),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&bob(), &ALICE::get().encode(), &vec![1][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE::get()),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&bob(), &BOB::get().encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE::get()),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&alice(), &ALICE::get().encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE::get()),
			EvmAccountsModule::eth_address(&alice()),
			EvmAccountsModule::eth_sign(&alice(), &ALICE::get().encode(), &[][..])
		));
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE::get()),
				EvmAccountsModule::eth_address(&alice()),
				EvmAccountsModule::eth_sign(&alice(), &ALICE::get().encode(), &[][..])
			),
			Error::<Runtime>::AccountIdHasMapped
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(BOB::get()),
				EvmAccountsModule::eth_address(&alice()),
				EvmAccountsModule::eth_sign(&alice(), &BOB::get().encode(), &[][..])
			),
			Error::<Runtime>::EthAddressHasMapped
		);
	});
}
