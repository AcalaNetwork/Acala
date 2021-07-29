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

//! Unit tests for the prices module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::{
	traits::{BadOrigin, Bounded},
	FixedPointNumber,
};

#[test]
fn lp_token_fair_price_works() {
	let lp_token_fair_price_0 = lp_token_fair_price(
		10000,
		20000,
		10000,
		Price::saturating_from_integer(100),
		Price::saturating_from_integer(200),
	)
	.unwrap();
	assert!(
		lp_token_fair_price_0 <= Price::saturating_from_integer(400)
			&& lp_token_fair_price_0 >= Price::saturating_from_integer(399)
	);

	assert_eq!(
		lp_token_fair_price(
			0,
			20000,
			10000,
			Price::saturating_from_integer(100),
			Price::saturating_from_integer(200)
		),
		None
	);
	assert_eq!(
		lp_token_fair_price(
			10000,
			0,
			10000,
			Price::saturating_from_integer(100),
			Price::saturating_from_integer(200)
		),
		Some(Price::from_inner(0))
	);
	assert_eq!(
		lp_token_fair_price(
			10000,
			20000,
			0,
			Price::saturating_from_integer(100),
			Price::saturating_from_integer(200)
		),
		Some(Price::from_inner(0))
	);
	assert_eq!(
		lp_token_fair_price(
			10000,
			20000,
			10000,
			Price::saturating_from_integer(100),
			Price::from_inner(0)
		),
		Some(Price::from_inner(0))
	);
	assert_eq!(
		lp_token_fair_price(
			10000,
			20000,
			10000,
			Price::from_inner(0),
			Price::saturating_from_integer(200)
		),
		Some(Price::from_inner(0))
	);

	assert_eq!(
		lp_token_fair_price(
			Balance::max_value(),
			Balance::max_value(),
			Balance::max_value(),
			Price::max_value() / Price::saturating_from_integer(2),
			Price::max_value() / Price::saturating_from_integer(2)
		),
		Some(Price::max_value() - Price::from_inner(1))
	);
	assert_eq!(
		lp_token_fair_price(
			Balance::max_value(),
			Balance::max_value(),
			Balance::max_value(),
			Price::max_value(),
			Price::max_value()
		),
		None
	);
}

#[test]
fn get_price_of_stable_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(AUSD, false),
			Some(Price::saturating_from_integer(1000000u128))
		); // 1 USD, right shift the decimal point (18-12) places

		assert_ok!(PricesModule::lock_price(Origin::signed(1), AUSD));
		assert_eq!(
			PricesModule::get_price(AUSD, true),
			Some(Price::saturating_from_integer(1000000u128))
		);

		mock_oracle_update();
		assert_eq!(
			PricesModule::get_price(AUSD, false),
			Some(Price::saturating_from_integer(1000000u128))
		);
	});
}

#[test]
fn get_price_of_liquid_currency() {
	ExtBuilder::default().build().execute_with(|| {
		// get real-time price
		assert_eq!(
			PricesModule::get_price(DOT, false),
			Some(Price::saturating_from_integer(10000000000u128))
		); // 100 USD, right shift the decimal point (18-12) places
		assert_eq!(
			PricesModule::get_price(LDOT, false),
			Some(Price::saturating_from_integer(5000000000u128))
		); // dot_price * 1/2

		// lock LDOT price
		assert_ok!(PricesModule::lock_price(Origin::signed(1), LDOT));
		assert_eq!(
			PricesModule::locked_price(LDOT),
			Some(Price::saturating_from_integer(5000000000u128))
		);

		// lock DOT price
		assert_ok!(PricesModule::lock_price(Origin::signed(1), DOT));
		assert_eq!(
			PricesModule::locked_price(DOT),
			Some(Price::saturating_from_integer(10000000000u128))
		);

		// mock oracle update
		mock_oracle_update();

		// get real-time price of LDOT, still use real-time price of DOT to calculate
		assert_eq!(
			PricesModule::get_price(DOT, false),
			Some(Price::saturating_from_integer(1000000000u128))
		); // 10 USD, right shift the decimal point (18-12) places
		assert_eq!(
			PricesModule::get_price(LDOT, false),
			Some(Price::saturating_from_integer(600000000u128))
		); // dot_price * 3/5

		// get locked price of LDOT
		assert_eq!(
			PricesModule::get_price(LDOT, true),
			Some(Price::saturating_from_integer(5000000000u128))
		);

		// unlock LDOT price
		assert_ok!(PricesModule::unlock_price(Origin::signed(1), LDOT));
		assert_eq!(
			PricesModule::get_price(LDOT, true),
			Some(Price::saturating_from_integer(600000000u128))
		);
	});
}

