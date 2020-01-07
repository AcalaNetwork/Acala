//! Unit tests for the prices module.

#![cfg(test)]

use super::*;
use mock::{ExtBuilder, PricesModule, ACA, AUSD, BTC, DOT, ETH, OTHER};

#[test]
fn get_price_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(AUSD, DOT), Some(Price::from_rational(100, 1)));
		assert_eq!(PricesModule::get_price(AUSD, BTC), Some(Price::from_rational(5000, 1)));
		assert_eq!(PricesModule::get_price(AUSD, ACA), Some(Price::from_rational(10, 1)));
		assert_eq!(PricesModule::get_price(AUSD, AUSD), Some(Price::from_rational(1, 1)));
	});
}

#[test]
fn price_is_none_when_no_source() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(AUSD, 5), None);
		assert_eq!(PricesModule::get_price(5, AUSD), None);
		assert_eq!(PricesModule::get_price(5, 6), None);
	});
}

#[test]
fn price_is_zero_when_zero_source() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::get_price(AUSD, OTHER), Some(Price::from_parts(0)));
	});
}

#[test]
fn lock_price_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(PricesModule::locked_price(AUSD), None);
		PricesModule::lock_price(AUSD);
		assert_eq!(
			PricesModule::locked_price(AUSD),
			Some(Some(Price::from_rational(101, 100)))
		);
		assert_eq!(PricesModule::get_price(AUSD, AUSD), Some(Price::from_rational(1, 1)));
	});
}

#[test]
fn unlock_price_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		PricesModule::lock_price(AUSD);
		assert_eq!(
			PricesModule::locked_price(AUSD),
			Some(Some(Price::from_rational(101, 100)))
		);
		PricesModule::unlock_price(AUSD);
		assert_eq!(PricesModule::locked_price(AUSD), None);
		assert_eq!(PricesModule::locked_price(ETH), None);
		PricesModule::lock_price(ETH);
		assert_eq!(PricesModule::locked_price(ETH), Some(None));
		PricesModule::unlock_price(ETH);
		assert_eq!(PricesModule::locked_price(ETH), None);
	});
}
