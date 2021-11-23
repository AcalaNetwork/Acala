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
	dollar, millicent, Currencies, Event, ExtBuilder, HomaLite, MockRelayBlockNumberProvider, Origin, Runtime, System,
	ACALA, ALICE, BOB, CHARLIE, DAVE, INITIAL_BALANCE, INVALID_CALLER, KSM, LKSM,
};
use sp_runtime::traits::BadOrigin;

#[test]
fn mock_initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		let initial_dollar = dollar(INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(KSM, &ALICE), initial_dollar);
		assert_eq!(Currencies::free_balance(KSM, &BOB), initial_dollar);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &BOB), initial_dollar);
		assert_eq!(Currencies::free_balance(ACALA, &DAVE), initial_dollar);
	});
}

#[test]
fn mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = dollar(1000);

		assert_ok!(HomaLite::set_minting_cap(Origin::root(), 5 * dollar(INITIAL_BALANCE)));

		assert_noop!(
			HomaLite::mint(Origin::signed(DAVE), amount),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// Since the exchange rate is not set, use the default 1:10 ratio
		// liquid = (amount - MintFee) * 10 * (1 - MaxRewardPerEra)
		//        = 0.99 * (1000 - 0.01)  * 10 = 9899.901
		let mut liquid = 9_899_901_000_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), amount));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), liquid);
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(ALICE, amount, liquid)));
		// The total staking currency is now increased.
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1000));

		// Set the total staking amount
		let lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_009_899_901_000_000_000);

		// Set the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), lksm_issuance / 5));

		assert_eq!(
			HomaLite::get_exchange_rate(),
			ExchangeRate::saturating_from_rational(lksm_issuance / 5, lksm_issuance)
		);

		// The exchange rate is now 1:5 ratio
		// liquid = (1000 - 0.01) * 1_009_899_901_000_000_000 / 201_979_980_200_000_000 * 0.99
		liquid = 4_949_950_500_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), liquid);
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(BOB, amount, liquid)));
	});
}

#[test]
fn repeated_mints_have_similar_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		let amount = dollar(1000);

		assert_ok!(HomaLite::set_minting_cap(Origin::root(), 5 * dollar(INITIAL_BALANCE)));

		// Set the total staking amount
		let mut lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, dollar(1_000_000));

		// Set the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), lksm_issuance / 5));

		// The exchange rate is now 1:5 ratio
		// liquid = (1000 - 0.01) * 1000 / 200 * 0.99
		let liquid_1 = 4_949_950_500_000_000;
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		assert_eq!(Currencies::free_balance(KSM, &BOB), dollar(999_000));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), liquid_1);
		// The effective exchange rate is lower than the theoretical rate.
		assert!(liquid_1 < dollar(5000));

		// New total issuance
		lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_004_949_950_500_000_000);
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(201_000));

		// Second exchange
		// liquid = (1000 - 0.01) * 1004949.9505 / 201000 * 0.99
		let liquid_2 = 4_949_703_990_002_433; // Actual amount is lower due to rounding loss
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(BOB, amount, liquid_2)));
		assert_eq!(Currencies::free_balance(KSM, &BOB), 998_000_000_000_000_001);
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 9_899_654_490_002_433);

		// Since the effective exchange rate is lower than the theortical rate, Liquid currency becomes more
		// valuable.
		assert!(liquid_1 > liquid_2);

		// The effective exchange rate should be quite close.
		// In this example the difffence is about 0.005%
		assert!(Permill::from_rational(liquid_1 - liquid_2, liquid_1) < Permill::from_rational(5u128, 1_000u128));

		// Now increase the Staking total by 1%
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), 201_999_999_999_999_999);
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(204_020)));
		lksm_issuance = Currencies::total_issuance(LKSM);
		assert_eq!(lksm_issuance, 1_009_899_654_490_002_433);

		// liquid = (1000 - 0.01) * 1009899.654490002433 / 204020 * 0.99
		let liquid_3 = 4_900_454_170_858_356; // Actual amount is lower due to rounding loss
		assert_ok!(HomaLite::mint(Origin::signed(BOB), amount));
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(BOB, amount, liquid_3)));
		assert_eq!(Currencies::free_balance(KSM, &BOB), 997_000_000_000_000_002);
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 14_800_108_660_860_789);

		// Increasing the Staking total increases the value of Liquid currency - this makes up for the
		// staking rewards.
		assert!(liquid_3 < liquid_2);
		assert!(liquid_3 < liquid_1);
	});
}

#[test]
fn mint_fails_when_cap_is_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000)));

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
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000)));

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
			HomaLite::set_total_staking_currency(Origin::root(), 0),
			Error::<Runtime>::InvalidTotalStakingCurrency
		);
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), 1));
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), 1);
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(1)));
	});
}

