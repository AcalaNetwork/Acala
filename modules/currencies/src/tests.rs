// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Unit tests for the currencies module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	alice_account, bob_account, deploy_contracts, erc20_address, eva_account, AccountId, AdaptedBasicCurrency,
	Currencies, Event, ExtBuilder, NativeCurrency, Origin, PalletBalances, Runtime, System, Tokens, EVM, ID_1,
	NATIVE_CURRENCY_ID, X_TOKEN_ID,
};
use sp_core::H160;
use sp_runtime::traits::BadOrigin;
use support::EVM as EVMTrait;

#[test]
fn multi_lockable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::set_lock(ID_1, X_TOKEN_ID, &alice_account(), 50));
			assert_eq!(Tokens::locks(&alice_account(), X_TOKEN_ID).len(), 1);
			assert_ok!(Currencies::set_lock(ID_1, NATIVE_CURRENCY_ID, &alice_account(), 50));
			assert_eq!(PalletBalances::locks(&alice_account()).len(), 1);
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
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 100);
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 100);

			assert_ok!(Currencies::reserve(X_TOKEN_ID, &alice_account(), 30));
			assert_ok!(Currencies::reserve(NATIVE_CURRENCY_ID, &alice_account(), 40));
			assert_eq!(Currencies::reserved_balance(X_TOKEN_ID, &alice_account()), 30);
			assert_eq!(Currencies::reserved_balance(NATIVE_CURRENCY_ID, &alice_account()), 40);
		});
}

#[test]
fn native_currency_lockable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::set_lock(ID_1, &alice_account(), 10));
			assert_eq!(PalletBalances::locks(&alice_account()).len(), 1);
			assert_ok!(NativeCurrency::remove_lock(ID_1, &alice_account()));
			assert_eq!(PalletBalances::locks(&alice_account()).len(), 0);
		});
}

#[test]
fn native_currency_reservable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::reserve(&alice_account(), 50));
			assert_eq!(NativeCurrency::reserved_balance(&alice_account()), 50);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_lockable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::set_lock(ID_1, &alice_account(), 10));
			assert_eq!(PalletBalances::locks(&alice_account()).len(), 1);
			assert_ok!(AdaptedBasicCurrency::remove_lock(ID_1, &alice_account()));
			assert_eq!(PalletBalances::locks(&alice_account()).len(), 0);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_reservable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::reserve(&alice_account(), 50));
			assert_eq!(AdaptedBasicCurrency::reserved_balance(&alice_account()), 50);
		});
}

#[test]
fn multi_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			<EVM as EVMTrait<AccountId>>::set_origin(alice_account());
			assert_ok!(Currencies::transfer(
				Some(alice_account()).into(),
				bob_account(),
				X_TOKEN_ID,
				50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob_account()), 150);
		});
}

#[test]
fn multi_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				X_TOKEN_ID,
				&alice_account(),
				50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 150);
		});
}

#[test]
fn native_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer_native_currency(
				Some(alice_account()).into(),
				bob_account(),
				50
			));
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 50);
			assert_eq!(NativeCurrency::free_balance(&bob_account()), 150);

			assert_ok!(NativeCurrency::transfer(&alice_account(), &bob_account(), 10));
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 40);
			assert_eq!(NativeCurrency::free_balance(&bob_account()), 160);

			assert_eq!(Currencies::slash(NATIVE_CURRENCY_ID, &alice_account(), 10), 0);
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 30);
			assert_eq!(NativeCurrency::total_issuance(), 190);
		});
}

