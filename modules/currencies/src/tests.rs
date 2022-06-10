// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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
use crate::mock::Erc20HoldingAccount;
use frame_support::{assert_noop, assert_ok, weights::GetDispatchInfo};
use mock::{
	alice, bob, deploy_contracts, erc20_address, eva, AccountId, AdaptedBasicCurrency, CouncilAccount, Currencies,
	DustAccount, Event, ExtBuilder, NativeCurrency, Origin, PalletBalances, Runtime, System, Tokens, ALICE_BALANCE,
	DOT, EVM, ID_1, NATIVE_CURRENCY_ID, X_TOKEN_ID,
};
use sp_core::H160;
use sp_runtime::{
	traits::{BadOrigin, Bounded},
	ModuleError,
};
use support::mocks::MockAddressMapping;
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
fn basic_currency_adapting_pallet_balances_deposit_throw_error_when_actual_deposit_is_not_expected() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(PalletBalances::total_balance(&eva()), 0);
			assert_eq!(PalletBalances::total_issuance(), 200);
			assert_noop!(
				AdaptedBasicCurrency::deposit(&eva(), 1),
				Error::<Runtime>::DepositFailed
			);
			assert_eq!(PalletBalances::total_balance(&eva()), 0);
			assert_eq!(PalletBalances::total_issuance(), 200);
			assert_ok!(AdaptedBasicCurrency::deposit(&eva(), 2));
			assert_eq!(PalletBalances::total_balance(&eva()), 2);
			assert_eq!(PalletBalances::total_issuance(), 202);
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
			System::assert_has_event(Event::Tokens(tokens::Event::Transfer {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 50,
			}));
			System::assert_has_event(Event::Currencies(crate::Event::Transferred {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 50,
			}));

			System::reset_events();
			assert_ok!(<Currencies as MultiCurrency<AccountId>>::transfer(
				X_TOKEN_ID,
				&alice(),
				&bob(),
				10
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 40);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &bob()), 160);
			System::assert_has_event(Event::Tokens(tokens::Event::Transfer {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 10,
			}));
			System::assert_has_event(Event::Currencies(crate::Event::Transferred {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 10,
			}));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::deposit(
				X_TOKEN_ID,
				&alice(),
				100
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 140);
			System::assert_last_event(Event::Tokens(tokens::Event::Deposited {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 100,
			}));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::withdraw(
				X_TOKEN_ID,
				&alice(),
				20
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &alice()), 120);
			System::assert_last_event(Event::Tokens(tokens::Event::Withdrawn {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 20,
			}));
		});
}

#[test]
fn erc20_total_issuance_should_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY_ID, 100000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				Currencies::total_issuance(CurrencyId::Erc20(erc20_address())),
				ALICE_BALANCE
			);
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
				ALICE_BALANCE
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
				ALICE_BALANCE
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
			assert_noop!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob(), 100),
				Error::<Runtime>::BalanceTooLow,
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
			assert_noop!(
				Currencies::ensure_can_withdraw(CurrencyId::Erc20(erc20_address()), &bob(), 101),
				Error::<Runtime>::BalanceTooLow,
			);
		});
}

