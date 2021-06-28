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
	traits::{BadOrigin, Bounded, Zero},
	FixedPointNumber,
};

#[test]
fn integer_sqrt_works() {
	assert_eq!(
		integer_sqrt(U256::from(u128::MAX).saturating_mul(U256::from(u128::MAX))),
		U256::from(u128::MAX)
	);
	assert_eq!(integer_sqrt(U256::from(5)), U256::from(2));
	assert_eq!(integer_sqrt(U256::from(4)), U256::from(2));
	assert_eq!(integer_sqrt(U256::from(3)), U256::from(1));
	assert_eq!(integer_sqrt(U256::from(2)), U256::from(1));
	assert_eq!(integer_sqrt(U256::from(1)), U256::from(1));
	assert_eq!(integer_sqrt(U256::from(0)), U256::from(0));
}

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
fn get_price_from_oracle() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(BTC),
			Some(Price::saturating_from_integer(500000000000000u128))
		); // 50000 USD, right shift the decimal point (18-8) places
		assert_eq!(
			PricesModule::get_price(DOT),
			Some(Price::saturating_from_integer(10000000000u128))
		); // 100 USD, right shift the decimal point (18-10) places
		assert_eq!(PricesModule::get_price(ACA), Some(Price::zero()));
	});
}

#[test]
fn get_price_of_stable_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(AUSD),
			Some(Price::saturating_from_integer(1000000u128))
		); // 1 USD, right shift the decimal point (18-12) places
	});
}

#[test]
fn get_price_of_liquid_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(LDOT),
			Some(Price::saturating_from_integer(5000000000u128))
		); // 50 USD, right shift the decimal point (18-10) places
	});
}

#[test]
fn get_price_of_lp_token_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(MockDEX::get_liquidity_pool(AUSD, DOT), (10000, 200));
		assert_eq!(PricesModule::get_price(LP_AUSD_DOT), None);
		assert_ok!(Tokens::deposit(LP_AUSD_DOT, &1, 100));
		assert_eq!(Tokens::total_issuance(LP_AUSD_DOT), 100);
		assert_eq!(
			PricesModule::get_price(AUSD),
			Some(Price::saturating_from_rational(1000000u128, 1))
		);
		assert_eq!(
			PricesModule::get_price(LP_AUSD_DOT),
			lp_token_fair_price(
				Tokens::total_issuance(LP_AUSD_DOT),
				MockDEX::get_liquidity_pool(AUSD, DOT).0,
				MockDEX::get_liquidity_pool(AUSD, DOT).1,
				PricesModule::get_price(AUSD).unwrap(),
				PricesModule::get_price(DOT).unwrap()
			)
		);

		assert_eq!(MockDEX::get_liquidity_pool(BTC, AUSD), (0, 0));
		assert_eq!(PricesModule::get_price(LP_BTC_AUSD), None);
	});
}

#[test]
fn get_relative_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_relative_price(DOT, AUSD),
			Some(Price::saturating_from_rational(10000, 1)) /* 1DOT = 100AUSD, right shift the decimal point (12-10)
			                                                 * places */
		);
		assert_eq!(
			PricesModule::get_relative_price(BTC, AUSD),
			Some(Price::saturating_from_rational(500000000, 1)) /* 1BTC = 50000AUSD, right shift the decimal point
			                                                     * (12-8) places */
		);
		assert_eq!(
			PricesModule::get_relative_price(LDOT, DOT),
			Some(Price::saturating_from_rational(1, 2)) // 1LDOT = 1/2DOT, right shift the decimal point (10-10) places
		);
		assert_eq!(
			PricesModule::get_relative_price(AUSD, AUSD),
			Some(Price::saturating_from_rational(1, 1)) // 1AUSD = 1AUSD, right shift the decimal point (10-10) places
		);
		assert_eq!(PricesModule::get_relative_price(AUSD, ACA), None);
	});
}

#[test]
fn lock_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(BTC),
			Some(Price::saturating_from_integer(500000000000000u128))
		);
		LockedPrice::<Runtime>::insert(BTC, Price::saturating_from_integer(80000));
		assert_eq!(
			PricesModule::get_price(BTC),
			Some(Price::saturating_from_integer(800000000000000u128))
		);
	});
}

#[test]
fn lock_price_call_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(PricesModule::lock_price(Origin::signed(5), BTC), BadOrigin,);
		assert_ok!(PricesModule::lock_price(Origin::signed(1), BTC));
		System::assert_last_event(Event::PricesModule(crate::Event::LockPrice(
			BTC,
			Price::saturating_from_integer(50000),
		)));
		assert_eq!(
			PricesModule::locked_price(BTC),
			Some(Price::saturating_from_integer(50000))
		);
	});
}

#[test]
fn unlock_price_call_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		LockedPrice::<Runtime>::insert(BTC, Price::saturating_from_integer(80000));
		assert_noop!(PricesModule::unlock_price(Origin::signed(5), BTC), BadOrigin,);
		assert_ok!(PricesModule::unlock_price(Origin::signed(1), BTC));
		System::assert_last_event(Event::PricesModule(crate::Event::UnlockPrice(BTC)));
		assert_eq!(PricesModule::locked_price(BTC), None);
	});
}