#[test]
fn can_adjust_total_staking_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), 1));
		assert_eq!(HomaLite::total_staking_currency(), 1);

		assert_noop!(
			HomaLite::adjust_total_staking_currency(Origin::signed(ALICE), 5000i128),
			BadOrigin
		);

		// Can adjust total_staking_currency with DAVE.
		assert_ok!(HomaLite::adjust_total_staking_currency(Origin::root(), 5000i128));
		assert_eq!(HomaLite::total_staking_currency(), 5001);
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(5001)));

		// Can decrease total_staking_currency.
		assert_ok!(HomaLite::adjust_total_staking_currency(Origin::root(), -5000i128));
		assert_eq!(HomaLite::total_staking_currency(), 1);
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(1)));

		// overflow can be handled
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Balance::max_value()
		));

		assert_ok!(HomaLite::adjust_total_staking_currency(Origin::root(), 1i128));
		assert_eq!(HomaLite::total_staking_currency(), Balance::max_value());

		// Do not allow TotalStakingCurrency to become 0
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), 5000));
		assert_noop!(
			HomaLite::adjust_total_staking_currency(Origin::root(), -5000i128),
			Error::<Runtime>::InvalidTotalStakingCurrency
		);
		assert_eq!(HomaLite::total_staking_currency(), 5000);

		// TotalStakingCurrency must be at least 1
		assert_ok!(HomaLite::adjust_total_staking_currency(Origin::root(), -4999i128));
	});
}

#[test]
fn can_adjust_available_staking_balance_with_no_matches() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			HomaLite::adjust_available_staking_balance(Origin::signed(ALICE), 5000i128, 10),
			BadOrigin
		);

		// Can adjust available_staking_balance with DAVE.
		assert_ok!(HomaLite::adjust_available_staking_balance(Origin::root(), 5001i128, 10));
		assert_eq!(HomaLite::available_staking_balance(), 5001);
		System::assert_last_event(Event::HomaLite(crate::Event::AvailableStakingBalanceSet(5001)));

		// Can decrease available_staking_balance.
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			-5001i128,
			10
		));
		assert_eq!(HomaLite::total_staking_currency(), 0);
		System::assert_last_event(Event::HomaLite(crate::Event::AvailableStakingBalanceSet(0)));

		// Underflow / overflow can be handled due to the use of saturating arithmetic
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			-10_000i128,
			10
		));
		assert_eq!(HomaLite::available_staking_balance(), 0);

		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			i128::max_value(),
			10
		));
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			i128::max_value(),
			10
		));
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			i128::max_value(),
			10
		));
		assert_eq!(HomaLite::available_staking_balance(), Balance::max_value());
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

		// Requires Root privilege.
		assert_noop!(
			HomaLite::set_minting_cap(Origin::signed(ALICE), dollar(1_000)),
			BadOrigin
		);

		// Set the cap.
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000)));

		// Cap should be set now.
		assert_eq!(StakingCurrencyMintCap::<Runtime>::get(), dollar(1_000));

		System::assert_last_event(Event::HomaLite(crate::Event::StakingCurrencyMintCapUpdated(dollar(
			1_000,
		))));
	});
}

#[test]
fn can_set_xcm_dest_weight() {
	ExtBuilder::default().build().execute_with(|| {
		// Requires Root privilege.
		assert_noop!(
			HomaLite::set_xcm_dest_weight(Origin::signed(ALICE), 1_000_000),
			BadOrigin
		);

		// Set the cap.
		assert_ok!(HomaLite::set_xcm_dest_weight(Origin::root(), 1_000_000));

		// Cap should be set now.
		assert_eq!(XcmDestWeight::<Runtime>::get(), 1_000_000);

		System::assert_last_event(Event::HomaLite(crate::Event::XcmDestWeightSet(1_000_000)));
	});
}

#[test]
fn can_schedule_unbond() {
	ExtBuilder::default().build().execute_with(|| {
		// Requires Root privilege.
		assert_noop!(
			HomaLite::schedule_unbond(Origin::signed(ALICE), 1_000_000, 100),
			BadOrigin
		);

		// Schedule an unbond.
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), 1_000_000, 100));

		// Storage should be updated now.
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(1_000_000, 100)]);

		System::assert_last_event(Event::HomaLite(crate::Event::ScheduledUnbondAdded(1_000_000, 100)));

		// Schedule another unbond.
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), 200, 80));

		// Storage should be updated now.
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(1_000_000, 100), (200, 80)]);

		System::assert_last_event(Event::HomaLite(crate::Event::ScheduledUnbondAdded(200, 80)));
	});
}

#[test]
fn can_replace_schedule_unbond() {
	ExtBuilder::default().build().execute_with(|| {
		// Requires Root privilege.
		assert_noop!(
			HomaLite::replace_schedule_unbond(Origin::signed(ALICE), vec![(1_000_000, 100)]),
			BadOrigin
		);

		// Schedule an unbond.
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), 1_000_000, 100));
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), 200, 80));
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(1_000_000, 100), (200, 80)]);

		// replace the current storage.
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(800, 2), (1357, 120)],
		));
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(800, 2), (1357, 120)]);

		System::assert_last_event(Event::HomaLite(crate::Event::ScheduledUnbondReplaced));
	});
}

// on_idle can call xcm to increase AvailableStakingBalance
#[test]
fn on_idle_can_process_xcm_to_increase_available_staking_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(100, 1), (200, 2), (30, 2)],
		));
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(100, 1), (200, 2), (30, 2)]);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);

		// Block number 0 has nothing scheduled
		MockRelayBlockNumberProvider::set(0);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(100, 1), (200, 2), (30, 2)]);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);

		// Block number 1
		MockRelayBlockNumberProvider::set(1);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(200, 2), (30, 2)]);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 100);

		// Block number 2. Each on_idle call should unbond one item.
		MockRelayBlockNumberProvider::set(2);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(30, 2)]);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 300);

		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![]);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 330);
	});
}