#[test]
fn native_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::update_balance(&alice_account(), 10));
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 110);

			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				NATIVE_CURRENCY_ID,
				&alice_account(),
				10
			));
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 120);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_transfer() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::transfer(&alice_account(), &bob_account(), 50));
			assert_eq!(PalletBalances::total_balance(&alice_account()), 50);
			assert_eq!(PalletBalances::total_balance(&bob_account()), 150);

			// creation fee
			assert_ok!(AdaptedBasicCurrency::transfer(&alice_account(), &eva_account(), 10));
			assert_eq!(PalletBalances::total_balance(&alice_account()), 40);
			assert_eq!(PalletBalances::total_balance(&eva_account()), 10);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_deposit() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::deposit(&eva_account(), 50));
			assert_eq!(PalletBalances::total_balance(&eva_account()), 50);
			assert_eq!(PalletBalances::total_issuance(), 250);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_withdraw() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::withdraw(&alice_account(), 100));
			assert_eq!(PalletBalances::total_balance(&alice_account()), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_slash() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(AdaptedBasicCurrency::slash(&alice_account(), 101), 1);
			assert_eq!(PalletBalances::total_balance(&alice_account()), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_update_balance() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::update_balance(&alice_account(), -10));
			assert_eq!(PalletBalances::total_balance(&alice_account()), 90);
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
				alice_account(),
				NATIVE_CURRENCY_ID,
				-10
			));
			assert_eq!(NativeCurrency::free_balance(&alice_account()), 90);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 100);
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				alice_account(),
				X_TOKEN_ID,
				10
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 110);
		});
}

#[test]
fn update_balance_call_fails_if_not_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::update_balance(Some(alice_account()).into(), alice_account(), X_TOKEN_ID, 100),
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
			assert_ok!(Currencies::transfer(
				Some(alice_account()).into(),
				bob_account(),
				X_TOKEN_ID,
				50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob_account()), 150);

			let transferred_event = Event::currencies(crate::Event::Transferred(
				X_TOKEN_ID,
				alice_account(),
				bob_account(),
				50,
			));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::transfer(
				X_TOKEN_ID,
				&alice_account(),
				&bob_account(),
				10
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 40);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob_account()), 160);

			let transferred_event = Event::currencies(crate::Event::Transferred(
				X_TOKEN_ID,
				alice_account(),
				bob_account(),
				10,
			));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::deposit(
				X_TOKEN_ID,
				&alice_account(),
				100
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 140);

			let transferred_event = Event::currencies(crate::Event::Deposited(X_TOKEN_ID, alice_account(), 100));
			assert!(System::events().iter().any(|record| record.event == transferred_event));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::withdraw(
				X_TOKEN_ID,
				&alice_account(),
				20
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice_account()), 120);

			let transferred_event = Event::currencies(crate::Event::Withdrawn(X_TOKEN_ID, alice_account(), 20));
			assert!(System::events().iter().any(|record| record.event == transferred_event));
		});
}

#[test]
fn erc20_total_issuance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(Currencies::total_issuance(CurrencyId::Erc20(erc20_address())), 10000);
		});
}

#[test]
fn erc20_free_balance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			// empty address
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(H160::default()), &alice_account()),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				0
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				10000
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				0
			);
		});
}

#[test]
fn erc20_total_balance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			// empty address
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(H160::default()), &alice_account()),
				0
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(H160::default()), &bob_account()),
				0
			);

			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				10000
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				0
			);
		});
}

#[test]
fn erc20_ensure_withdraw_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice_account());
			assert_ok!(Currencies::ensure_can_withdraw(
				CurrencyId::Erc20(erc20_address()),
				&alice_account(),
				100
			));
			assert_eq!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob_account(), 100),
				Err(Error::<Runtime>::BalanceTooLow.into()),
			);
			assert_ok!(Currencies::transfer(
				Origin::signed(alice_account()),
				bob_account(),
				CurrencyId::Erc20(erc20_address()),
				100
			));
			assert_ok!(Currencies::ensure_can_withdraw(
				CurrencyId::Erc20(erc20_address()),
				&bob_account(),
				100
			));
			assert_eq!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob_account(), 101),
				Err(Error::<Runtime>::BalanceTooLow.into()),
			);
		});
}

