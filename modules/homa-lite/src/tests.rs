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
	dollar, millicent, Currencies, Event, ExtBuilder, HomaLite, MinimumMintThreshold, MintFee, Origin, Runtime, System,
	ACALA, ALICE, BOB, INITIAL_BALANCE, INVALID_CALLER, KSM, LKSM, ROOT,
};
use sp_runtime::traits::BadOrigin;

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
fn mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = dollar(1000);

		assert_ok!(HomaLite::set_minting_cap(
			Origin::signed(ROOT),
			5 * dollar(INITIAL_BALANCE)
		));

		assert_noop!(
			HomaLite::mint(Origin::signed(ROOT), amount, 0),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// Since the exchange rate is not set, use the default 1:10 ratio
		// liquid = (amount - MintFee) * 10 * (1 - MaxRewardPerEra)
		let mut liquid = Permill::from_percent(90).mul((amount - MintFee::get()) * 10);
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), amount, 0));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), liquid);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::Minted(ALICE, amount, liquid))
		);

		// Set the total staking amount
		let lksm_issuance = Currencies::total_issuance(LKSM);
		// Set the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::signed(ROOT),
			lksm_issuance / 5
		));

		// The exchange rate is now 1:5 ratio
		liquid = Permill::from_percent(90).mul((amount - MintFee::get()) * 5);
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount, 0));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), liquid);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::Minted(BOB, amount, liquid))
		);
	});
}

#[test]
fn mint_fails_when_below_minimum() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::signed(ROOT), dollar(1_000)));

		// The mint amount must be strictly larger than Mint fee + Minimum amount allowed.
		assert_noop!(
			HomaLite::mint(Origin::signed(ALICE), MintFee::get() + MinimumMintThreshold::get(), 0),
			Error::<Runtime>::MintAmountBelowMinimumThreshold
		);

		assert_ok!(HomaLite::mint(
			Origin::signed(ALICE),
			MintFee::get() + MinimumMintThreshold::get() + millicent(1),
			0
		));
	});
}

#[test]
fn mint_fails_when_cap_is_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::signed(ROOT), dollar(1_000)));

		assert_noop!(
			HomaLite::mint(Origin::signed(ALICE), dollar(1_001), 0),
			Error::<Runtime>::ExceededStakingCurrencyMintCap
		);

		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(1_000), 0));

		assert_noop!(
			HomaLite::mint(Origin::signed(ALICE), dollar(1), 0),
			Error::<Runtime>::ExceededStakingCurrencyMintCap
		);
	});
}

#[test]
fn failed_xcm_transfer_is_handled() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::signed(ROOT), dollar(1_000)));

		// XCM transfer fails if it is called by INVALID_CALLER.
		assert_noop!(
			HomaLite::mint(Origin::signed(INVALID_CALLER), dollar(1), 0),
			Error::<Runtime>::XcmTransferFailed
		);
	});
}

#[test]
fn cannot_set_total_staking_currency_to_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaLite::set_total_staking_currency(Origin::signed(ROOT), 0),
			Error::<Runtime>::InvalidTotalStakingCurrency
		);
		assert_ok!(HomaLite::set_total_staking_currency(Origin::signed(ROOT), 1));
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), 1);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::TotalStakingCurrencySet(1))
		);
	});
}

#[test]
fn requires_root_to_set_total_staking_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaLite::set_total_staking_currency(Origin::signed(ALICE), 0),
			BadOrigin
		);
	});
}

#[test]
fn can_set_mint_cap() {
	ExtBuilder::default().build().execute_with(|| {
		// Current cap is not set
		assert_eq!(StakingCurrencyMintCap::<Runtime>::get(), 0);

		// Requires Root previlege.
		assert_noop!(
			HomaLite::set_minting_cap(Origin::signed(ALICE), dollar(1_000)),
			BadOrigin
		);

		// Set the cap.
		assert_ok!(HomaLite::set_minting_cap(Origin::signed(ROOT), dollar(1_000)));

		// Cap should be set now.
		assert_eq!(StakingCurrencyMintCap::<Runtime>::get(), dollar(1_000));

		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::StakingCurrencyMintCapUpdated(dollar(1_000)))
		);
	});
}