// New available staking balances can redeem queued requests immediately
#[test]
fn new_available_staking_currency_can_handle_redeem_requests() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(1_000), 1)],
		));
		MockRelayBlockNumberProvider::set(1);

		// Added some redeem_requests to the queue
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(11_000),
			Permill::zero()
		));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(10_989), Permill::zero()))
		);

		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(0));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(10_989));

		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		// All available staking currency should be redeemed, paying the `XcmUnbondFee`
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1); // rounding error
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(999) - 1); // rounding error
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(98911) / 100);
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(98911) / 100, Permill::zero()))
		);

		// Add more redeem request
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(1_000) as i128
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(1_000),
			Permill::zero()
		));
		// 1000 - withdraw_fee = 999
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ALICE),
			Some((dollar(999), Permill::zero()))
		);

		// Change the exchange rate to 1(S) : 10(L)
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Add more staking currency by adjust_available_staking_balance also
		// automatically fullfill pending redeem request.
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			dollar(200) as i128,
			10
		));

		// The 2 remaining requests are redeemed, the leftover is stored.
		// available_staking_remain = 200 -  99.9 - 98.911 = 1.189
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1_189_000_000_001);

		assert_eq!(RedeemRequests::<Runtime>::get(&ALICE), None);
		assert_eq!(HomaLite::get_exchange_rate(), Ratio::saturating_from_rational(1, 10));
		// staking_gained = 99.9 - 1 (xcm_fee) = 98.9
		assert_eq!(
			Currencies::free_balance(KSM, &ALICE),
			dollar(INITIAL_BALANCE) + dollar(989) / 10
		);
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 0);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);

		// The last request is redeemed, the leftover is stored.
		// staking = 999(first redeem) + 98.911(this redeem) - 1(xcm_fee) = 1096.911 (with rounding error)
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 1_096_910_999_999_999);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&DAVE), None);
	});
}

// Exchange rate can change when redeem requests are waiting in queue.
// Test if on_idle can handle exchange ratio changes
#[test]
fn on_idle_can_handle_changes_in_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// When redeem was requested, 100_000 is redeemed to 10_000 staking currency
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100_000),
			Permill::zero()
		));

		// Set the total staking amount
		assert_eq!(Currencies::total_issuance(LKSM), dollar(999_900));

		// Change the exchange rate to 1(S) : 5(L)
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(200_000)));

		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(100_000), 1)],
		));
		MockRelayBlockNumberProvider::set(1);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		// All available staking currency should be redeemed.
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 80_018_001_800_180_019);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 19_980_998_199_819_981);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(900_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&DAVE), None);
	});
}

// Redeem can be redeemed immediately if there are staking staking balance.
// Redeem requests unfulfilled are added to the queue.
#[test]
fn request_redeem_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			50_000_000_000_000_000,
			10
		));

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), dollar(50_000));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Redeem amount has to be above a threshold.
		assert_noop!(
			HomaLite::request_redeem(Origin::signed(DAVE), dollar(1), Permill::zero()),
			Error::<Runtime>::AmountBelowMinimumThreshold
		);

		// the user must have sufficient funds to request redeem.
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(1_000_000));
		assert_noop!(
			HomaLite::request_redeem(Origin::signed(DAVE), dollar(1_000_001), Permill::zero()),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// When there are staking balances available, redeem requests are completed immediately, with fee
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 40_009_000_900_090_010);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 9_989_999_099_909_990);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(900_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&DAVE), None);

		// check the correct events are emitted
		let events = System::events();

		// Reserved LKSM with withdraw fee deducted
		assert_eq!(
			events[events.len() - 7].event,
			Event::Tokens(orml_tokens::Event::Reserved(LKSM, DAVE, dollar(99_900)))
		);
		// Redeem requested, with some withdraw fee deducted.
		assert_eq!(
			events[events.len() - 6].event,
			Event::HomaLite(crate::Event::RedeemRequested(
				DAVE,
				dollar(99_900),
				Permill::zero(),
				dollar(100)
			))
		);
		// The request is redeemed with available_staking_balances. TotalStaking is adjusted.
		assert_eq!(
			events[events.len() - 5].event,
			Event::HomaLite(crate::Event::TotalStakingCurrencySet(90_009_000_900_090_010))
		);
		// Deposit KSM into redeemer's account
		assert_eq!(
			events[events.len() - 4].event,
			Event::Tokens(orml_tokens::Event::Endowed(KSM, DAVE, 9_989_999_099_909_990))
		);
		assert_eq!(
			events[events.len() - 3].event,
			Event::Currencies(module_currencies::Event::Deposited(KSM, DAVE, 9_989_999_099_909_990))
		);
		// Reserved LKSM is unreserved and slashed.
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Unreserved(LKSM, DAVE, dollar(99_900)))
		);
		// Some amount of the request is redeemed
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Redeemed(DAVE, 9_989_999_099_909_990, dollar(99_900)))
		);

		// Redeem requests can be partially filled.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(500_000),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 49_997_999_999_999_999);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(400_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 99_672_249_999_999_994);
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((99_672_249_999_999_994, Permill::zero()))
		);

		// When no available_staking_balance, add the redeem order to the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(150_000),
			Permill::zero()
		));

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 49_997_999_999_999_999);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), 349_672_249_999_999_994);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 149_949_672_250_000_000);
	});
}