#[test]
fn get_price_of_dex_share_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(DOT, false),
			Some(Price::saturating_from_integer(10000000000u128))
		); // 100 USD, right shift the decimal point (18-12) places
		assert_eq!(
			PricesModule::get_price(AUSD, true),
			Some(Price::saturating_from_integer(1000000u128))
		);
		assert_eq!(Tokens::total_issuance(LP_AUSD_DOT), 0);
		assert_eq!(MockDEX::get_liquidity_pool(AUSD, DOT), (10000, 200));

		// the total issuance of dex share currency is zero
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, false), None);

		// issue LP
		assert_ok!(Tokens::deposit(LP_AUSD_DOT, &1, 100));
		assert_eq!(Tokens::total_issuance(LP_AUSD_DOT), 100);

		let lp_price_1 = lp_token_fair_price(
			Tokens::total_issuance(LP_AUSD_DOT),
			MockDEX::get_liquidity_pool(AUSD, DOT).0,
			MockDEX::get_liquidity_pool(AUSD, DOT).1,
			PricesModule::get_price(AUSD, false).unwrap(),
			PricesModule::get_price(DOT, false).unwrap(),
		);

		// get the real-time price of LP
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, false), lp_price_1);

		// lock LP price
		assert_ok!(PricesModule::lock_price(Origin::signed(1), LP_AUSD_DOT));
		assert_eq!(PricesModule::locked_price(LP_AUSD_DOT), lp_price_1);

		// issue more LP
		assert_ok!(Tokens::deposit(LP_AUSD_DOT, &1, 100));
		assert_eq!(Tokens::total_issuance(LP_AUSD_DOT), 200);

		let lp_price_2 = lp_token_fair_price(
			Tokens::total_issuance(LP_AUSD_DOT),
			MockDEX::get_liquidity_pool(AUSD, DOT).0,
			MockDEX::get_liquidity_pool(AUSD, DOT).1,
			PricesModule::get_price(AUSD, false).unwrap(),
			PricesModule::get_price(DOT, false).unwrap(),
		);

		// get the real-time LP price, calculated by new total_issuance
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, false), lp_price_2);

		// get the locked LP price
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, true), lp_price_1);

		// mock oracle update
		mock_oracle_update();

		let lp_price_3 = lp_token_fair_price(
			Tokens::total_issuance(LP_AUSD_DOT),
			MockDEX::get_liquidity_pool(AUSD, DOT).0,
			MockDEX::get_liquidity_pool(AUSD, DOT).1,
			PricesModule::get_price(AUSD, false).unwrap(),
			PricesModule::get_price(DOT, false).unwrap(),
		);

		// get the real-time LP price, calculated by new total_issuance
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, false), lp_price_3);

		// get the locked LP price
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT, true), lp_price_1);
	});
}

#[test]
fn get_price_of_other_token_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(ACA, false),
			Some(Price::saturating_from_integer(0))
		);
		assert_eq!(PricesModule::get_price(KSM, false), None);

		// lock ACA price worked
		assert_ok!(PricesModule::lock_price(Origin::signed(1), ACA));

		// lock KSM price didn't work
		assert_ok!(PricesModule::lock_price(Origin::signed(1), ACA));

		// mock oracle update
		mock_oracle_update();

		// get real-time price
		assert_eq!(
			PricesModule::get_price(ACA, false),
			Some(Price::saturating_from_integer(30000000u128))
		); // 30 USD, right shift the decimal point (18-12) places
		assert_eq!(
			PricesModule::get_price(KSM, false),
			Some(Price::saturating_from_integer(200000000u128))
		); // 200 USD, right shift the decimal point (18-12) places

		// get locked price
		assert_eq!(
			PricesModule::get_price(ACA, true),
			Some(Price::saturating_from_integer(0))
		);

		// these's no locked price, will get real-time price
		assert_eq!(
			PricesModule::get_price(KSM, true),
			Some(Price::saturating_from_integer(200000000u128))
		);
	});
}

