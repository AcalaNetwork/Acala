// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
fn enable_average_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		Timestamp::set_timestamp(1000);
		assert_noop!(
			DexOracle::enable_average_price(RuntimeOrigin::signed(0), AUSD, DOT, 0),
			BadOrigin
		);
		assert_noop!(
			DexOracle::enable_average_price(RuntimeOrigin::signed(1), AUSD, LP_AUSD_DOT, 0),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			DexOracle::enable_average_price(RuntimeOrigin::signed(1), AUSD, DOT, 0),
			Error::<Runtime>::IntervalIsZero
		);
		assert_noop!(
			DexOracle::enable_average_price(RuntimeOrigin::signed(1), AUSD, DOT, 12000),
			Error::<Runtime>::InvalidPool
		);

		set_pool(&AUSDDOTPair::get(), 1_000, 100);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(DexOracle::average_prices(AUSDDOTPair::get()), None);

		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			12000
		));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 1000)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(0),
				U256::from(0),
				1000,
				12000,
			))
		);

		assert_noop!(
			DexOracle::enable_average_price(RuntimeOrigin::signed(1), AUSD, DOT, 12000),
			Error::<Runtime>::AveragePriceAlreadyEnabled
		);
	});
}

#[test]
fn disable_average_price_work() {
	ExtBuilder::default().build().execute_with(|| {
		set_pool(&AUSDDOTPair::get(), 1_000, 100);
		Timestamp::set_timestamp(100);
		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			1000
		));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 100)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(0),
				U256::from(0),
				100,
				1000,
			))
		);

		assert_noop!(
			DexOracle::disable_average_price(RuntimeOrigin::signed(0), AUSD, DOT),
			BadOrigin
		);
		assert_noop!(
			DexOracle::disable_average_price(RuntimeOrigin::signed(1), AUSD, LP_AUSD_DOT),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			DexOracle::disable_average_price(RuntimeOrigin::signed(1), ACA, DOT),
			Error::<Runtime>::AveragePriceMustBeEnabled
		);

		assert_ok!(DexOracle::disable_average_price(RuntimeOrigin::signed(1), AUSD, DOT));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(DexOracle::average_prices(AUSDDOTPair::get()), None);
	});
}

#[test]
fn update_average_price_interval_work() {
	ExtBuilder::default().build().execute_with(|| {
		set_pool(&AUSDDOTPair::get(), 1_000, 100);
		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			1000
		));
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(0),
				U256::from(0),
				0,
				1000,
			))
		);

		assert_noop!(
			DexOracle::update_average_price_interval(RuntimeOrigin::signed(0), AUSD, DOT, 0),
			BadOrigin
		);
		assert_noop!(
			DexOracle::update_average_price_interval(RuntimeOrigin::signed(1), AUSD, LP_AUSD_DOT, 0),
			Error::<Runtime>::InvalidCurrencyId
		);
		assert_noop!(
			DexOracle::update_average_price_interval(RuntimeOrigin::signed(1), ACA, DOT, 0),
			Error::<Runtime>::AveragePriceMustBeEnabled
		);
		assert_noop!(
			DexOracle::update_average_price_interval(RuntimeOrigin::signed(1), AUSD, DOT, 0),
			Error::<Runtime>::IntervalIsZero
		);

		assert_ok!(DexOracle::update_average_price_interval(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			2000
		));
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 1000),
				ExchangeRate::saturating_from_rational(1000, 100),
				U256::from(0),
				U256::from(0),
				0,
				2000,
			))
		);
	});
}