#[test]
fn update_redeem_request_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// If the user doesn't have enough liquid currency, redeem fails.
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(1_000_000));
		assert_noop!(
			HomaLite::request_redeem(Origin::signed(DAVE), dollar(1_000_001), Permill::zero()),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);

		// Add the redeem order to the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(999), Permill::zero()))
		);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 0);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(999_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(999));

		// Adding extra value to the queue should only charge BaseWithdrawFee on the difference.
		// Also reserve the difference.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(2_000),
			Permill::zero()
		));

		let withdraw_fee = dollar(1001) / 1000; //BaseWithdrawFee::get().mul(diff_amount);
		let amount_reserved = dollar(999_999) / 1000; //diff_amount - withdraw_fee;
		let new_redeem_amount = 1_998_999_000_000_000; //dollar(2_000) - withdraw_fee;

		assert_eq!(Currencies::free_balance(KSM, &DAVE), 0);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(997_999));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), new_redeem_amount);

		// request_redeem replaces existing item in the queue, not add to it.
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((new_redeem_amount, Permill::zero()))
		);

		// check the correct events are emitted
		let events = System::events();
		// Reserved the extra LKSM
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Reserved(LKSM, DAVE, amount_reserved))
		);
		// Redeem requested, with some withdraw fee deducted.
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::RedeemRequested(
				DAVE,
				new_redeem_amount,
				Permill::zero(),
				withdraw_fee
			))
		);

		// Reducing the redeem amount unlocks the fund, but doesn't refund fee.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));

		assert_eq!(Currencies::free_balance(KSM, &DAVE), 0);
		// previous balance + returned = dollar(997_999) + 998.999
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), 998_997_999_000_000_000);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(1_000));

		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(1_000), Permill::zero()))
		);

		// check the correct events are emitted
		let events = System::events();
		// Unreserved the difference
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Unreserved(LKSM, DAVE, 998_999_000_000_000))
		);
		// Redeem requested, with no withdraw fee charged.
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::RedeemRequested(DAVE, dollar(1000), Permill::zero(), 0))
		);

		// When updating redeem request, the user must have enough liquid currency.
		assert_noop!(
			HomaLite::request_redeem(Origin::signed(DAVE), dollar(1_000_001), Permill::zero()),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn skip_redeem_requests_if_not_enough_reserved_liquid_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Redeem via mint fails if redeemer doesn't have enough reserve
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(999));
		assert_eq!(HomaLite::redeem_requests(&DAVE), Some((dollar(999), Permill::zero())));

		// Unreserve some money
		Currencies::unreserve(LKSM, &DAVE, dollar(499));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(500));

		// This mint is matched with redeem request since there are more than ~400 liquid in reserve.
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(40)));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 399_999_600_000_000);

		assert_eq!(
			HomaLite::redeem_requests(&DAVE),
			Some((599_000_400_000_000, Permill::zero()))
		);
		// Redeemed 40 KSM with rounding error
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(40) - 1);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 100_000_400_000_000);

		// Mint will skip the redeem request with insufficient reserved balance, without returning Error
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(1_000)));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 10_299_890_700_098_990);

		// Mint is done via XCM, redeem request is unaffected.
		assert_eq!(
			HomaLite::redeem_requests(&DAVE),
			Some((599_000_400_000_000, Permill::zero()))
		);
		// Redeemed 40 KSM with rounding error
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(40) - 1);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 100_000_400_000_000);

		// Matching with AvailableStakingBalance will skip the redeem request due to insufficient balance.
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			dollar(1_000) as i128,
			10
		));
		assert_eq!(HomaLite::available_staking_balance(), dollar(1_000));

		// Redeem request is unaffected.
		assert_eq!(
			HomaLite::redeem_requests(&DAVE),
			Some((599_000_400_000_000, Permill::zero()))
		);
		// Redeemed 40 KSM with rounding error
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(40) - 1);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 100_000_400_000_000);
	});
}

// request_redeem can handle dust redeem requests
#[test]
fn request_redeem_can_handle_dust_redeem_requests() {
	ExtBuilder::empty().build().execute_with(|| {
		let staking_amount = dollar(500_000) - millicent(1000);
		let liquid_amount = dollar(5_000_000);

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			liquid_amount as i128
		));
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			staking_amount as i128,
			10
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), staking_amount);

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Remaining is below the xcm_unbond_fee `dollar(1)`, therefore returned and requests filled.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			liquid_amount,
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1);
		assert_eq!(Currencies::free_balance(KSM, &ALICE), 499_998_989_999_999_999);

		// Remaining dust is returned
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 99_899_999_996);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ALICE), None);
	});
}

// on_idle can handle dust redeem requests
#[test]
fn on_idle_can_handle_dust_redeem_requests() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(500_501) as i128
		));

		// This amount will leave a dust after redeem
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(500_501),
			Permill::zero()
		));
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(50_000), 2)],
		));
		MockRelayBlockNumberProvider::set(2);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);
		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(49_999));
		// Dust amount is un-reserved and returned to the user
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 499_000_000_000);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ALICE), None);
	});
}