#[test]
fn erc20_transfer_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(bob(), NATIVE_CURRENCY_ID, 100000),
			(eva(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(eva());

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
				ALICE_BALANCE - 100
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 100
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
				ALICE_BALANCE - 90
			);
			assert_eq!(
				Currencies::total_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 90
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

			// Real origin not found
			assert_noop!(
				Currencies::transfer(Origin::signed(alice()), bob(), CurrencyId::Erc20(erc20_address()), 100),
				Error::<Runtime>::RealOriginNotFound
			);

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
			assert!(Currencies::can_reserve(CurrencyId::Erc20(erc20_address()), &alice(), 1));
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
			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				0
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
			);

			assert_ok!(Currencies::reserve(CurrencyId::Erc20(erc20_address()), &alice(), 100));

			assert_eq!(
				Currencies::reserved_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				100
			);
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 100
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
			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
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
				ALICE_BALANCE - 30
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
				ALICE_BALANCE - 15
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
				ALICE_BALANCE
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
			assert!(!Currencies::can_slash(CurrencyId::Erc20(erc20_address()), &alice(), 1));
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
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				bob(),
				CurrencyId::Erc20(erc20_address()),
				bob_balance
			));

			assert_eq!(
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - bob_balance
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
				ALICE_BALANCE - bob_balance
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
				ALICE_BALANCE - bob_balance
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
				ALICE_BALANCE - bob_balance + 20
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
			<EVM as EVMTrait<AccountId>>::set_origin(alice());

			assert_noop!(
				Currencies::update_balance(Origin::root(), alice(), CurrencyId::Erc20(erc20_address()), 1),
				Error::<Runtime>::Erc20InvalidOperation,
			);
		});
}

#[test]
fn erc20_withdraw_deposit_works() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(bob(), NATIVE_CURRENCY_ID, 100000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice());

			let erc20_holding_account = MockAddressMapping::get_account_id(&Erc20HoldingAccount::get());

			// transfer to all-zero account failed.
			assert_noop!(
				Currencies::transfer(
					Origin::signed(alice()),
					MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
					CurrencyId::Erc20(erc20_address()),
					100
				),
				module_evm_bridge::Error::<Runtime>::ExecutionRevert
			);
			// transfer to non-all-zero account ok.
			assert_ok!(Currencies::transfer(
				Origin::signed(alice()),
				erc20_holding_account.clone(),
				CurrencyId::Erc20(erc20_address()),
				100
			));
			assert_eq!(
				100,
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &erc20_holding_account)
			);

			// withdraw: sender to erc20 holding account
			assert_ok!(Currencies::withdraw(CurrencyId::Erc20(erc20_address()), &alice(), 100));
			assert_eq!(
				200,
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &erc20_holding_account)
			);

			// deposit: erc20 holding account to receiver
			assert_ok!(Currencies::deposit(CurrencyId::Erc20(erc20_address()), &bob(), 100));
			assert_eq!(
				100,
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &erc20_holding_account)
			);
			assert_eq!(
				100,
				Currencies::free_balance(CurrencyId::Erc20(erc20_address()), &bob())
			);

			// deposit failed, because erc20 holding account balance not enough
			assert_noop!(
				Currencies::deposit(CurrencyId::Erc20(erc20_address()), &bob(), 101),
				module_evm_bridge::Error::<Runtime>::ExecutionRevert
			);
		});
}

