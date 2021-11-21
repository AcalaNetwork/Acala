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

//! Unit tests using a mock with no fees.
//! This is mainly used to test economic model.

#![cfg(test)]

use super::*;
use frame_support::assert_ok;
use mock_no_fees::{dollar, Currencies, Event, ExtBuilder, HomaLite, Origin, System, ALICE, BOB, DAVE, KSM, LKSM};

#[test]
fn no_fee_runtime_has_no_fees() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));

		// Mint costs no fees
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(1_000)));
		assert_eq!(
			HomaLite::get_exchange_rate(),
			ExchangeRate::saturating_from_rational(1, 10)
		);
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(
			ALICE,
			dollar(1_000),
			dollar(10_000),
		)));
		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(999_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(10_000));

		assert_ok!(HomaLite::mint(Origin::signed(BOB), dollar(5_000)));
		System::assert_last_event(Event::HomaLite(crate::Event::Minted(
			BOB,
			dollar(5_000),
			dollar(50_000),
		)));
		assert_eq!(Currencies::free_balance(KSM, &BOB), dollar(995_000));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), dollar(50_000));

		//Redeem costs no fees
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(BOB),
			dollar(50_000),
			Permill::zero()
		));
		System::assert_last_event(Event::HomaLite(crate::Event::RedeemRequested(
			BOB,
			dollar(50_000),
			Permill::zero(),
			0,
		)));
		assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(5_000)));

		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(994_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(60_000));
		assert_eq!(Currencies::free_balance(KSM, &BOB), dollar(1_000_000));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), 0);
		let events = System::events();
		assert_eq!(
			events[events.len() - 2].event,
			Event::HomaLite(crate::Event::Redeemed(BOB, dollar(5000), dollar(50000),))
		);
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Minted(ALICE, dollar(5000), dollar(50000),))
		);

		// Redeem from AvailableStakingBalance costs no fees
		assert_ok!(HomaLite::schedule_unbond(Origin::root(), dollar(50_000), 0));
		let _ = HomaLite::on_idle(0, 5_000_000_000);

		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(100_000),
			Permill::zero()
		));

		assert_eq!(HomaLite::available_staking_balance(), dollar(40_000));
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(10_000));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(900_000));

		let events = System::events();
		assert_eq!(
			events[events.len() - 9].event,
			Event::HomaLite(crate::Event::ScheduledUnbondAdded(dollar(50_000), 0))
		);
		assert_eq!(
			events[events.len() - 8].event,
			Event::HomaLite(crate::Event::ScheduledUnbondWithdrew(dollar(50_000)))
		);
		assert_eq!(
			events[events.len() - 7].event,
			Event::Tokens(orml_tokens::Event::Reserved(LKSM, DAVE, dollar(100_000)))
		);
		assert_eq!(
			events[events.len() - 6].event,
			Event::HomaLite(crate::Event::RedeemRequested(DAVE, dollar(100_000), Permill::zero(), 0))
		);
		assert_eq!(
			events[events.len() - 5].event,
			Event::HomaLite(crate::Event::TotalStakingCurrencySet(dollar(96_000)))
		);
		assert_eq!(
			events[events.len() - 4].event,
			Event::Tokens(orml_tokens::Event::Endowed(KSM, DAVE, dollar(10_000)))
		);
		assert_eq!(
			events[events.len() - 3].event,
			Event::Currencies(module_currencies::Event::Deposited(KSM, DAVE, dollar(10_000)))
		);
		assert_eq!(
			events[events.len() - 2].event,
			Event::Tokens(orml_tokens::Event::Unreserved(LKSM, DAVE, dollar(100_000)))
		);
		assert_eq!(
			events[events.len() - 1].event,
			Event::HomaLite(crate::Event::Redeemed(DAVE, dollar(10_000), dollar(100_000)))
		);
	});
}

#[test]
fn mint_with_xcm_does_not_change_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));

		let exchange_rate = HomaLite::get_exchange_rate();

		for _ in 0..100 {
			assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(500)));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(950_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(500_000));
	});
}

