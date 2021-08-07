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
	dollar, Currencies, Event, ExtBuilder, HomaLite, Origin, Runtime, System, ACALA, ALICE, BOB, INITIAL_BALANCE,
	INVALID_CALLER, KSM, LKSM, ROOT,
};
use sp_runtime::{traits::BadOrigin, ArithmeticError};

#[test]
fn mock_initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		let initial_dollar = dollar(INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(KSM, &ALICE), initial_dollar);
		assert_eq!(Currencies::free_balance(KSM, &BOB), initial_dollar);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &BOB), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &ROOT), initial_dollar);
	});
}

#[test]
fn request_mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = dollar(1000);

		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));
		let current_batch = HomaLite::current_batch();

		assert_noop!(
			HomaLite::request_mint(Origin::signed(ROOT), amount, 0),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), amount, 0));
		assert_eq!(PendingAmount::<Runtime>::get(&current_batch, &ALICE), amount);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_batch, ALICE, amount))
		);
	});
}

#[test]
fn can_request_mint_more_than_once_in_a_batch() {
	ExtBuilder::default().build().execute_with(|| {
		let current_batch = HomaLite::current_batch();
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(10_000)));

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1000), 0));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_batch, ALICE, dollar(1000)))
		);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(500), 0));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::MintRequested(current_batch, ALICE, dollar(500)))
		);

		assert_eq!(PendingAmount::<Runtime>::get(&current_batch, &ALICE), dollar(1500));
	});
}

#[test]
fn request_mint_fails_when_below_minimum() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));
		assert_noop!(
			HomaLite::request_mint(Origin::signed(ALICE), 1_000, 0),
			Error::<Runtime>::MintAmountBelowMinimumThreshold
		);
	});
}

#[test]
fn request_mint_fails_when_cap_is_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));

		assert_noop!(
			HomaLite::request_mint(Origin::signed(ALICE), dollar(1_001), 0),
			Error::<Runtime>::ExceededStakingCurrencyMintCap
		);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1_000), 0));

		assert_noop!(
			HomaLite::request_mint(Origin::signed(ALICE), dollar(1), 0),
			Error::<Runtime>::ExceededStakingCurrencyMintCap
		);
	});
}

#[test]
fn failed_xcm_transfer_is_handled() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));

		// XCM transfer fails if it is called by INVALID_CALLER.
		assert_noop!(
			HomaLite::request_mint(Origin::signed(INVALID_CALLER), dollar(1), 0),
			Error::<Runtime>::XcmTransferFailed
		);
	});
}

#[test]
fn issue_works() {
	ExtBuilder::default().build().execute_with(|| {
		let current_batch = HomaLite::current_batch();
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(10_000)));
		assert_eq!(current_batch, 0);

		let lksm_issuance = Currencies::total_issuance(LKSM);
		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1000), 0));
		assert_ok!(HomaLite::request_mint(Origin::signed(BOB), dollar(500), 0));

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), dollar(3000)));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::BatchProcessed(0, dollar(3000), lksm_issuance))
		);

		assert_eq!(
			BatchTotalIssuanceInfo::<Runtime>::get(0),
			Some(TotalIssuanceInfo {
				staking_total: dollar(3000),
				liquid_total: lksm_issuance,
			})
		);
		assert_eq!(BatchTotalIssuanceInfo::<Runtime>::get(1), None);
		assert_eq!(HomaLite::current_batch(), 1);

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), dollar(1)));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::BatchProcessed(1, dollar(1), lksm_issuance))
		);
		assert_eq!(HomaLite::current_batch(), 2);
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

		assert_eq!(HomaLite::current_batch(), 0);
	});
}

#[test]
fn claim_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(10_000)));

		let lksm_issuance = Currencies::total_issuance(LKSM);
		let ksm_issuance = lksm_issuance * 5;
		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1000), 0));
		assert_ok!(HomaLite::request_mint(Origin::signed(BOB), dollar(5000), 0));

		let alice_yield = dollar(1000) * lksm_issuance / ksm_issuance;
		let bob_yield = dollar(5000) * lksm_issuance / ksm_issuance;

		// Trying to claim without "issue" call will fail
		assert_noop!(
			HomaLite::claim(Origin::signed(ALICE), ALICE, 0),
			Error::<Runtime>::LiquidCurrencyNotIssuedForThisBatch
		);

		assert_ok!(HomaLite::issue(Origin::signed(ROOT), ksm_issuance));

		// Now that the liquid currency for batch 0 is issued, users can claim them.
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
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));

		// Creates zero total issuance to trigger divide by zero error
		let zero_issuance = TotalIssuanceInfo {
			staking_total: 0,
			liquid_total: 0,
		};
		BatchTotalIssuanceInfo::<Runtime>::insert(0, zero_issuance);

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1000), 0));

		// Now that the liquid currency for Batch 0 is issued, users can claim them.
		assert_noop!(
			HomaLite::claim(Origin::signed(ALICE), ALICE, 0),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn repeated_claims_has_no_effect() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(20_000)));

		assert_ok!(HomaLite::request_mint(Origin::signed(ALICE), dollar(1000), 0));
		assert_ok!(HomaLite::issue(Origin::signed(ROOT), dollar(10000)));
		assert_ok!(HomaLite::claim(Origin::signed(ALICE), ALICE, 0));

		let alice_balance = Currencies::free_balance(LKSM, &ALICE);

		// The mint has already been claimed. claiming again does nothing.
		assert_ok!(HomaLite::claim(Origin::signed(ALICE), ALICE, 0));

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), alice_balance);
	});
}

#[test]
fn can_set_mint_cap() {
	ExtBuilder::default().build().execute_with(|| {
		// Current cap is not set
		assert_eq!(StakingCurrencyMintCap::<Runtime>::get(), 0);

		// Requires Root previlege.
		assert_noop!(
			HomaLite::set_staking_currency_cap(Origin::signed(ALICE), dollar(1_000)),
			BadOrigin
		);

		// Set the cap.
		assert_ok!(HomaLite::set_staking_currency_cap(Origin::signed(ROOT), dollar(1_000)));

		// Cap should be set now.
		assert_eq!(StakingCurrencyMintCap::<Runtime>::get(), dollar(1_000));

		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::StakingCurrencyMintCapUpdated(dollar(1_000)))
		);
	});
}
