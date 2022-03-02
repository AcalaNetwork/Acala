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

use crate::setup::*;

use sp_runtime::traits::One;

use ecosystem_aqua_dao::{Discount, DiscountRate, Subscription, SubscriptionState};

const ADAO_CURRENCY: CurrencyId = CurrencyId::Token(TokenSymbol::ADAO);

#[test]
fn subscription() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 1_000_000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), ADAO_CURRENCY, 1_000_000 * dollar(ADAO_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// setup DEX
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(BOB)),
				ADAO_CURRENCY,
				USD_CURRENCY,
				1_000 * dollar(ADAO_CURRENCY),
				10_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			// create subscription
			let subscription = Subscription {
				currency_id: USD_CURRENCY,
				vesting_period: 1_000,
				min_amount: dollar(ADAO_CURRENCY) * 10,
				min_price: Price::one(),
				amount: dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 100_000,
				discount: Discount {
					max: DiscountRate::saturating_from_rational(8, 10),
					inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
					dec_per_unit: DiscountRate::saturating_from_rational(1, 1_000),
				},
				state: SubscriptionState {
					total_sold: 0,
					last_sold_at: 0,
					last_discount: DiscountRate::saturating_from_rational(8, 10),
				},
			};
			assert_ok!(AquaDao::create_subscription(Origin::root(), subscription));

			let alice = AccountId::from(ALICE);
			assert_ok!(AquaDao::subscribe(
				Origin::signed(alice),
				0,
				dollar(USD_CURRENCY) * 1_000,
				0
			));
		});
}