#[test]
fn fungible_inspect_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(alice(), X_TOKEN_ID, 200000),
		])
		.build()
		.execute_with(|| {
			deploy_contracts();

			// Test for Inspect::total_issuance
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				100000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(CurrencyId::Erc20(erc20_address())),
				ALICE_BALANCE
			);
			assert_eq!(<NativeCurrency as fungible::Inspect<_>>::total_issuance(), 100000);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 100000);

			// Test for Inspect::minimum_balance
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::minimum_balance(NATIVE_CURRENCY_ID),
				2
			);
			assert_eq!(<Currencies as fungibles::Inspect<_>>::minimum_balance(X_TOKEN_ID), 0);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::minimum_balance(CurrencyId::Erc20(erc20_address())),
				0
			);
			assert_eq!(<NativeCurrency as fungible::Inspect<_>>::minimum_balance(), 2);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::minimum_balance(), 2);

			// Test for Inspect::balance
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				45380
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
			);
			assert_eq!(<NativeCurrency as fungible::Inspect<_>>::balance(&alice()), 45380);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()), 45380);

			// Test for Inspect::reducible_balance. No locks or reserves
			// With Keep alive
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(NATIVE_CURRENCY_ID, &alice(), true),
				45378
			);
			assert_eq!(
				<NativeCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), true),
				45378
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), true),
				45378
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(X_TOKEN_ID, &alice(), true),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					true
				),
				ALICE_BALANCE
			);

			// Test for Inspect::reducible_balance. No locks or reserves
			// without Keep alive.
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(NATIVE_CURRENCY_ID, &alice(), false),
				45380
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(X_TOKEN_ID, &alice(), false),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					false
				),
				ALICE_BALANCE
			);
			assert_eq!(
				<NativeCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), false),
				45380
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), false),
				45380
			);

			// Set some locks
			assert_ok!(Currencies::set_lock(ID_1, NATIVE_CURRENCY_ID, &alice(), 1000));
			assert_ok!(Currencies::set_lock(ID_1, X_TOKEN_ID, &alice(), 1000));

			// Test Inspect::reducible_balance with locks
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(NATIVE_CURRENCY_ID, &alice(), true),
				44380
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(X_TOKEN_ID, &alice(), true),
				199000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					true
				),
				ALICE_BALANCE
			);
			assert_eq!(
				<NativeCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), true),
				44380
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), true),
				44380
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(NATIVE_CURRENCY_ID, &alice(), false),
				44380
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(X_TOKEN_ID, &alice(), false),
				199000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::reducible_balance(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					false
				),
				ALICE_BALANCE
			);
			assert_eq!(
				<NativeCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), false),
				44380
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::reducible_balance(&alice(), false),
				44380
			);

			// Test for Inspect::can_deposit
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(
					NATIVE_CURRENCY_ID,
					&alice(),
					Bounded::max_value(),
					false
				),
				DepositConsequence::Overflow
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_deposit(&alice(), Bounded::max_value(), false),
				DepositConsequence::Overflow
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(NATIVE_CURRENCY_ID, &bob(), 1, false),
				DepositConsequence::BelowMinimum
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_deposit(&bob(), 1, false),
				DepositConsequence::BelowMinimum
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(NATIVE_CURRENCY_ID, &alice(), 100, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_deposit(&alice(), 100, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(NATIVE_CURRENCY_ID, &alice(), 0, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_deposit(&alice(), 0, false),
				DepositConsequence::Success
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), Bounded::max_value(), false),
				DepositConsequence::Overflow
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), Bounded::max_value(), false),
				DepositConsequence::Overflow
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), 100, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), 100, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), 0, false),
				DepositConsequence::Success
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_deposit(X_TOKEN_ID, &alice(), 0, false),
				DepositConsequence::Success
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					Bounded::max_value(),
					false
				),
				DepositConsequence::Overflow
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					100,
					false
				),
				DepositConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_deposit(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					0,
					false
				),
				DepositConsequence::Success
			);

			// TODO: update test code to assert_eq! for this: https://github.com/AcalaNetwork/Acala/issues/2038
			// Test Inspect::can_withdraw
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(NATIVE_CURRENCY_ID, &alice(), Bounded::max_value()),
				WithdrawConsequence::Underflow
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_withdraw(&alice(), Bounded::max_value()),
				WithdrawConsequence::Underflow
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(NATIVE_CURRENCY_ID, &alice(), 44381),
				WithdrawConsequence::Frozen
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_withdraw(&alice(), 44381),
				WithdrawConsequence::Frozen
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(NATIVE_CURRENCY_ID, &alice(), 100),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_withdraw(&alice(), 100),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(NATIVE_CURRENCY_ID, &alice(), 0),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::can_withdraw(&alice(), 0),
				WithdrawConsequence::Success
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), Bounded::max_value()),
				WithdrawConsequence::Underflow
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), Bounded::max_value()),
				WithdrawConsequence::Underflow
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 200001),
				WithdrawConsequence::Underflow
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 200001),
				WithdrawConsequence::Underflow
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 100),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 100),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 0),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Tokens as fungibles::Inspect<_>>::can_withdraw(X_TOKEN_ID, &alice(), 0),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					Bounded::max_value()
				),
				WithdrawConsequence::NoFunds
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(CurrencyId::Erc20(erc20_address()), &alice(), 100),
				WithdrawConsequence::Success
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::can_withdraw(CurrencyId::Erc20(erc20_address()), &alice(), 0),
				WithdrawConsequence::Success
			);
		});
}

