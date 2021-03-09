//! Unit tests for the prices module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn get_price_from_oracle() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(BTC),
			Some(Price::saturating_from_integer(500000000000000u128))
		); // 50000 USD, right shift the decimal point (18-10) places
		assert_eq!(
			PricesModule::get_price(DOT),
			Some(Price::saturating_from_integer(10000000000u128))
		); // 100 USD, right shift the decimal point (18-12) places
		assert_eq!(PricesModule::get_price(ACA), Some(Price::zero()));
	});
}

#[test]
fn get_price_of_stable_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(AUSD),
			Some(Price::saturating_from_integer(1000000))
		); // 1 USD, right shift the decimal point (18-12) places
	});
}

#[test]
fn get_price_of_liquid_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_price(LDOT),
			Some(Price::saturating_from_integer(5000000000u128))
		); // 50 USD, right shift the decimal point (18-12) places
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

		let lock_price_event = Event::prices(crate::Event::LockPrice(BTC, Price::saturating_from_integer(50000)));
		assert!(System::events().iter().any(|record| record.event == lock_price_event));
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

		let unlock_price_event = Event::prices(crate::Event::UnlockPrice(BTC));
		assert!(System::events().iter().any(|record| record.event == unlock_price_event));

		assert_eq!(PricesModule::locked_price(BTC), None);
	});
}
