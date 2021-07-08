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
	AccountId, Currencies, Event, ExtBuilder, HomaLite, Origin, Runtime, System, ACALA, ALICE, BOB, INITIAL_BALANCE,
	KSM, LKSM, RELAYCHAIN_STASH, ROOT,
};
use primitives::Balance;
use sp_runtime::traits::BadOrigin;

#[test]
fn mock_initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(KSM, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(LKSM, &BOB), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), INITIAL_BALANCE);
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
			HomaLite::request_mint(Origin::signed(BOB), 1000),
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

// can_request_mint
// * insufficient balance
// * stash account not set
//
// issue
// can_issue
// * 0 total
// * can_issue multiple eras
// * wrong caller
// * current era incremented
//
// claim
// * can claim
// * amount is correct
// * if total staked is 0
// * if era total is not set
// * repeated claims
//