#[test]
fn fungible_mutate_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(alice(), X_TOKEN_ID, 200000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				100000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				100000
			);
			assert_ok!(<Currencies as fungibles::Mutate<_>>::mint_into(
				NATIVE_CURRENCY_ID,
				&alice(),
				1000
			));
			System::assert_last_event(Event::Balances(pallet_balances::Event::Deposit {
				who: alice(),
				amount: 1000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				101000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				101000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_ok!(<Currencies as fungibles::Mutate<_>>::mint_into(
				X_TOKEN_ID,
				&alice(),
				1000
			));
			System::assert_last_event(Event::Tokens(tokens::Event::Deposited {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 1000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				201000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				201000
			);

			assert_ok!(<Currencies as fungibles::Mutate<_>>::mint_into(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				0
			));
			// mint_into will deposit erc20 holding account to recipient.
			// but here erc20 holding account don't have enough balance.
			assert_noop!(
				<Currencies as fungibles::Mutate<_>>::mint_into(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::DepositFailed
			);

			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 101000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				101000
			);
			assert_ok!(<AdaptedBasicCurrency as fungible::Mutate<_>>::mint_into(&alice(), 1000));
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 102000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				102000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				102000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				102000
			);
			assert_ok!(<Currencies as fungibles::Mutate<_>>::burn_from(
				NATIVE_CURRENCY_ID,
				&alice(),
				1000
			));
			System::assert_last_event(Event::Balances(pallet_balances::Event::Withdraw {
				who: alice(),
				amount: 1000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				101000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				101000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				201000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				201000
			);
			assert_ok!(<Currencies as fungibles::Mutate<_>>::burn_from(
				X_TOKEN_ID,
				&alice(),
				1000
			));
			System::assert_last_event(Event::Tokens(tokens::Event::Withdrawn {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 1000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);

			assert_ok!(<Currencies as fungibles::Mutate<_>>::burn_from(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				0
			));
			assert_noop!(
				<Currencies as fungibles::Mutate<_>>::burn_from(CurrencyId::Erc20(erc20_address()), &alice(), 1),
				Error::<Runtime>::Erc20InvalidOperation
			);

			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 101000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				101000
			);
			assert_ok!(<AdaptedBasicCurrency as fungible::Mutate<_>>::burn_from(&alice(), 1000));
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 100000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				100000
			);

			// Burn dust if remaining is less than ED.
			assert_eq!(
				<Currencies as fungibles::Mutate<_>>::burn_from(NATIVE_CURRENCY_ID, &alice(), 99_999),
				Ok(100_000)
			);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 0);
		});
}

#[test]
fn fungible_transfer_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 500000),
			(alice(), X_TOKEN_ID, 200000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &bob()),
				0
			);

			System::reset_events();
			assert_ok!(<Currencies as fungibles::Transfer<_>>::transfer(
				NATIVE_CURRENCY_ID,
				&alice(),
				&bob(),
				10000,
				true
			));
			System::assert_has_event(Event::Balances(pallet_balances::Event::Transfer {
				from: alice(),
				to: bob(),
				amount: 10000,
			}));
			System::assert_has_event(Event::Currencies(crate::Event::Transferred {
				currency_id: NATIVE_CURRENCY_ID,
				from: alice(),
				to: bob(),
				amount: 10000,
			}));

			assert_noop!(
				<Currencies as fungibles::Transfer<_>>::transfer(NATIVE_CURRENCY_ID, &alice(), &bob(), 489_999, true),
				DispatchError::Module(ModuleError {
					index: 1,
					error: [4, 0, 0, 0],
					message: Some("KeepAlive")
				})
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				490000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &bob()),
				10000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &bob()), 0);
			System::reset_events();
			assert_ok!(<Currencies as fungibles::Transfer<_>>::transfer(
				X_TOKEN_ID,
				&alice(),
				&bob(),
				10000,
				true
			));
			System::assert_has_event(Event::Tokens(tokens::Event::Transfer {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 10000,
			}));
			System::assert_has_event(Event::Currencies(crate::Event::Transferred {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 10000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				190000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &bob()),
				10000
			);

			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				490000
			);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&bob()), 10000);
			assert_ok!(<AdaptedBasicCurrency as fungible::Transfer<_>>::transfer(
				&alice(),
				&bob(),
				10000,
				true
			));
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				480000
			);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&bob()), 20000);

			deploy_contracts();
			<EVM as EVMTrait<AccountId>>::set_origin(alice());
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
			assert_ok!(<Currencies as fungibles::Transfer<_>>::transfer(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				&bob(),
				2000,
				true
			));
			System::assert_last_event(Event::Currencies(crate::Event::Transferred {
				currency_id: CurrencyId::Erc20(erc20_address()),
				from: alice(),
				to: bob(),
				amount: 2000,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 2000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &bob()),
				2000
			);
		});
}

