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
	alice, bob, deploy_contracts, erc20_address, eva, AccountId, AdaptedBasicCurrency, Currencies, Event, ExtBuilder,
	NativeCurrency, Origin, PalletBalances, Runtime, System, Tokens, EVM, ID_1, NATIVE_CURRENCY_ID, X_TOKEN_ID,
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
			assert_ok!(Currencies::set_lock(ID_1, X_TOKEN_ID, &alice(), 50));
			assert_eq!(Tokens::locks(&alice(), X_TOKEN_ID).len(), 1);
			assert_ok!(Currencies::set_lock(ID_1, NATIVE_CURRENCY_ID, &alice(), 50));
			assert_eq!(PalletBalances::locks(&alice()).len(), 1);
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
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 100);
			assert_eq!(NativeCurrency::free_balance(&alice()), 100);

			assert_ok!(Currencies::reserve(X_TOKEN_ID, &alice(), 30));
			assert_ok!(Currencies::reserve(NATIVE_CURRENCY_ID, &alice(), 40));
			assert_eq!(Currencies::reserved_balance(X_TOKEN_ID, &alice()), 30);
			assert_eq!(Currencies::reserved_balance(NATIVE_CURRENCY_ID, &alice()), 40);
		});
}

#[test]
fn native_currency_lockable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::set_lock(ID_1, &alice(), 10));
			assert_eq!(PalletBalances::locks(&alice()).len(), 1);
			assert_ok!(NativeCurrency::remove_lock(ID_1, &alice()));
			assert_eq!(PalletBalances::locks(&alice()).len(), 0);
		});
}

#[test]
fn native_currency_reservable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::reserve(&alice(), 50));
			assert_eq!(NativeCurrency::reserved_balance(&alice()), 50);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_lockable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::set_lock(ID_1, &alice(), 10));
			assert_eq!(PalletBalances::locks(&alice()).len(), 1);
			assert_ok!(AdaptedBasicCurrency::remove_lock(ID_1, &alice()));
			assert_eq!(PalletBalances::locks(&alice()).len(), 0);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_reservable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::reserve(&alice(), 50));
			assert_eq!(AdaptedBasicCurrency::reserved_balance(&alice()), 50);
		});
}

#[test]
fn multi_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_ok!(Currencies::transfer(Some(alice()).into(), bob(), X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob()), 150);
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
				&alice(),
				50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 150);
		});
}

#[test]
fn native_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer_native_currency(Some(alice()).into(), bob(), 50));
			assert_eq!(NativeCurrency::free_balance(&alice()), 50);
			assert_eq!(NativeCurrency::free_balance(&bob()), 150);

			assert_ok!(NativeCurrency::transfer(&alice(), &bob(), 10));
			assert_eq!(NativeCurrency::free_balance(&alice()), 40);
			assert_eq!(NativeCurrency::free_balance(&bob()), 160);

			assert_eq!(Currencies::slash(NATIVE_CURRENCY_ID, &alice(), 10), 0);
			assert_eq!(NativeCurrency::free_balance(&alice()), 30);
			assert_eq!(NativeCurrency::total_issuance(), 190);
		});
}

#[test]
fn native_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::update_balance(&alice(), 10));
			assert_eq!(NativeCurrency::free_balance(&alice()), 110);

			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				NATIVE_CURRENCY_ID,
				&alice(),
				10
			));
			assert_eq!(NativeCurrency::free_balance(&alice()), 120);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_transfer() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::transfer(&alice(), &bob(), 50));
			assert_eq!(PalletBalances::total_balance(&alice()), 50);
			assert_eq!(PalletBalances::total_balance(&bob()), 150);

			// creation fee
			assert_ok!(AdaptedBasicCurrency::transfer(&alice(), &eva(), 10));
			assert_eq!(PalletBalances::total_balance(&alice()), 40);
			assert_eq!(PalletBalances::total_balance(&eva()), 10);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_deposit() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::deposit(&eva(), 50));
			assert_eq!(PalletBalances::total_balance(&eva()), 50);
			assert_eq!(PalletBalances::total_issuance(), 250);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_withdraw() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::withdraw(&alice(), 100));
			assert_eq!(PalletBalances::total_balance(&alice()), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_slash() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(AdaptedBasicCurrency::slash(&alice(), 101), 1);
			assert_eq!(PalletBalances::total_balance(&alice()), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_update_balance() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::update_balance(&alice(), -10));
			assert_eq!(PalletBalances::total_balance(&alice()), 90);
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
				alice(),
				NATIVE_CURRENCY_ID,
				-10
			));
			assert_eq!(NativeCurrency::free_balance(&alice()), 90);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 100);
			assert_ok!(Currencies::update_balance(Origin::root(), alice(), X_TOKEN_ID, 10));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 110);
		});
}

#[test]
fn update_balance_call_fails_if_not_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::update_balance(Some(alice()).into(), alice(), X_TOKEN_ID, 100),
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
			assert_ok!(Currencies::transfer(Some(alice()).into(), bob(), X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob()), 150);
			System::assert_last_event(Event::Currencies(crate::Event::Transferred(
				X_TOKEN_ID,
				alice(),
				bob(),
				50,
			)));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::transfer(
				X_TOKEN_ID,
				&alice(),
				&bob(),
				10
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 40);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob()), 160);
			System::assert_last_event(Event::Currencies(crate::Event::Transferred(
				X_TOKEN_ID,
				alice(),
				bob(),
				10,
			)));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::deposit(
				X_TOKEN_ID,
				&alice(),
				100
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 140);
			System::assert_last_event(Event::Currencies(crate::Event::Deposited(X_TOKEN_ID, alice(), 100)));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::withdraw(
				X_TOKEN_ID,
				&alice(),
				20
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 120);
			System::assert_last_event(Event::Currencies(crate::Event::Withdrawn(X_TOKEN_ID, alice(), 20)));
		});
}

