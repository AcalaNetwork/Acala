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

use crate::setup::*;
use module_prices::RealTimePriceProvider;
use module_support::PriceProvider;

#[cfg(any(feature = "with-karura-runtime"))]
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

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				Some(relaychain_price)
			);

			let default_ratio = DefaultExchangeRate::get();
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

#[cfg(any(feature = "with-karura-runtime"))]
#[test]
fn test_update_liquid_currency_price() {
	ExtBuilder::default()
		.balances(vec![(alice(), LIQUID_CURRENCY, 1000 * dollar(LIQUID_CURRENCY))])
		.build()
		.execute_with(|| {
			let relaychain_price = Price::saturating_from_rational(10, 1);

			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, relaychain_price)]);

			assert_ok!(HomaLite::set_total_staking_currency(
				Origin::root(),
				100 * dollar(RELAY_CHAIN_CURRENCY)
			));

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(100, 1000))
			);

			assert_ok!(HomaLite::set_total_staking_currency(
				Origin::root(),
				110 * dollar(RELAY_CHAIN_CURRENCY)
			));

			assert_eq!(
				RealTimePriceProvider::<Runtime>::get_relative_price(LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY),
				Some(Ratio::saturating_from_rational(110, 1000))
			);
		});
}