#[test]
fn fungible_unbalanced_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 100000),
			(alice(), X_TOKEN_ID, 200000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				100000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				100000
			);
			assert_ok!(<Currencies as fungibles::Unbalanced<_>>::set_balance(
				NATIVE_CURRENCY_ID,
				&alice(),
				80000
			));
			System::assert_last_event(Event::Balances(pallet_balances::Event::BalanceSet {
				who: alice(),
				free: 80000,
				reserved: 0,
			}));
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				100000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				80000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_ok!(<Currencies as fungibles::Unbalanced<_>>::set_balance(
				X_TOKEN_ID,
				&alice(),
				80000
			));
			System::assert_last_event(Event::Tokens(tokens::Event::BalanceSet {
				currency_id: X_TOKEN_ID,
				who: alice(),
				free: 80000,
				reserved: 0,
			}));

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				80000
			);

			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 100000);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()), 80000);
			assert_ok!(<AdaptedBasicCurrency as fungible::Unbalanced<_>>::set_balance(
				&alice(),
				60000
			));
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 100000);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()), 60000);

			assert_noop!(
				<Currencies as fungibles::Unbalanced<_>>::set_balance(CurrencyId::Erc20(erc20_address()), &alice(), 0),
				Error::<Runtime>::Erc20InvalidOperation
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				100000
			);
			<Currencies as fungibles::Unbalanced<_>>::set_total_issuance(NATIVE_CURRENCY_ID, 60000);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(NATIVE_CURRENCY_ID),
				60000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID),
				200000
			);
			<Currencies as fungibles::Unbalanced<_>>::set_total_issuance(X_TOKEN_ID, 80000);
			assert_eq!(<Currencies as fungibles::Inspect<_>>::total_issuance(X_TOKEN_ID), 80000);
			System::assert_last_event(Event::Tokens(tokens::Event::TotalIssuanceSet {
				currency_id: X_TOKEN_ID,
				amount: 80000,
			}));

			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 60000);
			<AdaptedBasicCurrency as fungible::Unbalanced<_>>::set_total_issuance(0);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::total_issuance(), 0);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(CurrencyId::Erc20(erc20_address())),
				0
			);
			<Currencies as fungibles::Unbalanced<_>>::set_total_issuance(CurrencyId::Erc20(erc20_address()), 80000);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::total_issuance(CurrencyId::Erc20(erc20_address())),
				0
			);
		});
}

