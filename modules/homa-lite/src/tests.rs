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

//! Unit tests for the Homa-Lite Module

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	Currencies, Event, ExtBuilder, HomaLite, Origin, Runtime, System, ACALA, ALICE, BOB, INITIAL_BALANCE, KSM, LKSM,
	RELAYCHAIN_STASH, ROOT,
};
use sp_runtime::traits::BadOrigin;

#[test]
fn mock_initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(KSM, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(KSM, &BOB), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &BOB), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &ROOT), INITIAL_BALANCE);
	});
}

#[test]
fn set_relaychain_stash_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(HomaLite::relaychain_stash_account(), None);

		// Only root/governance can set Stash account.
		assert_noop!(HomaLite::set_stash_account_id(Origin::signed(ALICE), BOB), BadOrigin);

		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));
		assert_eq!(HomaLite::relaychain_stash_account(), Some(RELAYCHAIN_STASH));

		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::RelaychainStashAccountUpdated(RELAYCHAIN_STASH))
		);
	});
}

#[test]
fn request_mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));
		let current_era = HomaLite::current_era();

		assert_noop!(
			HomaLite::request_mint(Origin::signed(ROOT), 1000),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));
		assert_eq!(PendingAmount::<Runtime>::get(&current_era, &ALICE), 1000);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_era, ALICE, 1000))
		);
	});
}

#[test]
fn request_mint_fails_without_relaychain_stash_set() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaLite::request_mint(Origin::signed(ALICE), 1000),
			Error::<Runtime>::RelaychainStashAccountNotSet
		);
	});
}

#[test]
fn can_request_mint_more_than_once_in_an_era() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));
		let current_era = HomaLite::current_era();

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_era, ALICE, 1000))
		);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 500));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_era, ALICE, 500))
		);

		assert_eq!(PendingAmount::<Runtime>::get(&current_era, &ALICE), 1500);
	});
}

#[test]
fn issue_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));
		let current_era = HomaLite::current_era();
		assert_eq!(current_era, 0);

		let lksm_issuance = Currencies::total_issuance(LKSM);
		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));
		assert_ok!(HomaLite::request_mint(Origin::signed(BOB), 500));

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), 3000));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::EraTotalRecorded(0, 3000, lksm_issuance))
		);

		assert_eq!(
			EraTotalIssuanceInfo::<Runtime>::get(0),
			Some(TotalIssuanceInfo {
				staking_total: 3000,
				liquid_total: lksm_issuance,
			})
		);
		assert_eq!(EraTotalIssuanceInfo::<Runtime>::get(1), None);
		assert_eq!(HomaLite::current_era(), 1);

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), 1));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::EraTotalRecorded(1, 1, lksm_issuance))
		);
		assert_eq!(HomaLite::current_era(), 2);
	});
}

#[test]
fn issue_can_handle_failed_cases() {
	ExtBuilder::default().build().execute_with(|| {
		// Total issuance cannot be set to zero
		assert_noop!(
			HomaLite::issue(Origin::signed(ROOT), 0),
			Error::<Runtime>::InvalidStakedCurrencyTotalIssuance
		);

		// Only Issuer Origin is allowed to make issue call.
		assert_noop!(HomaLite::issue(Origin::signed(ALICE), 0), BadOrigin);

		assert_eq!(HomaLite::current_era(), 0);
	});
}

#[test]
fn claim_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));

		let lksm_issuance = Currencies::total_issuance(LKSM);
		let ksm_issuance = lksm_issuance * 5;
		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));
		assert_ok!(HomaLite::request_mint(Origin::signed(BOB), 5000));

		let alice_yield = 1000 * lksm_issuance / ksm_issuance;
		let bob_yield = 5000 * lksm_issuance / ksm_issuance;

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), ksm_issuance));

		// Now that the liquid currency for Era 0 is issued, users can claim them.
		assert_ok!(HomaLite::claim(Origin::signed(ALICE), ALICE, 0));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::LiquidCurrencyClaimed(0, ALICE, alice_yield))
		);
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), alice_yield);

		assert_ok!(HomaLite::claim(Origin::signed(ALICE), BOB, 0));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::LiquidCurrencyClaimed(0, BOB, bob_yield))
		);
		assert_eq!(Currencies::free_balance(LKSM, &BOB), bob_yield);
	});
}

#[test]
fn claim_can_handle_math_errors() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));

		// Creates zero total issuance to trigger divide by zero error
		let zero_issuance = TotalIssuanceInfo {
			staking_total: 0,
			liquid_total: 0,
		};
		EraTotalIssuanceInfo::<Runtime>::insert(0, zero_issuance);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));

		// Now that the liquid currency for Era 0 is issued, users can claim them.
		assert_noop!(
			HomaLite::claim(Origin::signed(ALICE), ALICE, 0),
			Error::<Runtime>::ArithmeticError
		);
	});
}

#[test]
fn repeated_claims_has_no_effect() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup the Relaychain's stash account.
		assert_ok!(HomaLite::set_stash_account_id(Origin::signed(ROOT), RELAYCHAIN_STASH));

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), 1000));
		assert_ok!(HomaLite::issue(Origin::signed(ROOT), 10000));
		assert_ok!(HomaLite::claim(Origin::signed(ALICE), ALICE, 0));

		let alice_balance = Currencies::free_balance(LKSM, &ALICE);

		// The mint has already been claimed. claiming again does nothing.
		assert_ok!(HomaLite::claim(Origin::signed(ALICE), ALICE, 0));

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), alice_balance);
	});
}