#[test]
fn try_update_cumulative_work() {
	ExtBuilder::default().build().execute_with(|| {
		// initialize cumulative price
		set_pool(&AUSDDOTPair::get(), 1_000, 100);
		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			1000
		));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);

		// will not cumulative if now is not gt than the last update cumulative timestamp.
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
		// initialize average prices
		assert_eq!(Timestamp::now(), 0);
		set_pool(&AUSDDOTPair::get(), 1000, 100);
		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			AUSD,
			DOT,
			1000
		));
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(0),
				U256::from(0),
				0,
				1000
			))
		);
		set_pool(&ACADOTPair::get(), 1000, 1000);
		assert_ok!(DexOracle::enable_average_price(
			RuntimeOrigin::signed(1),
			ACA,
			DOT,
			2000
		));
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 1),
				ExchangeRate::saturating_from_rational(1, 1),
				U256::from(0),
				U256::from(0),
				0,
				2000
			))
		);

		// elapsed time is lt all update interval of trading pairs, no trading pairs will not update average
		// price.
		Timestamp::set_timestamp(999);
		DexOracle::on_initialize(1);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(0),
				U256::from(0),
				0,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 1),
				ExchangeRate::saturating_from_rational(1, 1),
				U256::from(0),
				U256::from(0),
				0,
				2000,
			))
		);

		// elapsed time is lt the update interval of AUSD/DOT, update average price of AUSD/DOT after try
		// update cumulatives.
		Timestamp::set_timestamp(1200);
		DexOracle::on_initialize(2);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128),
				1200
			)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128),
				1200,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(U256::from(0), U256::from(0), 0)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 1),
				ExchangeRate::saturating_from_rational(1, 1),
				U256::from(0),
				U256::from(0),
				0,
				2000,
			))
		);

		// elapsed time is lt the update interval of ACA/DOT, update average price of ACA/DOT after try
		// update cumulatives.
		set_pool(&ACADOTPair::get(), 1000, 2000);
		Timestamp::set_timestamp(2100);
		DexOracle::on_initialize(3);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128),
				1200
			)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(1, 10),
				ExchangeRate::saturating_from_rational(10, 1),
				U256::from(120_000_000_000_000_000_000u128),
				U256::from(12_000_000_000_000_000_000_000u128),
				1200,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(
				U256::from(4_200_000_000_000_000_000_000u128),
				U256::from(1_050_000_000_000_000_000_000u128),
				2100
			)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(2000, 1000),
				ExchangeRate::saturating_from_rational(1000, 2000),
				U256::from(4_200_000_000_000_000_000_000u128),
				U256::from(1_050_000_000_000_000_000_000u128),
				2100,
				2000,
			))
		);

		set_pool(&AUSDDOTPair::get(), 2000, 100);
		set_pool(&ACADOTPair::get(), 1000, 4000);
		Timestamp::set_timestamp(5000);
		DexOracle::on_initialize(4);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(310_000_000_000_000_000_000u128),
				U256::from(88_000_000_000_000_000_000_000u128),
				5000
			)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 2000),
				ExchangeRate::saturating_from_rational(2000, 100),
				U256::from(310_000_000_000_000_000_000u128),
				U256::from(88_000_000_000_000_000_000_000u128),
				5000,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(
				U256::from(15_800_000_000_000_000_000_000u128),
				U256::from(1_775_000_000_000_000_000_000u128),
				5000
			)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(4000, 1000),
				ExchangeRate::saturating_from_rational(1000, 4000),
				U256::from(15_800_000_000_000_000_000_000u128),
				U256::from(1_775_000_000_000_000_000_000u128),
				5000,
				2000,
			))
		);

		// mock update cumulatives, the average prices are not updated.
		Timestamp::set_timestamp(5500);
		DexOracle::on_initialize(5);
		DexOracle::try_update_cumulative(&AUSDDOTPair::get(), 100, 100);
		DexOracle::try_update_cumulative(&ACADOTPair::get(), 2000, 200);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(810_000_000_000_000_000_000u128),
				U256::from(88_500_000_000_000_000_000_000u128),
				5500
			)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(100, 2000),
				ExchangeRate::saturating_from_rational(2000, 100),
				U256::from(310_000_000_000_000_000_000u128),
				U256::from(88_000_000_000_000_000_000_000u128),
				5000,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(
				U256::from(15_850_000_000_000_000_000_000u128),
				U256::from(6_775_000_000_000_000_000_000u128),
				5500
			)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(4000, 1000),
				ExchangeRate::saturating_from_rational(1000, 4000),
				U256::from(15_800_000_000_000_000_000_000u128),
				U256::from(1_775_000_000_000_000_000_000u128),
				5000,
				2000,
			))
		);

		// update average prices of AUSD/DOT and ACA/DOT
		set_pool(&AUSDDOTPair::get(), 1000, 100);
		set_pool(&ACADOTPair::get(), 1000, 1000);
		Timestamp::set_timestamp(7000);
		DexOracle::on_initialize(6);
		assert_eq!(
			DexOracle::cumulatives(AUSDDOTPair::get()),
			(
				U256::from(960_000_000_000_000_000_000u128),
				U256::from(103_500_000_000_000_000_000_000u128),
				7000
			)
		);
		assert_eq!(
			DexOracle::average_prices(AUSDDOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(325, 1000),
				ExchangeRate::saturating_from_rational(775, 100),
				U256::from(960_000_000_000_000_000_000u128),
				U256::from(103_500_000_000_000_000_000_000u128),
				7000,
				1000,
			))
		);
		assert_eq!(
			DexOracle::cumulatives(ACADOTPair::get()),
			(
				U256::from(17_350_000_000_000_000_000_000u128),
				U256::from(8_275_000_000_000_000_000_000u128),
				7000
			)
		);
		assert_eq!(
			DexOracle::average_prices(ACADOTPair::get()),
			Some((
				ExchangeRate::saturating_from_rational(775, 1000),
				ExchangeRate::saturating_from_rational(325, 100),
				U256::from(17_350_000_000_000_000_000_000u128),
				U256::from(8_275_000_000_000_000_000_000u128),
				7000,
				2000,
			))
		);
	});
}

#[test]
fn dex_price_providers_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT), None);
		assert_eq!(CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD), None);
		assert_eq!(AverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT), None);
		assert_eq!(AverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD), None);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			None
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			None
		);

		set_pool(&AUSDDOTPair::get(), 1_000, 100);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(1, 10))
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 1))
		);
		assert_eq!(AverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT), None);
		assert_eq!(AverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD), None);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(1, 10))
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 1))
		);

		AveragePrices::<Runtime>::insert(
			&AUSDDOTPair::get(),
			(
				ExchangeRate::saturating_from_rational(2, 10),
				ExchangeRate::saturating_from_rational(10, 2),
				U256::from(0),
				U256::from(0),
				0,
				1000,
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
			AverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			AverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);

		set_pool(&AUSDDOTPair::get(), 300, 100);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(100, 300))
		);
		assert_eq!(
			CurrentDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(300, 100))
		);
		assert_eq!(
			AverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			AverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(AUSD, DOT),
			Some(ExchangeRate::saturating_from_rational(2, 10))
		);
		assert_eq!(
			PriorityAverageDEXPriceProvider::<Runtime>::get_relative_price(DOT, AUSD),
			Some(ExchangeRate::saturating_from_rational(10, 2))
		);
	});
}