#[test]
fn fungible_inspect_hold_and_hold_trait_should_work() {
	ExtBuilder::default()
		.balances(vec![
			(alice(), NATIVE_CURRENCY_ID, 500000),
			(alice(), X_TOKEN_ID, 200000),
			(bob(), NATIVE_CURRENCY_ID, 10000),
			(bob(), X_TOKEN_ID, 10000),
		])
		.build()
		.execute_with(|| {
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(NATIVE_CURRENCY_ID, &alice(), 499998),
				true
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(NATIVE_CURRENCY_ID, &alice(), 500001),
				false
			);
			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				NATIVE_CURRENCY_ID,
				&alice(),
				20000
			));
			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::hold(NATIVE_CURRENCY_ID, &alice(), 500000),
				DispatchError::Module(ModuleError {
					index: 1,
					error: [2, 0, 0, 0],
					message: Some("InsufficientBalance",),
				})
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				20000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(X_TOKEN_ID, &alice(), 200000),
				true
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(X_TOKEN_ID, &alice(), 200001),
				false
			);
			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				X_TOKEN_ID,
				&alice(),
				20000
			));
			System::assert_last_event(Event::Tokens(tokens::Event::Reserved {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 20000,
			}));

			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::hold(X_TOKEN_ID, &alice(), 200000),
				DispatchError::Module(ModuleError {
					index: 2,
					error: [0, 0, 0, 0],
					message: Some("BalanceTooLow",),
				},)
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				20000
			);

			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				500000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				20000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::can_hold(&alice(), 20000),
				true
			);
			assert_ok!(<AdaptedBasicCurrency as fungible::MutateHold<_>>::hold(&alice(), 20000));
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				500000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				40000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				40000
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(NATIVE_CURRENCY_ID, &alice(), 10000, true),
				Ok(10000)
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				30000
			);
			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::release(NATIVE_CURRENCY_ID, &alice(), 50000, false),
				DispatchError::Module(ModuleError {
					index: 1,
					error: [2, 0, 0, 0],
					message: Some("InsufficientBalance")
				})
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(NATIVE_CURRENCY_ID, &alice(), 50000, true),
				Ok(30000)
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				0
			);
			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				NATIVE_CURRENCY_ID,
				&alice(),
				30000
			));

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				20000
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(X_TOKEN_ID, &alice(), 10000, true),
				Ok(10000)
			);
			System::assert_last_event(Event::Tokens(tokens::Event::Unreserved {
				currency_id: X_TOKEN_ID,
				who: alice(),
				amount: 10000,
			}));

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				10000
			);
			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::release(X_TOKEN_ID, &alice(), 100000, false),
				DispatchError::Module(ModuleError {
					index: 2,
					error: [0, 0, 0, 0],
					message: Some("BalanceTooLow")
				})
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(X_TOKEN_ID, &alice(), 100000, true),
				Ok(10000)
			);
			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				X_TOKEN_ID,
				&alice(),
				10000
			));

			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				500000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				30000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::MutateHold<_>>::release(&alice(), 10000, true),
				Ok(10000)
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				500000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				20000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				500000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				20000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &bob()),
				10000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &bob()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					NATIVE_CURRENCY_ID,
					&alice(),
					&bob(),
					2000,
					false,
					true
				),
				Ok(2000)
			);
			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					NATIVE_CURRENCY_ID,
					&alice(),
					&bob(),
					200000,
					false,
					true
				),
				DispatchError::Module(ModuleError {
					index: 1,
					error: [2, 0, 0, 0],
					message: Some("InsufficientBalance")
				})
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &alice()),
				498000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &alice()),
				18000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(NATIVE_CURRENCY_ID, &bob()),
				12000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(NATIVE_CURRENCY_ID, &bob()),
				2000
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				200000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				10000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &bob()),
				10000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &bob()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					X_TOKEN_ID,
					&alice(),
					&bob(),
					2000,
					false,
					true
				),
				Ok(2000)
			);
			System::assert_last_event(Event::Tokens(tokens::Event::ReserveRepatriated {
				currency_id: X_TOKEN_ID,
				from: alice(),
				to: bob(),
				amount: 2000,
				status: BalanceStatus::Reserved,
			}));

			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					X_TOKEN_ID,
					&alice(),
					&bob(),
					200000,
					false,
					true
				),
				DispatchError::Module(ModuleError {
					index: 2,
					error: [0, 0, 0, 0],
					message: Some("BalanceTooLow")
				})
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &alice()),
				198000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &alice()),
				8000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(X_TOKEN_ID, &bob()),
				12000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(X_TOKEN_ID, &bob()),
				2000
			);

			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				498000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				18000
			);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&bob()), 12000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&bob()),
				2000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::MutateHold<_>>::transfer_held(&alice(), &bob(), 2000, false, true),
				Ok(2000)
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&alice()),
				496000
			);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&alice()),
				16000
			);
			assert_eq!(<AdaptedBasicCurrency as fungible::Inspect<_>>::balance(&bob()), 14000);
			assert_eq!(
				<AdaptedBasicCurrency as fungible::InspectHold<_>>::balance_on_hold(&bob()),
				4000
			);

			deploy_contracts();
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(CurrencyId::Erc20(erc20_address()), &alice(), 8000),
				true
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::can_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					ALICE_BALANCE + 1
				),
				false
			);
			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				8000
			));

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 8000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				8000
			);

			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					0,
					true
				),
				Ok(0)
			);

			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::release(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					8001,
					false
				),
				Error::<Runtime>::BalanceTooLow
			);
			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::release(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					8001,
					true
				),
				Ok(8000)
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				0
			);

			assert_ok!(<Currencies as fungibles::MutateHold<_>>::hold(
				CurrencyId::Erc20(erc20_address()),
				&alice(),
				8000
			));

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 8000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				8000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);

			assert_noop!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					&bob(),
					8001,
					false,
					true
				),
				Error::<Runtime>::BalanceTooLow
			);

			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					&bob(),
					2000,
					false,
					true
				),
				Ok(2000)
			);

			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 8000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				6000
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(CurrencyId::Erc20(erc20_address()), &bob()),
				2000
			);

			assert_eq!(
				<Currencies as fungibles::MutateHold<_>>::transfer_held(
					CurrencyId::Erc20(erc20_address()),
					&alice(),
					&bob(),
					6001,
					true,
					true
				),
				Ok(6000)
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &alice()),
				ALICE_BALANCE - 8000
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(
					CurrencyId::Erc20(erc20_address()),
					&alice()
				),
				0
			);
			assert_eq!(
				<Currencies as fungibles::Inspect<_>>::balance(CurrencyId::Erc20(erc20_address()), &bob()),
				0
			);
			assert_eq!(
				<Currencies as fungibles::InspectHold<_>>::balance_on_hold(CurrencyId::Erc20(erc20_address()), &bob()),
				8000
			);
		});
}

