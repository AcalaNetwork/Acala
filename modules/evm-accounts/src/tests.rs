//! Unit tests for the evm-accounts module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{alice, bob, Event, EvmAccountsModule, ExtBuilder, Origin, Runtime, System, ALICE, BOB};
use std::str::FromStr;

#[test]
fn claim_account_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE),
			EvmAccountsModule::eth_address(&alice()),
			EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
		));
		let event = Event::evm_accounts(crate::Event::ClaimAccount(
			ALICE,
			EvmAccountsModule::eth_address(&alice()),
		));
		assert!(System::events().iter().any(|record| record.event == event));
		assert!(
			Accounts::<Runtime>::contains_key(EvmAccountsModule::eth_address(&alice()))
				&& EvmAddresses::<Runtime>::contains_key(ALICE)
		);
	});
}

#[test]
fn claim_account_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&bob(), &ALICE.encode(), &vec![1][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&bob(), &BOB.encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE),
				EvmAccountsModule::eth_address(&bob()),
				EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
			),
			Error::<Runtime>::InvalidSignature
		);
		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE),
			EvmAccountsModule::eth_address(&alice()),
			EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
		));
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE),
				EvmAccountsModule::eth_address(&alice()),
				EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
			),
			Error::<Runtime>::AccountIdHasMapped
		);
		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(BOB),
				EvmAccountsModule::eth_address(&alice()),
				EvmAccountsModule::eth_sign(&alice(), &BOB.encode(), &[][..])
			),
			Error::<Runtime>::EthAddressHasMapped
		);
	});
}

#[test]
fn evm_get_account_id() {
	ExtBuilder::default().build().execute_with(|| {
		let evm_account = EvmAccountsModule::eth_address(&alice());
		let evm_account_to_default = {
			let mut bytes = *b"evm:aaaaaaaaaaaaaaaaaaaa\0\0\0\0\0\0\0\0";
			bytes[4..24].copy_from_slice(&evm_account[..]);
			AccountId32::from(bytes)
		};
		assert_eq!(
			EvmAddressMapping::<Runtime>::get_account_id(&evm_account),
			evm_account_to_default
		);

		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE),
			EvmAccountsModule::eth_address(&alice()),
			EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
		));

		assert_eq!(EvmAddressMapping::<Runtime>::get_account_id(&evm_account), ALICE);
		assert_eq!(
			EvmAddressMapping::<Runtime>::get_evm_address(&ALICE).unwrap(),
			evm_account
		);

		assert!(EvmAddressMapping::<Runtime>::is_linked(
			&evm_account_to_default,
			&evm_account
		));
		assert!(EvmAddressMapping::<Runtime>::is_linked(&ALICE, &evm_account));
	});
}

#[test]
fn account_to_evm() {
	ExtBuilder::default().build().execute_with(|| {
		let default_evm_account = EvmAddress::from_str("f0bd9ffde7f9f4394d8cc1d86bf24d87e5d5a9a9").unwrap();
		assert_eq!(EvmAddressMapping::<Runtime>::get_evm_address(&ALICE), None);

		let alice_evm_account = EvmAccountsModule::eth_address(&alice());

		assert_ok!(EvmAccountsModule::claim_account(
			Origin::signed(ALICE),
			alice_evm_account,
			EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
		));

		assert_eq!(EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_account), ALICE);
		assert_eq!(
			EvmAddressMapping::<Runtime>::get_evm_address(&ALICE).unwrap(),
			alice_evm_account
		);

		assert_eq!(
			EvmAddressMapping::<Runtime>::get_or_create_evm_address(&ALICE),
			alice_evm_account
		);

		assert!(EvmAddressMapping::<Runtime>::is_linked(&ALICE, &alice_evm_account));
		assert!(EvmAddressMapping::<Runtime>::is_linked(&ALICE, &default_evm_account));
	});
}

#[test]
fn account_to_evm_with_create_default() {
	ExtBuilder::default().build().execute_with(|| {
		let default_evm_account = EvmAddress::from_str("f0bd9ffde7f9f4394d8cc1d86bf24d87e5d5a9a9").unwrap();
		assert_eq!(
			EvmAddressMapping::<Runtime>::get_or_create_evm_address(&ALICE),
			default_evm_account
		);
		assert_eq!(
			EvmAddressMapping::<Runtime>::get_evm_address(&ALICE),
			Some(default_evm_account)
		);

		assert_eq!(
			EvmAddressMapping::<Runtime>::get_account_id(&default_evm_account),
			ALICE
		);

		assert!(EvmAddressMapping::<Runtime>::is_linked(&ALICE, &default_evm_account));

		let alice_evm_account = EvmAccountsModule::eth_address(&alice());

		assert_noop!(
			EvmAccountsModule::claim_account(
				Origin::signed(ALICE),
				alice_evm_account,
				EvmAccountsModule::eth_sign(&alice(), &ALICE.encode(), &[][..])
			),
			Error::<Runtime>::AccountIdHasMapped
		);
	});
}
