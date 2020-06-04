//! Unit tests for the prices module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{ExtBuilder, Origin, PricesModule, System, TestEvent, ACA, AUSD, BTC, DOT, LDOT};
use orml_utilities::fixed_u128::FixedUnsignedNumber;
use sp_runtime::traits::BadOrigin;

#[test]
fn get_price_from_oracle() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(BTC), Some(Price::from_natural(5000)));
		assert_eq!(PricesModule::get_price(DOT), Some(Price::from_natural(100)));
		assert_eq!(PricesModule::get_price(ACA), Some(Price::from_natural(0)));
	});
}

#[test]
fn get_price_of_stable_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(AUSD), Some(Price::from_natural(1)));
	});
}

#[test]
fn get_price_of_liquid_currency_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(LDOT), Some(Price::from_natural(50)));
	});
}

#[test]
fn get_relative_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			PricesModule::get_relative_price(DOT, AUSD),
			Some(Price::saturating_from_rational(100, 1))
		);
		assert_eq!(
			PricesModule::get_relative_price(BTC, AUSD),
			Some(Price::saturating_from_rational(5000, 1))
		);
		assert_eq!(
			PricesModule::get_relative_price(LDOT, DOT),
			Some(Price::saturating_from_rational(1, 2))
		);
		assert_eq!(
			PricesModule::get_relative_price(AUSD, AUSD),
			Some(Price::saturating_from_rational(1, 1))
		);
		assert_eq!(PricesModule::get_relative_price(AUSD, ACA), None);
	});
}

#[test]
fn lock_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(BTC), Some(Price::from_natural(5000)));
		LockedPrice::insert(BTC, Price::from_natural(8000));
		assert_eq!(PricesModule::get_price(BTC), Some(Price::from_natural(8000)));
	});
}

#[test]
fn lock_price_call_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(PricesModule::lock_price(Origin::signed(5), BTC), BadOrigin,);
		assert_ok!(PricesModule::lock_price(Origin::ROOT, BTC));

		let lock_price_event = TestEvent::prices(Event::LockPrice(BTC, Price::from_natural(5000)));
		assert!(System::events().iter().any(|record| record.event == lock_price_event));

		assert_eq!(PricesModule::locked_price(BTC), Some(Price::from_natural(5000)));
	});
}

#[test]
fn unlock_price_call_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		LockedPrice::insert(BTC, Price::from_natural(8000));
		assert_noop!(PricesModule::unlock_price(Origin::signed(5), BTC), BadOrigin,);
		assert_ok!(PricesModule::unlock_price(Origin::signed(1), BTC));

		let unlock_price_event = TestEvent::prices(Event::UnlockPrice(BTC));
		assert!(System::events().iter().any(|record| record.event == unlock_price_event));

		assert_eq!(PricesModule::locked_price(BTC), None);
	});
}