#[test]
fn mint_with_redeem_does_not_change_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(1_000_000),
			Permill::zero()
		));
		let exchange_rate = HomaLite::get_exchange_rate();

		for _ in 0..100 {
			assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(500)));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(950_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(500_000));

		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(50_000));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), 0);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), dollar(500_000));

		// Add redeem with 50% extra reward.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(ALICE),
			dollar(500_000),
			Permill::from_percent(50)
		));

		for _ in 0..100 {
			assert_ok!(HomaLite::mint(Origin::signed(BOB), dollar(1_000)));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		// 950_000 + 50_000 * 50%, since the other 50% went to the minter as rewards.
		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(975_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), 0);

		// Got 25_000 extra as extra rewards
		assert_eq!(Currencies::free_balance(KSM, &BOB), dollar(925_000));
		assert_eq!(Currencies::free_balance(LKSM, &BOB), dollar(1_000_000));

		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(100_000));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), 0);
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
	});
}

#[test]
fn redeem_with_available_staking_does_not_change_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));

		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			dollar(100) as i128,
			100
		));

		let exchange_rate = HomaLite::get_exchange_rate();

		// test repeated redeem using available staking
		for _ in 0..100 {
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(DAVE),
				dollar(10),
				Permill::zero()
			));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		assert_eq!(HomaLite::available_staking_balance(), 0);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(100));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(999_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);

		// Test repeated adjust_available_staking_balance with a queued redeem request.
		assert_ok!(HomaLite::request_redeem(
			Origin::signed(DAVE),
			dollar(10_000),
			Permill::zero()
		));
		for _ in 0..100 {
			assert_ok!(HomaLite::adjust_available_staking_balance(
				Origin::root(),
				dollar(10) as i128,
				100
			));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		assert_eq!(HomaLite::available_staking_balance(), 0);
		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(1100));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(989_000));
		assert_eq!(Currencies::reserved_balance(LKSM, &DAVE), 0);
	});
}

#[test]
fn mint_and_redeem_at_the_same_time_does_not_change_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));
		assert_ok!(HomaLite::adjust_available_staking_balance(
			Origin::root(),
			dollar(5_000) as i128,
			0
		));

		let exchange_rate = HomaLite::get_exchange_rate();

		// The first 50 redeems are done using available_staking_balance.
		// The next 50 redeems are matched with mint.
		for _ in 0..100 {
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(DAVE),
				dollar(1000),
				Permill::zero()
			));
			assert_ok!(HomaLite::mint(Origin::signed(ALICE), dollar(100)));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}

		assert_eq!(Currencies::free_balance(KSM, &ALICE), dollar(990_000));
		assert_eq!(Currencies::free_balance(LKSM, &ALICE), dollar(100_000));

		assert_eq!(Currencies::free_balance(KSM, &DAVE), dollar(10_000));
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(900_000));
	});
}

#[test]
fn updaing_and_cancelling_redeem_requests_does_not_change_exchange_rate() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(HomaLite::set_total_staking_currency(
			Origin::root(),
			Currencies::total_issuance(LKSM) / 10
		));
		assert_ok!(HomaLite::set_minting_cap(Origin::root(), dollar(1_000_000)));

		let exchange_rate = HomaLite::get_exchange_rate();

		for i in 1..101 {
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(DAVE),
				dollar(i * 100u128),
				Permill::from_percent(i as u32)
			));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}
		assert_eq!(HomaLite::redeem_requests(DAVE), Some((dollar(10_000), Permill::one())));

		for i in 1..101 {
			assert_ok!(HomaLite::request_redeem(
				Origin::signed(DAVE),
				dollar((100 - i) * 100u128),
				Permill::from_percent(100 - i as u32)
			));
			assert_eq!(exchange_rate, HomaLite::get_exchange_rate());
		}
		assert_eq!(HomaLite::redeem_requests(DAVE), None);

		assert_eq!(Currencies::free_balance(KSM, &DAVE), 0);
		assert_eq!(Currencies::free_balance(LKSM, &DAVE), dollar(1_000_000));
	});
}