#[test]
fn sweep_dust_tokens_works() {
	ExtBuilder::default().build().execute_with(|| {
		tokens::Accounts::<Runtime>::insert(
			bob(),
			DOT,
			tokens::AccountData {
				free: 1,
				frozen: 0,
				reserved: 0,
			},
		);
		tokens::Accounts::<Runtime>::insert(
			eva(),
			DOT,
			tokens::AccountData {
				free: 2,
				frozen: 0,
				reserved: 0,
			},
		);
		tokens::Accounts::<Runtime>::insert(
			alice(),
			DOT,
			tokens::AccountData {
				free: 0,
				frozen: 1,
				reserved: 0,
			},
		);
		tokens::Accounts::<Runtime>::insert(
			DustAccount::get(),
			DOT,
			tokens::AccountData {
				free: 100,
				frozen: 0,
				reserved: 0,
			},
		);
		tokens::TotalIssuance::<Runtime>::insert(DOT, 104);

		let accounts = vec![bob(), eva(), alice()];

		assert_noop!(
			Currencies::sweep_dust(Origin::signed(bob()), DOT, accounts.clone()),
			DispatchError::BadOrigin
		);

		assert_ok!(Currencies::sweep_dust(
			Origin::signed(CouncilAccount::get()),
			DOT,
			accounts
		));
		System::assert_last_event(Event::Currencies(crate::Event::DustSwept {
			currency_id: DOT,
			who: bob(),
			amount: 1,
		}));

		// bob's account is gone
		assert_eq!(tokens::Accounts::<Runtime>::contains_key(bob(), DOT), false);
		assert_eq!(Currencies::free_balance(DOT, &bob()), 0);

		// eva's account remains, not below ED
		assert_eq!(Currencies::free_balance(DOT, &eva()), 2);

		// Dust transferred to dust receiver
		assert_eq!(Currencies::free_balance(DOT, &DustAccount::get()), 101);
		// Total issuance remains the same
		assert_eq!(Currencies::total_issuance(DOT), 104);
	});
}

