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

use crate::setup::*;
use module_prices::RealTimePriceProvider;
use module_support::PriceProvider;

#[test]
fn test_default_liquid_currency_price() {
	ExtBuilder::default()
		.balances(vec![(
			alice(),
			RELAY_CHAIN_CURRENCY,
			100 * dollar(RELAY_CHAIN_CURRENCY),
		)])
		.build()
		.execute_with(|| {
			assert_eq!(RealTimePriceProvider::<Runtime>::get_price(RELAY_CHAIN_CURRENCY), None);
			assert_eq!(RealTimePriceProvider::<Runtime>::get_price(LIQUID_CURRENCY), None);

			let relaychain_price = Price::saturating_from_rational(10, 1);

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, relaychain_price)]);

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			{
				assert_eq!(
					RealTimePriceProvider::<Runtime>::get_price(RELAY_CHAIN_CURRENCY),
					Some(Price::saturating_from_integer(1_000_000_000u128))
				);
				assert_eq!(
					RealTimePriceProvider::<Runtime>::get_price(LIQUID_CURRENCY),
					Some(Price::saturating_from_integer(100_000_000u128))
				);
			}
			#[cfg(feature = "with-karura-runtime")]
			{
				assert_eq!(
					RealTimePriceProvider::<Runtime>::get_price(RELAY_CHAIN_CURRENCY),
					Some(Price::saturating_from_integer(10_000_000u128))
				);
				assert_eq!(
					RealTimePriceProvider::<Runtime>::get_price(LIQUID_CURRENCY),
					Some(Price::saturating_from_integer(1_000_000u128))
				);
			}

			let default_ratio = DefaultExchangeRate::get();

			#[cfg(any(feature = "with-mandala-runtime", feature = "with-acala-runtime"))]
			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, USD_CURRENCY),
				Some(relaychain_price * default_ratio * 100.into())
			);
			#[cfg(feature = "with-karura-runtime")]
			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, USD_CURRENCY),
				Some(relaychain_price * default_ratio)
			);

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY),
				Some(default_ratio)
			);
		});
}

#[test]
fn test_update_liquid_currency_price() {
	ExtBuilder::default()
		.balances(vec![(alice(), LIQUID_CURRENCY, 1000 * dollar(LIQUID_CURRENCY))])
		.build()
		.execute_with(|| {
			let relaychain_price = Price::saturating_from_rational(10, 1);

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, relaychain_price)]);

			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::root(),
				vec![(0, Some(100 * dollar(RELAY_CHAIN_CURRENCY)), None)]
			));

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(100, 1000))
			);

			assert_ok!(Homa::reset_ledgers(
				RuntimeOrigin::root(),
				vec![(0, Some(110 * dollar(RELAY_CHAIN_CURRENCY)), None)]
			));

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(110, 1000))
			);
		});
}