// mint can handle dust redeem requests
#[test]
fn mint_can_handle_dust_redeem_requests() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(INITIAL_BALANCE)));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			1_001_001_101_101_101 as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			KSM,
			dollar(101) as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Redeem enough for 100 KSM with dust remaining
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			1_001_001_101_101_101,
			Permill::zero()
		));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ALICE),
			Some((1_000_000_100_000_000, Permill::zero()))
		);
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 0);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 1_000_000_100_000_000);

		let mint_amount = HomaLite::convert_liquid_to_staking(1_000_000_000_000_000).unwrap();
		assert_eq!(mint_amount, 100_100_100_100_099);
		// Mint 100 KSM, remaining dust should be returned to the redeemer.
		assert_ok!(HomaLite::mint(Origin::signed(BOB), mint_amount));

		// some dust due to rounding error left
		assert_eq!(Currencies::free_balance(KSM, &BOB), 899_899_899_902);
		// Minted approximately $1000 LKSM
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 999_999_999_999_990);

		// Redeemed $100 KSM for ALICE, with rounding error
		assert_eq!(Currencies::free_balance(KSM, &ALICE), 100_100_100_100_098);
		// Dust LKSM is returned to the redeemer.
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 100_000_010);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ALICE), None);

		let events = System::events();
		assert_eq!(
			events[events.len() - 4].event,
			Event::Currencies(module_currencies::Event::Transferred(
				KSM,
				BOB,
				ALICE,
				100_100_100_100_098
			))
		);
		assert_eq!(
			events[events.len() - 3].event,
			Event::HomaLite(crate::Event::Redeemed(ALICE, 100_100_100_100_098, 999_999_999_999_990))
		);
		// Dust returned to redeemer
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Unreserved(LKSM, ALICE, 100_000_010))
		);
		// total amount minted, with rounding error
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Minted(BOB, 100_100_100_100_099, 999_999_999_999_990))
		);
	});
}

// can cancel redeem requests
#[test]
fn can_cancel_requested_redeem() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(99_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(99_900), Permill::zero()))
		);

		assert_ok!(HomaLite::request_redeem(Origin::signed(DAVE), 0, Permill::zero()));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&DAVE), None);
	});
}

// can replace redeem requests
#[test]
fn can_replace_requested_redeem() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(99_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(99_900), Permill::zero()))
		);

		// Reducing the amount unlocks the difference.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(50_000),
			Permill::from_percent(50)
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(50_000));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(50_000), Permill::from_percent(50)))
		);

		// Increasing the amount locks additional liquid currency.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(150_000),
			Permill::from_percent(10)
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(149_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(149_900), Permill::from_percent(10)))
		);
	});
}

// mint can match all redeem requests, up to the given limit
// can cancel redeem requests
#[test]
fn mint_can_match_requested_redeem() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(INITIAL_BALANCE)));
		assert_ok!(Currencies::deposit(LKSM, &ALICE, dollar(200)));
		assert_ok!(Currencies::deposit(LKSM, &BOB, dollar(200)));
		assert_ok!(Currencies::deposit(KSM, &CHARLIE, dollar(100)));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(200),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(200),
			Permill::zero()
		));

		assert_eq!(Currencies::free_balance(KSM, &CHARLIE), dollar(100));
		assert_eq!(Currencies::free_balance(LKSM, &CHARLIE), 0);

		// Minting request can match up to 2 requests at a time. The rest is exchanged via XCM
		assert_ok!(HomaLite::mint(Origin::signed(CHARLIE), dollar(100)));

		// XCM will cost some fee
		assert_eq!(Currencies::free_balance(LKSM, &CHARLIE), 993_897_000_000_000);

		assert_eq!(RedeemRequests::<Runtime>::get(&ALICE), None);
		assert_eq!(RedeemRequests::<Runtime>::get(&BOB), None);
		assert_eq!(
			RedeemRequests::<Runtime>::get(&DAVE),
			Some((dollar(999) / 10, Permill::zero()))
		);

		let events = System::events();

		// Matching CHARLIE's mint with ALICE's redeem
		assert_eq!(
			events[events.len() - 9].event,
			Event::Tokens(orml_tokens::Event::RepatriatedReserve(
				LKSM,
				ALICE,
				CHARLIE,
				199_800_000_000_000,
				BalanceStatus::Free
			))
		);
		assert_eq!(
			events[events.len() - 8].event,
			Event::Currencies(module_currencies::Event::Transferred(
				KSM,
				CHARLIE,
				ALICE,
				19_980_000_000_000
			))
		);
		assert_eq!(
			events[events.len() - 7].event,
			Event::HomaLite(crate::Event::Redeemed(ALICE, 19_980_000_000_000, 199_800_000_000_000))
		);

		// Matching CHARLIE's mint with BOB's redeem
		assert_eq!(
			events[events.len() - 6].event,
			Event::Tokens(orml_tokens::Event::RepatriatedReserve(
				LKSM,
				BOB,
				CHARLIE,
				199_800_000_000_000,
				BalanceStatus::Free
			))
		);
		assert_eq!(
			events[events.len() - 5].event,
			Event::Currencies(module_currencies::Event::Transferred(
				KSM,
				CHARLIE,
				BOB,
				19_980_000_000_000
			))
		);
		assert_eq!(
			events[events.len() - 4].event,
			Event::HomaLite(crate::Event::Redeemed(BOB, 19_980_000_000_000, 199_800_000_000_000))
		);

		// Mint via XCM: 600 LKSM - XCM fee
		assert_eq!(
			events[events.len() - 3].event,
			Event::HomaLite(crate::Event::TotalStakingCurrencySet(60_040_000_000_000))
		);

		assert_eq!(
			events[events.len() - 2].event,
			Event::Currencies(module_currencies::Event::Deposited(LKSM, CHARLIE, 594_297_000_000_000))
		);

		// Finally the minted event that contains total KSM and LKSM minted
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Minted(CHARLIE, 100000000000000, 993_897_000_000_000))
		);
	});
}

