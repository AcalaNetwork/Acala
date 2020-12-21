//! Unit tests for the currencies module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	alice, bob, erc20_address, AccountId, AdaptedBasicCurrency, Currencies, ExtBuilder, NativeCurrency, Origin,
	PalletBalances, Runtime, System, TestEvent, Tokens, ALICE, BOB, EVA, ID_1, NATIVE_CURRENCY_ID, X_TOKEN_ID,
};
use sp_core::H160;
use sp_runtime::traits::BadOrigin;

#[test]
fn multi_lockable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			Currencies::set_lock(ID_1, X_TOKEN_ID, &ALICE, 50);
			assert_eq!(Tokens::locks(&ALICE, X_TOKEN_ID).len(), 1);
			Currencies::set_lock(ID_1, NATIVE_CURRENCY_ID, &ALICE, 50);
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
		});
}

#[test]
fn multi_reservable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::total_issuance(NATIVE_CURRENCY_ID), 200);
			assert_eq!(Currencies::total_issuance(X_TOKEN_ID), 200);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 100);
			assert_eq!(NativeCurrency::free_balance(&ALICE), 100);

			assert_ok!(Currencies::reserve(X_TOKEN_ID, &ALICE, 30));
			assert_ok!(Currencies::reserve(NATIVE_CURRENCY_ID, &ALICE, 40));
			assert_eq!(Currencies::reserved_balance(X_TOKEN_ID, &ALICE), 30);
			assert_eq!(Currencies::reserved_balance(NATIVE_CURRENCY_ID, &ALICE), 40);
		});
}

#[test]
fn native_currency_lockable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			NativeCurrency::set_lock(ID_1, &ALICE, 10);
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
			NativeCurrency::remove_lock(ID_1, &ALICE);
			assert_eq!(PalletBalances::locks(&ALICE).len(), 0);
		});
}

#[test]
fn native_currency_reservable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::reserve(&ALICE, 50));
			assert_eq!(NativeCurrency::reserved_balance(&ALICE), 50);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_lockable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			AdaptedBasicCurrency::set_lock(ID_1, &ALICE, 10);
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
			AdaptedBasicCurrency::remove_lock(ID_1, &ALICE);
			assert_eq!(PalletBalances::locks(&ALICE).len(), 0);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_reservable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::reserve(&ALICE, 50));
			assert_eq!(AdaptedBasicCurrency::reserved_balance(&ALICE), 50);
		});
}

#[test]
fn multi_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer(Some(ALICE).into(), BOB, X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 150);
		});
}

#[test]
fn multi_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				X_TOKEN_ID, &ALICE, 50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 150);
		});
}

#[test]
fn native_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer_native_currency(Some(ALICE).into(), BOB, 50));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 50);
			assert_eq!(NativeCurrency::free_balance(&BOB), 150);

			assert_ok!(NativeCurrency::transfer(&ALICE, &BOB, 10));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 40);
			assert_eq!(NativeCurrency::free_balance(&BOB), 160);

			assert_eq!(Currencies::slash(NATIVE_CURRENCY_ID, &ALICE, 10), 0);
			assert_eq!(NativeCurrency::free_balance(&ALICE), 30);
			assert_eq!(NativeCurrency::total_issuance(), 190);
		});
}

#[test]
fn native_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::update_balance(&ALICE, 10));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 110);

			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				NATIVE_CURRENCY_ID,
				&ALICE,
				10
			));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 120);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_transfer() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::transfer(&ALICE, &BOB, 50));
			assert_eq!(PalletBalances::total_balance(&ALICE), 50);
			assert_eq!(PalletBalances::total_balance(&BOB), 150);

			// creation fee
			assert_ok!(AdaptedBasicCurrency::transfer(&ALICE, &EVA, 10));
			assert_eq!(PalletBalances::total_balance(&ALICE), 40);
			assert_eq!(PalletBalances::total_balance(&EVA), 10);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_deposit() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::deposit(&EVA, 50));
			assert_eq!(PalletBalances::total_balance(&EVA), 50);
			assert_eq!(PalletBalances::total_issuance(), 250);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_withdraw() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::withdraw(&ALICE, 100));
			assert_eq!(PalletBalances::total_balance(&ALICE), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_slash() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(AdaptedBasicCurrency::slash(&ALICE, 101), 1);
			assert_eq!(PalletBalances::total_balance(&ALICE), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_update_balance() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::update_balance(&ALICE, -10));
			assert_eq!(PalletBalances::total_balance(&ALICE), 90);
			assert_eq!(PalletBalances::total_issuance(), 190);
		});
}

#[test]
fn update_balance_call_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				ALICE,
				NATIVE_CURRENCY_ID,
				-10
			));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 90);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 100);
			assert_ok!(Currencies::update_balance(Origin::root(), ALICE, X_TOKEN_ID, 10));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 110);
		});
}

#[test]
fn update_balance_call_fails_if_not_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::update_balance(Some(ALICE).into(), ALICE, X_TOKEN_ID, 100),
			BadOrigin
		);
	});
}

#[test]
fn call_event_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			assert_ok!(Currencies::transfer(Some(ALICE).into(), BOB, X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 150);

			let transferred_event = TestEvent::currencies(RawEvent::Transferred(X_TOKEN_ID, ALICE, BOB, 50));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::transfer(
				X_TOKEN_ID, &ALICE, &BOB, 10
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 40);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 160);

			let transferred_event = TestEvent::currencies(RawEvent::Transferred(X_TOKEN_ID, ALICE, BOB, 10));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::deposit(
				X_TOKEN_ID, &ALICE, 100
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 140);

			let transferred_event = TestEvent::currencies(RawEvent::Deposited(X_TOKEN_ID, ALICE, 100));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::withdraw(
				X_TOKEN_ID, &ALICE, 20
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 120);

			let transferred_event = TestEvent::currencies(RawEvent::Withdrawn(X_TOKEN_ID, ALICE, 20));
			assert!(System::events().iter().any(|record| record.event == transferred_event));
		});
}

