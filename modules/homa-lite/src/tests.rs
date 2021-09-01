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
			HomaLite::mint(Origin::signed(ROOT), amount),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// Since the exchange rate is not set, use the default 1:10 ratio
		// liquid = (amount - MintFee) * 10 * (1 - MaxRewardPerEra)
		//        = 0.99 * (1000 - 0.01)  * 10 = 9899.901
		let mut liquid = 9_899_901_000_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), amount));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), liquid);
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::Minted(ALICE, amount, liquid))
		);
		// The total staking currency is now increased.
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1000));

		// Set the total staking amount
		let lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_009_899_901_000_000_000);

		// Set the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::signed(ROOT),
			lksm_issuance / 5
		));

		assert_eq!(
			HomaLite::get_staking_exchange_rate(),
			ExchangeRate::saturating_from_rational(lksm_issuance, lksm_issuance / 5)
		);
		assert_eq!(
			LiquidExchangeProvider::<Runtime>::get_exchange_rate(),
			ExchangeRate::saturating_from_rational(lksm_issuance / 5, lksm_issuance)
		);

		// The exchange rate is now 1:5 ratio
		// liquid = (1000 - 0.01) * 1_009_899_901_000_000_000 / 201_979_980_200_000_000 * 0.99
		liquid = 4_949_950_500_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), liquid);

		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::Minted(BOB, amount, liquid))
		);
	});
}

#[test]
fn repeated_mints_have_similar_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = dollar(1000);

		assert_ok!(HomaLite::set_minting_cap(
			Origin::signed(ROOT),
			5 * dollar(INITIAL_BALANCE)
		));

		// Set the total staking amount
		let mut lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, dollar(1_000_000));

		// Set the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::signed(ROOT),
			lksm_issuance / 5
		));

		// The exchange rate is now 1:5 ratio
		// liquid = (1000 - 0.01) * 1000 / 200 * 0.99
		let liquid_1 = 4_949_950_500_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), liquid_1);
		// The effective exchange rate is lower than the theoretical rate.
		assert!(liquid_1 < dollar(5000));

		// New total issuance
		lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_004_949_950_500_000_000);
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(201_000));

		// Second exchange
		// liquid = (1000 - 0.01) * 1004949.9505 / 201000 * 0.99
		let liquid_2 = 4_949_703_990_002_437;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 9_899_654_490_002_437);

		// Since the effective exchange rate is lower than the theortical rate, Liquid currency becomes more
		// valuable.
		assert!(liquid_1 > liquid_2);

		// The effective exchange rate should be quite close.
		// In this example the difffence is about 0.005%
		assert!(Permill::from_rational(liquid_1 - liquid_2, liquid_1) < Permill::from_rational(5u128, 1_000u128));

		// Now increase the Staking total by 1%
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(202_000));
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::signed(ROOT),
			dollar(204_020)
		));
		lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_009_899_654_490_002_437);

		// liquid = (1000 - 0.01) * 1009899.654490002437 / 204020 * 0.99
		let liquid_3 = 4_900_454_170_858_361;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 14_800_108_660_860_799);

		// Increasing the Staking total increases the value of Liquid currency - this makes up for the
		// staking rewards.
		assert!(liquid_3 < liquid_2);
		assert!(liquid_3 < liquid_1);
	});
}

#[test]
fn mint_fails_when_cap_is_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::signed(ROOT), dollar(1_000)));

		assert_noop!(
			HomaLite::mint(Origin::signed(ALICE), dollar(1_001)),
			Error::<Runtime>::ExceededStakingCurrencyMintCap
		);

		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(1_000)));

		assert_noop!(
			HomaLite::mint(Origin::signed(ALICE), dollar(1)),
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
			HomaLite::mint(Origin::signed(INVALID_CALLER), dollar(1)),
			DispatchError::Other("invalid caller"),
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

#[test]
fn can_set_xcm_dest_weight() {
	ExtBuilder::default().build().execute_with(|| {
		// Requires Root previlege.
		assert_noop!(
			HomaLite::set_xcm_dest_weight(Origin::signed(ALICE), 1_000_000),
			BadOrigin
		);

		// Set the cap.
		assert_ok!(HomaLite::set_xcm_dest_weight(Origin::signed(ROOT), 1_000_000));

		// Cap should be set now.
		assert_eq!(XcmDestWeight::<Runtime>::get(), 1_000_000);

		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::HomaLite(crate::Event::XcmDestWeightSet(1_000_000))
		);
	});
}
