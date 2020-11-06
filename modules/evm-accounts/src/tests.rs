//! Unit tests for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	alice, bob, bob_account_id, Balances, EvmAccountsModule, ExtBuilder, Origin, Runtime, System, TestEvent, ALICE, BOB,
};

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

		// claim another eth address
		assert_eq!(Balances::free_balance(&ALICE::get()), 0);
		assert_eq!(Balances::free_balance(&bob_account_id()), 100000);
		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE::get()),
			EvmAccountsModule::eth_address(&bob()),
			EvmAccountsModule::eth_sign(&bob(), &ALICE::get().encode(), &[][..])
		));
		assert!(
			!Accounts::<Runtime>::contains_key(EvmAccountsModule::eth_address(&alice()))
				&& Accounts::<Runtime>::contains_key(EvmAccountsModule::eth_address(&bob()))
				&& EvmAddresses::<Runtime>::contains_key(ALICE::get())
		);
		assert_eq!(Balances::free_balance(&ALICE::get()), 100000);
		assert_eq!(Balances::free_balance(&BOB::get()), 0);
	});
}

#[test]
fn claim_account_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
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
			Error::<Runtime>::EthAddressHasMapped
		);
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
	});
}