#[test]
fn erc20_transfer_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice_account(), NATIVE_CURRENCY_ID, 100000),
			(bob_account(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			<EVM as EVMTrait<AccountId>>::set_origin(alice_account());
			<EVM as EVMTrait<AccountId>>::set_origin(bob_account());
			assert_ok!(Currencies::transfer(
				Origin::signed(alice_account()),
				bob_account(),
				CurrencyId::Erc20(erc20_address()),
				100
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				100
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				100
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 100
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 100
			);

			assert_ok!(Currencies::transfer(
				Origin::signed(bob_account()),
				alice_account(),
				CurrencyId::Erc20(erc20_address()),
				10
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				90
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				90
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 90
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 90
			);
		});
}

#[test]
fn erc20_transfer_should_fail() {
	ExtBuilder::default()
		.balances(vec![
			(alice_account(), NATIVE_CURRENCY_ID, 100000),
			(bob_account(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice_account());
			<EVM as EVMTrait<AccountId>>::set_origin(bob_account());
			// empty address
			assert!(Currencies::transfer(
				Origin::signed(alice_account()),
				bob_account(),
				CurrencyId::Erc20(H160::default()),
				100
			)
			.is_err());

			// bob can't transfer. bob balance 0
			assert!(Currencies::transfer(
				Origin::signed(bob_account()),
				alice_account(),
				CurrencyId::Erc20(erc20_address()),
				1
			)
			.is_err());
		});
}

#[test]
fn erc20_can_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::can_reserve(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				true
			);
		});
}

#[test]
fn erc20_slash_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::slash_reserved(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				1
			);
			assert_ok!(Currencies::reserve(
				CurrencyId::Erc20(erc20_address()),
				&alice_account(),
				100
			));
			assert_eq!(
				Currencies::slash_reserved(CurrencyId::Erc20(erc20_address()), &alice_account(), 10),
				10
			);
		});
}

#[test]
fn erc20_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);

			assert_ok!(Currencies::reserve(
				CurrencyId::Erc20(erc20_address()),
				&alice_account(),
				100
			));

			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				100
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 100
			);
		});
}

#[test]
fn erc20_unreserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				0
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice_account(), 0),
				0
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice_account(), 50),
				50
			);
			assert_ok!(Currencies::reserve(
				CurrencyId::Erc20(erc20_address()),
				&alice_account(),
				30
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 30
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				30
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice_account(), 15),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance - 15
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				15
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice_account(), 30),
				15
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				0
			);
		});
}

#[test]
fn erc20_should_not_slash() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::can_slash(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				false
			);
			// calling slash will return 0
			assert_eq!(
				Currencies::slash(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				0
			);
		});
}

#[test]
fn erc20_should_not_be_lockable() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_noop!(
				Currencies::set_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::extend_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::remove_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice_account()),
				Error::<Runtime>::Erc20InvalidOperation
			);
		});
}

#[test]
fn erc20_repatriate_reserved_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice_account(), NATIVE_CURRENCY_ID, 100000),
			(bob_account(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let bob_balance = 100;
			let alice_balance = 10000 - bob_balance;
			<EVM as EVMTrait<AccountId>>::set_origin(alice_account());
			assert_ok!(Currencies::transfer(
				Origin::signed(alice_account()),
				bob_account(),
				CurrencyId::Erc20(erc20_address()),
				bob_balance
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				0
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&alice_account(),
					&alice_account(),
					0,
					BalanceStatus::Free
				),
				Ok(0)
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&alice_account(),
					&alice_account(),
					50,
					BalanceStatus::Free
				),
				Ok(50)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				0
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				bob_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				0
			);
			assert_ok!(Currencies::reserve(
				CurrencyId::Erc20(erc20_address()),
				&bob_account(),
				50
			));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob_account(),
					&bob_account(),
					60,
					BalanceStatus::Reserved
				),
				Ok(10)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);

			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob_account(),
					&alice_account(),
					30,
					BalanceStatus::Reserved
				),
				Ok(0)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				30
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				20
			);

			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob_account(),
					&alice_account(),
					30,
					BalanceStatus::Free
				),
				Ok(10)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				alice_balance + 20
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice_account()),
				30
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				50
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob_account()),
				0
			);
		});
}

#[test]
fn erc20_invalid_operation() {
	ExtBuilder::default()
		.balances(vec![(alice_account(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_noop!(
				Currencies::deposit(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::withdraw(CurrencyId::Erc20(erc20_address()), &alice_account(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::update_balance(Origin::root(), alice_account(), CurrencyId::Erc20(erc20_address()), 1),
				Error::<Runtime>::Erc20InvalidOperation,
			);
		});
}