#[test]
fn erc20_total_issuance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(Currencies::total_issuance(CurrencyId::Erc20(erc20_address())), 10000);
		});
}

#[test]
fn erc20_free_balance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			// empty address
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(H160::default()), &alice()),
				0
			);
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 0);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				10000
			);
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 0);
		});
}

#[test]
fn erc20_total_balance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			// empty address
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(H160::default()), &alice()),
				0
			);
			assert_eq!(Currencies::total_balance(CurrencyId::Erc20(H160::default()), &bob()), 0);

			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				10000
			);
			assert_eq!(Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob()), 0);
		});
}

#[test]
fn erc20_ensure_withdraw_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_ok!(Currencies::ensure_can_withdraw(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				100
			));
			assert_eq!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob(), 100),
				Err(Error::<Runtime>::BalanceTooLow.into()),
			);
			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				bob(),
				CurrencyId::Erc20(erc20_address()),
				100
			));
			assert_ok!(Currencies::ensure_can_withdraw(
				CurrencyId::Erc20(erc20_address()),
				&bob(),
				100
			));
			assert_eq!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob(), 101),
				Err(Error::<Runtime>::BalanceTooLow.into()),
			);
		});
}

#[test]
fn erc20_transfer_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(bob(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			<EVM as EVMTrait<AccountId>>::set_origin(bob());
			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				bob(),
				CurrencyId::Erc20(erc20_address()),
				100
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				100
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				100
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 100
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 100
			);

			assert_ok!(Currencies::transfer(
				Origin::signed(bob()),
				alice(),
				CurrencyId::Erc20(erc20_address()),
				10
			));

			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 90);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				90
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 90
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 90
			);
		});
}

#[test]
fn erc20_transfer_should_fail() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(bob(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			<EVM as EVMTrait<AccountId>>::set_origin(bob());
			// empty address
			assert!(
				Currencies::transfer(Origin::signed(alice()), bob(), CurrencyId::Erc20(H160::default()), 100).is_err()
			);

			// bob can't transfer. bob balance 0
			assert!(
				Currencies::transfer(Origin::signed(bob()), alice(), CurrencyId::Erc20(erc20_address()), 1).is_err()
			);
		});
}

#[test]
fn erc20_can_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::can_reserve(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				true
			);
		});
}

#[test]
fn erc20_slash_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::slash_reserved(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				1
			);
			assert_ok!(Currencies::reserve(CurrencyId::Erc20(erc20_address()), &alice(), 100));
			assert_eq!(
				Currencies::slash_reserved(CurrencyId::Erc20(erc20_address()), &alice(), 10),
				10
			);
		});
}

#[test]
fn erc20_reserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);

			assert_ok!(Currencies::reserve(CurrencyId::Erc20(erc20_address()), &alice(), 100));

			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				100
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 100
			);
		});
}

#[test]
fn erc20_unreserve_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let alice_balance = 10000;
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice(), 0),
				0
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice(), 50),
				50
			);
			assert_ok!(Currencies::reserve(CurrencyId::Erc20(erc20_address()), &alice(), 30));
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 30
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				30
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice(), 15),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance - 15
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				15
			);
			assert_eq!(
				Currencies::unreserve(CurrencyId::Erc20(erc20_address()), &alice(), 30),
				15
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);
		});
}

#[test]
fn erc20_should_not_slash() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::can_slash(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				false
			);
			// calling slash will return 0
			assert_eq!(Currencies::slash(CurrencyId::Erc20(erc20_address()), &alice(), 1), 0);
		});
}

#[test]
fn erc20_should_not_be_lockable() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_noop!(
				Currencies::set_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::extend_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::remove_lock(ID_1, CurrencyId::Erc20(erc20_address()), &alice()),
				Error::<Runtime>::Erc20InvalidOperation
			);
		});
}

#[test]
fn erc20_repatriate_reserved_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(bob(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let bob_balance = 100;
			let alice_balance = 10000 - bob_balance;
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				bob(),
				CurrencyId::Erc20(erc20_address()),
				bob_balance
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					&alice(),
					0,
					BalanceStatus::Free
				),
				Ok(0)
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					&alice(),
					50,
					BalanceStatus::Free
				),
				Ok(50)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				bob_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
			assert_ok!(Currencies::reserve(CurrencyId::Erc20(erc20_address()), &bob(), 50));
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 50);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				50
			);
			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob(),
					&bob(),
					60,
					BalanceStatus::Reserved
				),
				Ok(10)
			);
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 50);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				50
			);

			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob(),
					&alice(),
					30,
					BalanceStatus::Reserved
				),
				Ok(0)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				30
			);
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 50);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				20
			);

			assert_eq!(
				Currencies::repatriate_reserved(
					CurrencyId::Erc20(erc20_address()),
					&bob(),
					&alice(),
					30,
					BalanceStatus::Free
				),
				Ok(10)
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				alice_balance + 20
			);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				30
			);
			assert_eq!(Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob()), 50);
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
		});
}

#[test]
fn erc20_invalid_operation() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_noop!(
				Currencies::deposit(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::withdraw(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);
			assert_noop!(
				Currencies::update_balance(Origin::root(), alice(), CurrencyId::Erc20(erc20_address()), 1),
				Error::<Runtime>::Erc20InvalidOperation,
			);
		});
}