// can_mint_for_requests
#[test]
fn can_mint_for_request() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), 5 * dollar(INITIAL_BALANCE)));
		assert_ok!(Currencies::deposit(LKSM, &ALICE, dollar(2_000)));
		assert_ok!(Currencies::deposit(LKSM, &BOB, dollar(3_000)));
		assert_ok!(Currencies::deposit(KSM, &CHARLIE, dollar(4_00)));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(2_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(3_000),
			Permill::zero()
		));

		// Prioritize ALICE and BOB's requests
		assert_ok!(HomaLite::mint_for_requests(
			Origin::signed(CHARLIE),
			dollar(400),
			vec![ALICE, BOB]
		));

		assert_eq!(HomaLite::redeem_requests(DAVE), Some((dollar(999), Permill::zero())));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(999));

		assert_eq!(HomaLite::redeem_requests(ALICE), None);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);
		assert_eq!(HomaLite::redeem_requests(BOB), Some((dollar(995), Permill::zero())));
		assert_eq!(Currencies::reserved_balance(LKSM, &BOB), dollar(995));

		assert_eq!(Currencies::free_balance(LKSM, &CHARLIE), dollar(4_000));
	});
}

// Extra fee is paid from the redeemer to the Minter
#[test]
fn request_redeem_extra_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), 5 * dollar(INITIAL_BALANCE)));
		assert_ok!(Currencies::deposit(LKSM, &ALICE, dollar(200)));
		assert_ok!(Currencies::deposit(KSM, &CHARLIE, dollar(30)));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100),
			Permill::from_percent(50)
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(200),
			Permill::from_percent(10)
		));

		assert_ok!(HomaLite::mint(Origin::signed(CHARLIE), dollar(30)));

		// DAVE exchanges 50L-> 5S + 5S(fee)
		assert_eq!(HomaLite::redeem_requests(DAVE), None);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);

		// ALICE exchanges 180L->18S + 2S(fee)
		assert_eq!(HomaLite::redeem_requests(ALICE), None);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), 0);

		// Extra fee + mint fee are rewarded to the minter
		assert_eq!(Currencies::free_balance(KSM, &CHARLIE), 6_993_000_000_000);
		assert_eq!(Currencies::free_balance(LKSM, &CHARLIE), 299_898_000_000_000);
	});
}

// Test staking and liquid conversion work
#[test]
fn staking_and_liquid_conversion_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Default exchange rate is 1(S) : 10(L)
		assert_eq!(HomaLite::get_exchange_rate(), Ratio::saturating_from_rational(1, 10));

		assert_eq!(HomaLite::convert_staking_to_liquid(1_000_000), Ok(10_000_000));
		assert_eq!(HomaLite::convert_liquid_to_staking(10_000_000), Ok(1_000_000));

		// Set the total staking amount so the exchange rate is 1(S) : 5(L)
		assert_eq!(Currencies::total_issuance(LKSM), dollar(1_000_000));
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(200_000)));

		assert_eq!(HomaLite::get_exchange_rate(), Ratio::saturating_from_rational(1, 5));

		assert_eq!(HomaLite::convert_staking_to_liquid(1_000_000), Ok(5_000_000));
		assert_eq!(HomaLite::convert_liquid_to_staking(5_000_000), Ok(1_000_000));
	});
}

#[test]
fn redeem_can_handle_dust_available_staking_currency() {
	ExtBuilder::default().build().execute_with(|| {
		// If AvailableStakingBalance is not enough to pay for the unbonding fee, ignore it.
		// pub XcmUnbondFee: Balance = dollar(1);
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			999_000_000,
			10
		));

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 999_000_000);

		// Ignore the dust AvailableStakingBalance and put the full amount onto the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1000),
			Permill::zero()
		));

		assert_eq!(HomaLite::redeem_requests(DAVE), Some((dollar(999), Permill::zero())));
		System::assert_last_event(Event::HomaLite(crate::Event::RedeemRequested(
			DAVE,
			dollar(999),
			Permill::zero(),
			dollar(1),
		)));
	});
}