#[test]
fn sweep_dust_native_currency_works() {
	use frame_support::traits::StoredMap;
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(<Runtime as pallet_balances::Config>::AccountStore::insert(
			&bob(),
			pallet_balances::AccountData {
				free: 1,
				reserved: 0,
				misc_frozen: 0,
				fee_frozen: 0,
			},
		));
		assert_ok!(<Runtime as pallet_balances::Config>::AccountStore::insert(
			&eva(),
			pallet_balances::AccountData {
				free: 2,
				reserved: 0,
				misc_frozen: 0,
				fee_frozen: 0,
			},
		));
		assert_ok!(<Runtime as pallet_balances::Config>::AccountStore::insert(
			&alice(),
			pallet_balances::AccountData {
				free: 0,
				reserved: 0,
				misc_frozen: 2,
				fee_frozen: 2,
			},
		));
		assert_ok!(<Runtime as pallet_balances::Config>::AccountStore::insert(
			&DustAccount::get(),
			pallet_balances::AccountData {
				free: 100,
				reserved: 0,
				misc_frozen: 0,
				fee_frozen: 0,
			},
		));
		pallet_balances::TotalIssuance::<Runtime>::put(104);

		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &bob()), 1);
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &eva()), 2);
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &alice()), 0);
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &DustAccount::get()), 100);

		let accounts = vec![bob(), eva(), alice()];

		assert_noop!(
			Currencies::sweep_dust(Origin::signed(bob()), NATIVE_CURRENCY_ID, accounts.clone()),
			DispatchError::BadOrigin
		);

		assert_ok!(Currencies::sweep_dust(
			Origin::signed(CouncilAccount::get()),
			NATIVE_CURRENCY_ID,
			accounts
		));
		System::assert_last_event(Event::Currencies(crate::Event::DustSwept {
			currency_id: NATIVE_CURRENCY_ID,
			who: bob(),
			amount: 1,
		}));

		// bob's account is gone
		assert_eq!(System::account_exists(&bob()), false);
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &bob()), 0);

		// eva's account remains, not below ED
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &eva()), 2);

		// Dust transferred to dust receiver
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY_ID, &DustAccount::get()), 101);
		// Total issuance remains the same
		assert_eq!(Currencies::total_issuance(NATIVE_CURRENCY_ID), 104);
	});
}

#[test]
fn sweep_dust_erc20_not_allowed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::sweep_dust(
				Origin::signed(CouncilAccount::get()),
				CurrencyId::Erc20(erc20_address()),
				vec![]
			),
			Error::<Runtime>::Erc20InvalidOperation
		);
	});
}

#[test]
fn transfer_erc20_will_charge_gas() {
	ExtBuilder::default().build().execute_with(|| {
		let dispatch_info = module::Call::<Runtime>::transfer {
			dest: alice(),
			currency_id: CurrencyId::Erc20(erc20_address()),
			amount: 1,
		}
		.get_dispatch_info();
		assert_eq!(
			dispatch_info.weight,
			<Runtime as module::Config>::WeightInfo::transfer_non_native_currency()
				+ support::evm::limits::erc20::TRANSFER.gas // mock GasToWeight is 1:1
		);

		let dispatch_info = module::Call::<Runtime>::transfer {
			dest: alice(),
			currency_id: DOT,
			amount: 1,
		}
		.get_dispatch_info();
		assert_eq!(
			dispatch_info.weight,
			<Runtime as module::Config>::WeightInfo::transfer_non_native_currency()
		);
	});
}
