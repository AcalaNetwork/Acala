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
	dollar, Currencies, Event, ExtBuilder, HomaLite, MockRelayBlockNumberProvider, Origin, Runtime, System, ACALA,
	ALICE, BOB, CHARLIE, INITIAL_BALANCE, INVALID_CALLER, KSM, LKSM, ROOT,
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

		assert_ok!(HomaLite::set_minting_cap(Origin::root(), 5 * dollar(INITIAL_BALANCE)));

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
			HomaLite::adjust_total_staking_currency(Origin::signed(ALICE), 5000),
			BadOrigin
		);

		// Can adjust total_staking_currency with ROOT.
		assert_ok!(HomaLite::adjust_total_staking_currency(Origin::root(), 5000));

		assert_eq!(HomaLite::total_staking_currency(), 5001);
		System::assert_last_event(Event::HomaLite(crate::Event::TotalStakingCurrencySet(5001)));

		// Underflow / overflow causes error
		assert_noop!(
			HomaLite::adjust_total_staking_currency(Origin::root(), -5002),
			ArithmeticError::Underflow
		);

		assert_eq!(HomaLite::total_staking_currency(), 5001);

		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Balance::max_value()
		));

		assert_noop!(
			HomaLite::adjust_total_staking_currency(Origin::root(), 1),
			ArithmeticError::Overflow
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
		// Requires Root previlege.
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
		// Requires Root previlege.
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
		// Requires Root previlege.
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
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(1_000), 1)],
		));
		MockRelayBlockNumberProvider::set(1);

		// Added some redeem_requests to the queue
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(11_000),
			Permill::zero()
		));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(10_989), Permill::zero()))
		);

		assert_eq!(Currencies::free_balance(KSM, &ROOT), dollar(0));
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(10_989));

		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		// All available staking currency should be redeemed, paying the `XcmUnbondFee`
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), dollar(999));
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(989));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(989), Permill::zero()))
		);

		// Add more staking currency to fully satify the last redeem request
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(150), 2)],
		));
		MockRelayBlockNumberProvider::set(2);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		// The last request is redeemed, the leftover is stored.
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 51_100_000_000_000);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), 1_096_900_000_000_000);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(0));
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// Exchange rate can change when redeem requests are waiting in queue.
// Test if on_idle can handle exchange ratio changes
#[test]
fn on_idle_can_handle_changes_in_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		// When redeem was requested, 100_000 is redeemed to 10_000 staking currency
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
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
		assert_eq!(Currencies::free_balance(KSM, &ROOT), 19_980_998_199_819_981);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(900_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// Redeem can be redeemed immediately if there are staking staking balance.
// Redeem requests unfullfilled are added to the queue.
#[test]
fn request_redeem_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(50_000), 1)],
		));
		MockRelayBlockNumberProvider::set(1);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), dollar(50_000));

		// Redeem amount has to be above a threshold.
		assert_noop!(
			HomaLite::request_redeem(Origin::signed(ROOT), dollar(1), Permill::zero()),
			Error::<Runtime>::AmountBelowMinimumThreshold
		);

		// When there are staking balances available, redeem requests are completed immediately, with fee
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), dollar(40_010));
		assert_eq!(Currencies::free_balance(KSM, &ROOT), dollar(9_989));
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(900_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);

		// Redeem requests can be partially filled.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(500_000),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), dollar(49_998));
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(400_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(99_400));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(99_400), Permill::zero()))
		);

		// When no available_staking_balance, add the redeem order to the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(150_000),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 0);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), dollar(49_998));
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(349_400));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 149949400000000000);
		// request_redeem replaces existing item in the queue, not add to it.
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((149949400000000000, Permill::zero()))
		);
	});
}

// request_redeem can handle dust redeem requests
#[test]
fn request_redeem_can_handle_dust_redeem_requests() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(50_000), 1)],
		));
		MockRelayBlockNumberProvider::set(1);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), dollar(50_000));

		// Remaining `dollar(1)` is below the xcm_unbond_fee, therefore returned and requests filled.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(500_010),
			Permill::zero()
		));
		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 49_001_000_000_000);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), 49_949_999_000_000_000);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(499_990));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// on_idle can handle dust redeem requests
#[test]
fn on_idle_can_handle_dust_redeem_requests() {
	ExtBuilder::default().build().execute_with(|| {
		// Test that on_idle doesn't add dust redeem requests into the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(500_010),
			Permill::zero()
		));
		assert_ok!(HomaLite::replace_schedule_unbond(
			Origin::root(),
			vec![(dollar(50_000), 2)],
		));
		MockRelayBlockNumberProvider::set(2);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 49_001_000_000_000);
		assert_eq!(Currencies::free_balance(KSM, &ROOT), 49_949_999_000_000_000);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(499_990));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// mint can handle dust redeem requests
#[test]
fn mint_can_handle_dust_redeem_requests() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(INITIAL_BALANCE)));

		// Test that on_idle doesn't add dust redeem requests into the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(500_010),
			Permill::zero()
		));

		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(50_000)));

		assert_eq!(Currencies::free_balance(KSM, &ROOT), 49_950_999_000_000_000);
		assert_eq!(Currencies::free_balance(LKSM, &ROOT), dollar(499_990));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// can cancel redeem requests
#[test]
fn can_cancel_requested_redeem() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(99_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(99_900), Permill::zero()))
		);

		assert_ok!(HomaLite::request_redeem(Origin::signed(ROOT), 0, Permill::zero()));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);
		assert_eq!(RedeemRequests::<Runtime>::get(&ROOT), None);
	});
}

// can replace redeem requests
#[test]
fn can_replace_requested_redeem() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(100_000),
			Permill::zero()
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(99_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(99_900), Permill::zero()))
		);

		// Reducing the amount unlocks the difference.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(50_000),
			Permill::from_percent(50)
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(50_000));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
			Some((dollar(50_000), Permill::from_percent(50)))
		);

		// Increasing the amount locks additional liquid currency.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(150_000),
			Permill::from_percent(10)
		));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(149_900));
		assert_eq!(
			RedeemRequests::<Runtime>::get(&ROOT),
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
			Origin::signed(ROOT),
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
			Origin::signed(ROOT),
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

		assert_eq!(HomaLite::redeem_requests(ROOT), Some((dollar(999), Permill::zero())));
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), dollar(999));

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
			Origin::signed(ROOT),
			dollar(100),
			Permill::from_percent(50)
		));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(200),
			Permill::from_percent(10)
		));

		assert_ok!(HomaLite::mint(Origin::signed(CHARLIE), dollar(30)));

		// ROOT exchanges 50L-> 5S + 5S(fee)
		assert_eq!(HomaLite::redeem_requests(ROOT), None);
		assert_eq!(Currencies::reserved_balance(LKSM, &ROOT), 0);

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
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), 999_000_000, 0));
		MockRelayBlockNumberProvider::set(0);
		HomaLite::on_idle(MockRelayBlockNumberProvider::get(), 5_000_000_000);

		assert_eq!(AvailableStakingBalance::<Runtime>::get(), 999_000_000);

		// Ignore the dust AvailableStakingBalance and put the full amount onto the queue.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ROOT),
			dollar(1000),
			Permill::zero()
		));

		assert_eq!(HomaLite::redeem_requests(ROOT), Some((dollar(999), Permill::zero())));
		System::assert_last_event(Event::HomaLite(crate::Event::RedeemRequested(
			ROOT,
			dollar(999),
			Permill::zero(),
		)));
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