#[test]
fn get_relative_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_relative_price(DOT, true, AUSD, true),
			Some(Price::saturating_from_rational(10000, 1)) /* 1DOT = 100AUSD, right shift the decimal point (12-10)
			                                                 * places */
		);
		assert_eq!(
			PricesModule::get_relative_price(BTC, true, AUSD, true),
			Some(Price::saturating_from_rational(500000000, 1)) /* 1BTC = 50000AUSD, right shift the decimal point
			                                                     * (12-8) places */
		);
		assert_eq!(
			PricesModule::get_relative_price(LDOT, true, DOT, true),
			Some(Price::saturating_from_rational(1, 2)) // 1LDOT = 1/2DOT, right shift the decimal point (10-10) places
		);
		assert_eq!(
			PricesModule::get_relative_price(AUSD, true, AUSD, true),
			Some(Price::saturating_from_rational(1, 1)) // 1AUSD = 1AUSD, right shift the decimal point (10-10) places
		);
		assert_eq!(PricesModule::get_relative_price(AUSD, true, ACA, true), None);

		// lock price
		assert_ok!(PricesModule::lock_price(Origin::signed(1), DOT));
		assert_ok!(PricesModule::lock_price(Origin::signed(1), BTC));

		// mock oracle update
		mock_oracle_update();

		// get the relative price between locked DOT price and locked BTC
		assert_eq!(
			PricesModule::get_relative_price(DOT, true, BTC, true),
			Some(Price::saturating_from_rational(10000, 500000000)) /* 1DOT = 0.002BTC, right shift the decimal
			                                                         * point (12-8)
			                                                         * places */
		);

		// get the relative price between locked DOT price and real-time BTC price
		assert_eq!(
			PricesModule::get_relative_price(DOT, true, BTC, false),
			Some(Price::saturating_from_rational(10000, 400000000)) /* 1DOT = 0.0025BTC, right shift the decimal
			                                                         * point (12-8)
			                                                         * places */
		);

		// get the relative price between real-time DOT price and locked BTC
		assert_eq!(
			PricesModule::get_relative_price(DOT, false, BTC, true),
			Some(Price::saturating_from_rational(1000, 500000000)) /* 1DOT = 0.0002BTC, right shift the decimal
			                                                        * point (12-8)
			                                                        * places */
		);
	});
}

#[test]
fn lock_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_noop!(PricesModule::unlock_price(Origin::signed(5), BTC), BadOrigin);

		// lock the price of BTC
		assert_eq!(
			PricesModule::get_price(BTC, false),
			Some(Price::saturating_from_integer(500000000000000u128))
		);
		assert_eq!(PricesModule::locked_price(BTC), None);
		assert_ok!(PricesModule::lock_price(Origin::signed(1), BTC));
		System::assert_last_event(Event::PricesModule(crate::Event::LockPrice(
			BTC,
			Price::saturating_from_integer(500000000000000u128),
		)));
		assert_eq!(
			PricesModule::locked_price(BTC),
			Some(Price::saturating_from_integer(500000000000000u128))
		);

		// cannot lock the price of KSM when the price from oracle is None
		assert_eq!(PricesModule::get_price(KSM, false), None);
		assert_eq!(PricesModule::locked_price(KSM), None);
		assert_ok!(PricesModule::lock_price(Origin::signed(1), KSM));
		assert_eq!(PricesModule::locked_price(KSM), None);

		mock_oracle_update();

		// lock the price of KSM when the price of KSM from oracle is some
		assert_eq!(
			PricesModule::get_price(KSM, false),
			Some(Price::saturating_from_integer(200000000u128))
		);
		assert_eq!(PricesModule::locked_price(KSM), None);
		assert_ok!(PricesModule::lock_price(Origin::signed(1), KSM));
		System::assert_last_event(Event::PricesModule(crate::Event::LockPrice(
			KSM,
			Price::saturating_from_integer(200000000u128),
		)));
		assert_eq!(
			PricesModule::locked_price(KSM),
			Some(Price::saturating_from_integer(200000000u128))
		);
	});
}

#[test]
fn unlock_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_noop!(PricesModule::unlock_price(Origin::signed(5), BTC), BadOrigin,);

		// unlock the locked price of BTC
		assert_ok!(PricesModule::lock_price(Origin::signed(1), BTC));
		assert_eq!(
			PricesModule::locked_price(BTC),
			Some(Price::saturating_from_integer(500000000000000u128))
		);
		assert_ok!(PricesModule::unlock_price(Origin::signed(1), BTC));
		System::assert_last_event(Event::PricesModule(crate::Event::UnlockPrice(BTC)));
		assert_eq!(PricesModule::locked_price(BTC), None);

		// try unlocking the unlocked price nothing will happen
		assert_eq!(PricesModule::locked_price(KSM), None);
		assert_ok!(PricesModule::unlock_price(Origin::signed(1), KSM));
		// there's no new event triggered
		System::assert_last_event(Event::PricesModule(crate::Event::UnlockPrice(BTC)));
	});
}