#[test]
fn erc20_total_issuance_works() {
	let erc20 = CurrencyId::ERC20(erc20_address());
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::total_issuance(erc20), u128::max_value());
	});
}

#[test]
fn erc20_free_balance_works() {
	ExtBuilder::default().build().execute_with(|| {
		// empty address
		assert_eq!(
			Currencies::free_balance(CurrencyId::ERC20(H160::default()), &alice()),
			0
		);
		assert_eq!(Currencies::free_balance(CurrencyId::ERC20(H160::default()), &bob()), 0);

		assert_eq!(
			Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value()
		);
		assert_eq!(Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &bob()), 0);
	});
}

#[test]
fn erc20_total_balance_works() {
	ExtBuilder::default().build().execute_with(|| {
		// empty address
		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(H160::default()), &alice()),
			0
		);
		assert_eq!(Currencies::total_balance(CurrencyId::ERC20(H160::default()), &bob()), 0);

		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value()
		);
		assert_eq!(Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &bob()), 0);
	});
}

#[test]
fn erc20_ensure_withdraw() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Currencies::ensure_can_withdraw(
			CurrencyId::ERC20(erc20_address()),
			&alice(),
			100
		));
		assert_eq!(
			Currencies::ensure_can_withdraw(CurrencyId::ERC20(erc20_address()), &bob(), 100),
			Err(Error::<Runtime>::BalanceTooLow.into()),
		);
		assert_ok!(Currencies::transfer(
			Origin::signed(alice()),
			bob(),
			CurrencyId::ERC20(erc20_address()),
			100
		));
		assert_ok!(Currencies::ensure_can_withdraw(
			CurrencyId::ERC20(erc20_address()),
			&bob(),
			100
		));
		assert_eq!(
			Currencies::ensure_can_withdraw(CurrencyId::ERC20(erc20_address()), &bob(), 101),
			Err(Error::<Runtime>::BalanceTooLow.into()),
		);
	});
}

#[test]
fn erc20_transfer_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Currencies::transfer(
			Origin::signed(alice()),
			bob(),
			CurrencyId::ERC20(erc20_address()),
			100
		));

		assert_eq!(
			Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &bob()),
			100
		);
		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &bob()),
			100
		);

		assert_eq!(
			Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value() - 100
		);
		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value() - 100
		);

		assert_ok!(Currencies::transfer(
			Origin::signed(bob()),
			alice(),
			CurrencyId::ERC20(erc20_address()),
			10
		));

		assert_eq!(Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &bob()), 90);
		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &bob()),
			90
		);

		assert_eq!(
			Currencies::free_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value() - 90
		);
		assert_eq!(
			Currencies::total_balance(CurrencyId::ERC20(erc20_address()), &alice()),
			u128::max_value() - 90
		);
	});
}

#[test]
fn erc20_transfer_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		// empty address
		assert!(Currencies::transfer(Origin::signed(alice()), bob(), CurrencyId::ERC20(H160::default()), 100).is_err());

		// bob can't transfer. bob balance 0
		assert!(Currencies::transfer(Origin::signed(bob()), alice(), CurrencyId::ERC20(erc20_address()), 1).is_err());
	});
}

#[test]
fn erc20_invalid_operation() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::deposit(CurrencyId::ERC20(H160::default()), &alice(), 1),
			Error::<Runtime>::ERC20InvalidOperation,
		);
		assert_noop!(
			Currencies::withdraw(CurrencyId::ERC20(H160::default()), &alice(), 1),
			Error::<Runtime>::ERC20InvalidOperation,
		);
		assert_eq!(
			Currencies::can_slash(CurrencyId::ERC20(H160::default()), &alice(), 1),
			false,
		);
		assert_eq!(Currencies::slash(CurrencyId::ERC20(H160::default()), &alice(), 1), 0,);
		assert_noop!(
			Currencies::update_balance(Origin::root(), alice(), CurrencyId::ERC20(H160::default()), 1),
			Error::<Runtime>::ERC20InvalidOperation,
		);
		assert_eq!(
			Currencies::set_lock(ID_1, CurrencyId::ERC20(H160::default()), &alice(), 1),
			(),
		);
		assert_eq!(
			Currencies::extend_lock(ID_1, CurrencyId::ERC20(H160::default()), &alice(), 1),
			(),
		);
		assert_eq!(
			Currencies::remove_lock(ID_1, CurrencyId::ERC20(H160::default()), &alice()),
			(),
		);
		assert_eq!(
			Currencies::can_reserve(CurrencyId::ERC20(H160::default()), &alice(), 1),
			false,
		);
		assert_eq!(
			Currencies::slash_reserved(CurrencyId::ERC20(H160::default()), &alice(), 1),
			0,
		);
		assert_eq!(
			Currencies::reserved_balance(CurrencyId::ERC20(H160::default()), &alice()),
			0,
		);
		assert_noop!(
			Currencies::reserve(CurrencyId::ERC20(H160::default()), &alice(), 1),
			Error::<Runtime>::ERC20InvalidOperation,
		);
		assert_eq!(
			Currencies::unreserve(CurrencyId::ERC20(H160::default()), &alice(), 1),
			0,
		);
		assert_noop!(
			Currencies::repatriate_reserved(
				CurrencyId::ERC20(H160::default()),
				&alice(),
				&bob(),
				1,
				BalanceStatus::Free
			),
			Error::<Runtime>::ERC20InvalidOperation,
		);
	});
}
