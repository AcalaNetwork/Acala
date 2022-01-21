// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! Unit tests for the dex oracle module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;
use sp_runtime::{traits::BadOrigin, FixedPointNumber};

#[test]
fn enable_cumulative_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		Timestamp::set_timestamp(1000);
		assert_noop!(
			DexOracle::enable_cumulative_price(Origin::signed(0), AUSD, DOT),
			BadOrigin
		);
		assert_noop!(
			DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, LP_AUSD_DOT),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT),
			Error::<Runtime>::InvalidPool
		);

		set_pool(1_000, 100);
		assert_eq!(DexOracle::last_price_updated_time(), 0);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(DexOracle::cumulative_prices(AUSDDOTPair::get()), None);

		assert_ok!(DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(100_000_000_000_000_000_000u128),
				U256::from(10_000_000_000_000_000_000_000u128),
				1000
			)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(100_000_000_000_000_000_000u128),
				U256::from(10_000_000_000_000_000_000_000u128),
			))
		);

		assert_noop!(
			DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT),
			Error::<Runtime>::CumulativePriceAlreadyEnabled
		);
	});
}

#[test]
fn disable_cumulative_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		set_pool(1_000, 100);
		Timestamp::set_timestamp(100);
		assert_eq!(DexOracle::last_price_updated_time(), 0);
		assert_ok!(DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(10_000_000_000_000_000_000u128),
				U256::from(1_000_000_000_000_000_000_000u128),
				100
			)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(10_000_000_000_000_000_000u128),
				U256::from(1_000_000_000_000_000_000_000u128),
			))
		);

		assert_noop!(
			DexOracle::disable_cumulative_price(Origin::signed(0), AUSD, DOT),
			BadOrigin
		);
		assert_noop!(
			DexOracle::disable_cumulative_price(Origin::signed(1), AUSD, LP_AUSD_DOT),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			DexOracle::disable_cumulative_price(Origin::signed(1), ACA, DOT),
			Error::<Runtime>::CumulativePriceMustBeEnabled
		);

		assert_ok!(DexOracle::disable_cumulative_price(Origin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(DexOracle::cumulative_prices(AUSDDOTPair::get()), None);
	});
}

#[test]
fn try_update_cumulative_work() {
	ExtBuilder::default().build().execute_with(|| {
		// initialize cumulative price
		set_pool(1_000, 100);
		assert_ok!(DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);

		// will not cumulative if now is not gt than the last cumulative timestamp.
		assert_eq!(Timestamp::now(), 0);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 500, 200);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);

		Timestamp::set_timestamp(100);
		assert_eq!(Timestamp::now(), 100);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 500, 200);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(40_000_000_000_000_000_000u128),
				U256::from(250_000_000_000_000_000_000u128),
				100
			)
		);

		Timestamp::set_timestamp(200);
		assert_eq!(Timestamp::now(), 200);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 1_000, 100);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(50_000_000_000_000_000_000u128),
				U256::from(1_250_000_000_000_000_000_000u128),
				200
			)
		);

		// will not cumulative if TradingPair is not enabled as cumulative price.
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		DexOracle::try_update_cumulative(&ACADOTPair::get(), 500, 200);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
	});
}

#[test]
fn on_initialize_work() {
	ExtBuilder::default().build().execute_with(|| {
		// initialize cumulative price
		set_pool(1_000, 100);
		assert_eq!(Timestamp::now(), 0);
		assert_eq!(DexOracle::last_price_updated_time(), 0);
		assert_ok!(DexOracle::enable_cumulative_price(Origin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(0),
				U256::from(0)
			))
		);

		// interval is lt IntervalToUpdateCumulativePrice, will not update cumulative prices.
		Timestamp::set_timestamp(999);
		DexOracle::on_initialize(1);
		assert_eq!(DexOracle::last_price_updated_time(), 0);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(0),
				U256::from(0)
			))
		);

		// update cumulative prices after try update cumulatives.
		Timestamp::set_timestamp(1200);
		DexOracle::on_initialize(2);
		assert_eq!(DexOracle::last_price_updated_time(), 1200);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128),
				1200
			)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128)
			))
		);

		Timestamp::set_timestamp(1600);
		DexOracle::on_initialize(4);
		assert_eq!(DexOracle::last_price_updated_time(), 1200);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 20_000, 1_000);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(140_000_000_000_000_000_000u128),
				U256::from(20_000_000_000_000_000_000_000u128),
				1600
			)
		);
		Timestamp::set_timestamp(1800);
		DexOracle::on_initialize(5);
		assert_eq!(DexOracle::last_price_updated_time(), 1200);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 40_000, 1_000);
		set_pool(50_000, 1_000);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(145_000_000_000_000_000_000u128),
				U256::from(28_000_000_000_000_000_000_000u128),
				1800
			)
		);

		Timestamp::set_timestamp(2200);
		DexOracle::on_initialize(6);
		assert_eq!(DexOracle::last_price_updated_time(), 2200);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(153_000_000_000_000_000_000u128),
				U256::from(48_000_000_000_000_000_000_000u128),
				2200
			)
		);
		assert_eq!(
			DexOracle::cumulative_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(33, 1_000),
				ExchangeRate::saturating_from_rational(36, 1),
				U256::from(153_000_000_000_000_000_000u128),
				U256::from(48_000_000_000_000_000_000_000u128)
			))
		);
	});
}

#[test]
fn dex_price_providers_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT), None);
		assert_eq!(CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD), None);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			None
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			None
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			None
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			None
		);

		set_pool(1_000, 100);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(1, 10))
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 1))
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			None
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			None
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(1, 10))
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 1))
		);

		CumulativePrices::<Runtime>::insert(
			&AUSDDOTPair::get(),
			(
				ExchangeRate::saturating_from_rational(2, 10),
				ExchangeRate::saturating_from_rational(10, 2),
				U256::from(0),
				U256::from(0),
			),
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(1, 10))
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 1))
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);

		set_pool(300, 100);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(100, 300))
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(300, 100))
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			CumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			PriorityCumulativeDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
	});
}