#[test]
fn total_staking_currency_update_periodically() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(1_000_000)));

		let on_initialize_weight = <Runtime as Config>::WeightInfo::on_initialize();
		let on_initialize_without_work_weight = <Runtime as Config>::WeightInfo::on_initialize_without_work();

		// Interest rate isn't set yet - no interest rate calculation is done.
		assert_eq!(HomaLite::on_initialize(0), on_initialize_without_work_weight);
		// Default inflation rate is 0%
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1_000_000));

		for i in 1..100 {
			assert_eq!(HomaLite::on_initialize(i), on_initialize_without_work_weight);
		}
		// Interest rate isn't set yet - no interest rate calculation is done.
		assert_eq!(HomaLite::on_initialize(0), on_initialize_without_work_weight);
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1_000_000));

		// Interest rate can only be set by governance
		assert_noop!(
			HomaLite::set_staking_interest_rate_per_update(Origin::signed(ALICE), Permill::from_percent(1)),
			BadOrigin
		);
		assert_ok!(HomaLite::set_staking_interest_rate_per_update(
			Origin::root(),
			Permill::from_percent(1)
		));
		System::assert_last_event(Event::HomaLite(crate::Event::StakingInterestRatePerUpdateSet(
			Permill::from_percent(1),
		)));

		for i in 101..200 {
			assert_eq!(HomaLite::on_initialize(i), on_initialize_without_work_weight);
		}
		assert_eq!(HomaLite::on_initialize(200), on_initialize_weight);
		// Inflate by 1%: 1_000_000 * 1.01
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1_010_000));
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(dollar(
			1_010_000,
		))));

		for i in 201..300 {
			assert_eq!(HomaLite::on_initialize(i), on_initialize_without_work_weight);
		}
		assert_eq!(HomaLite::on_initialize(300), on_initialize_weight);
		// 1_010_000 * 1.01
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1_020_100));
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(dollar(
			1_020_100,
		))));

		for i in 301..400 {
			assert_eq!(HomaLite::on_initialize(i), on_initialize_without_work_weight);
		}
		assert_eq!(HomaLite::on_initialize(400), on_initialize_weight);
		//1_020_100 * 1.01
		assert_eq!(TotalStakingCurrency::<Runtime>::get(), dollar(1_030_301));
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(dollar(
			1_030_301,
		))));
	});
}

#[test]
fn process_scheduled_unbond_with_multiple_requests() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(100) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			LKSM,
			dollar(100) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			CHARLIE,
			LKSM,
			dollar(200) as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(40)));

		let rate1 = HomaLite::get_exchange_rate();
		assert_eq!(HomaLite::get_exchange_rate(), Ratio::saturating_from_rational(1, 10));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(100),
			Permill::zero()
		));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(100),
			Permill::zero()
		));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(CHARLIE),
			dollar(200),
			Permill::zero()
		));

		assert_ok!(HomaLite::replace_schedule_unbond(Origin::root(), vec![(dollar(30), 1)],));
		MockRelayBlockNumberProvider::set(1);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		let rate2 = HomaLite::get_exchange_rate();
		assert!(rate1 < rate2);

		// Some rounding error
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 1);

		// Some rounding error, 10 KSM - 1 KSM unbond fee
		assert_eq!(Currencies::free_balance(KSM, &ALICE), 8999999999999);
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 0);

		// 10 KSM - 1 KSM unbond fee
		assert_eq!(Currencies::free_balance(KSM, &BOB), 9000000000000);
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 0);

		// 10 KSM - 1 KSM unbond fee
		assert_eq!(Currencies::free_balance(KSM, &CHARLIE), 9000000000000);
		// 100 LKSM minus fee
		assert_eq!(Currencies::reserved_balance(LKSM, &CHARLIE), 99899999999996);
	});
}

#[test]
fn not_overcharge_redeem_fee() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(100) as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(Origin::root(), dollar(10)));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(50),
			Permill::zero()
		));

		let fee = dollar(50) / 1000;

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(50));
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), dollar(50) - fee);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(50) - fee,
			Permill::zero()
		));

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(50));
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), dollar(50) - fee);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(100) - fee,
			Permill::zero()
		));

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 0);
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), dollar(100) - fee * 2);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(20) - fee * 2,
			Permill::zero()
		));

		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(80));
		assert_eq!(Currencies::reserved_balance(LKSM, &ALICE), dollar(20) - fee * 2);
	});
}

#[test]
fn on_idle_matches_redeem_based_on_weights() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(INITIAL_BALANCE) as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// Schedule an unbond.
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), dollar(1_000_000), 0));
		MockRelayBlockNumberProvider::set(0);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(1_000),
			Permill::zero()
		));

		// Get the currently benchmarked weight.
		let xcm_weight = <Runtime as crate::Config>::WeightInfo::xcm_unbond();
		let redeem = <Runtime as crate::Config>::WeightInfo::redeem_with_available_staking_balance();

		// on_idle does nothing with insufficient weight
		assert_eq!(HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 0), 0);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(dollar(1_000_000), 0)]);
		assert_eq!(
			RedeemRequests::<Runtime>::get(DAVE),
			Some((dollar(999), Permill::zero()))
		);
		assert_eq!(
			RedeemRequests::<Runtime>::get(ALICE),
			Some((dollar(999), Permill::zero()))
		);

		// on_idle only perform XCM unbond with sufficient weight
		assert_eq!(
			HomaLite::on_idle(MockRelayBlockNumberProvider::get(), xcm_weight + 1),
			xcm_weight
		);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![]);
		assert_eq!(
			RedeemRequests::<Runtime>::get(DAVE),
			Some((dollar(999), Permill::zero()))
		);
		assert_eq!(
			RedeemRequests::<Runtime>::get(ALICE),
			Some((dollar(999), Permill::zero()))
		);

		// on_idle has weights to match only one redeem
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), dollar(1_000_000), 0));
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(dollar(1_000_000), 0)]);
		assert_eq!(
			HomaLite::on_idle(MockRelayBlockNumberProvider::get(), xcm_weight + redeem + 1),
			xcm_weight + redeem
		);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![]);
		assert_eq!(
			RedeemRequests::<Runtime>::get(DAVE),
			Some((dollar(999), Permill::zero()))
		);
		assert_eq!(RedeemRequests::<Runtime>::get(ALICE), None);

		// on_idle will match the remaining redeem request, even with no scheduled unbond.
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), dollar(1_000_000), 10));
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(dollar(1_000_000), 10)]);
		assert_eq!(
			HomaLite::on_idle(MockRelayBlockNumberProvider::get(), redeem + 1),
			redeem
		);
		assert_eq!(ScheduledUnbond::<Runtime>::get(), vec![(dollar(1_000_000), 10)]);
		assert_eq!(RedeemRequests::<Runtime>::get(DAVE), None);
		assert_eq!(RedeemRequests::<Runtime>::get(ALICE), None);
	});
}

#[test]
fn adjust_available_staking_balance_matches_redeem_based_on_input() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(INITIAL_BALANCE) as i128
		));

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			LKSM,
			dollar(INITIAL_BALANCE) as i128
		));

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(1_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(1_000),
			Permill::zero()
		));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));

		// match no redeem requests
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			dollar(1_000_000) as i128,
			0
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), dollar(1_000_000));

		// match only one request
		assert_ok!(HomaLite::adjust_available_staking_balance(Origin::root(), 1i128, 1));
		assert_eq!(
			RedeemRequests::<Runtime>::get(DAVE),
			Some((dollar(999), Permill::zero()))
		);
		assert_eq!(RedeemRequests::<Runtime>::get(BOB), None);
		assert_eq!(
			RedeemRequests::<Runtime>::get(ALICE),
			Some((dollar(999), Permill::zero()))
		);

		// match the remaining requests
		assert_ok!(HomaLite::adjust_available_staking_balance(Origin::root(), 1, 10));
		assert_eq!(RedeemRequests::<Runtime>::get(DAVE), None);
		assert_eq!(RedeemRequests::<Runtime>::get(ALICE), None);
		assert_eq!(RedeemRequests::<Runtime>::get(BOB), None);
	});
}

#[test]
fn available_staking_balances_can_handle_rounding_error_dust() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(5_000) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			LKSM,
			dollar(2_000) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			DAVE,
			LKSM,
			dollar(3_000) as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			1_000_237_000_000_000
		));
		let staking_amount = 999_999_999_999;
		let liquid_amount = HomaLite::convert_staking_to_liquid(staking_amount).unwrap();
		let staking_amount2 = HomaLite::convert_liquid_to_staking(liquid_amount).unwrap();
		assert_ne!(staking_amount, staking_amount2);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(5_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(2_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(3_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(999_999_999_999, 1)],
		));
		MockRelayBlockNumberProvider::set(1);

		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		// Dust AvailableStakingBalance remains
		assert_eq!(HomaLite::available_staking_balance(), 1);
		let events = System::events();
		assert_eq!(
			events[events.len() - 4].event,
			Event::HomaLite(crate::Event::ScheduledUnbondWithdrew(999_999_999_999))
		);
		assert_eq!(
			events[events.len() - 3].event,
			Event::HomaLite(crate::Event::TotalStakingCurrencySet(999_237_000_000_002))
		);
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Unreserved(LKSM, ALICE, 9_987_632_930_985))
		);
		// Actual staking deposited has `T::XcmUnbondFee` deducted
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Redeemed(ALICE, 0, 9_987_632_930_985))
		);
	});
}

#[test]
fn mint_can_handle_rounding_error_dust() {
	ExtBuilder::empty().build().execute_with(|| {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			LKSM,
			dollar(5_000) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			LKSM,
			dollar(2_000) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			DAVE,
			LKSM,
			dollar(3_000) as i128
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			DAVE,
			KSM,
			1_999_999_999_999 as i128
		));

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			1_000_237_000_000_000
		));
		let staking_amount = 999_999_999_999;
		let liquid_amount = HomaLite::convert_staking_to_liquid(staking_amount).unwrap();
		let staking_amount2 = HomaLite::convert_liquid_to_staking(liquid_amount).unwrap();
		assert_ne!(staking_amount, staking_amount2);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(5_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(2_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(3_000),
			Permill::zero()
		));
		assert_ok!(HomaLite::mint(Origin::signed(DAVE), 999_999_999_999,));

		// Dust is un-transferred from minter
		assert_eq!(Currencies::free_balance(KSM, &DAVE), 1000000000001);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), 9_987_632_930_985);

		let events = System::events();
		assert_eq!(
			events[events.len() - 5].event,
			Event::Tokens(orml_tokens::Event::RepatriatedReserve(
				LKSM,
				ALICE,
				DAVE,
				9_987_632_930_985,
				BalanceStatus::Free
			))
		);
		assert_eq!(
			events[events.len() - 3].event,
			Event::Currencies(module_currencies::Event::Transferred(KSM, DAVE, ALICE, 999_999_999_998))
		);
		// actual staking transferred is off due to rounding error
		assert_eq!(
			events[events.len() - 2].event,
			Event::HomaLite(crate::Event::Redeemed(ALICE, 999_999_999_998, 9_987_632_930_985))
		);
		// total amount minted includes dust caused by rounding error
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Minted(DAVE, 999_999_999_999, 9_987_632_930_985))
		);
	});
}
