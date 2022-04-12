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

use super::utils::{dollar, set_balance};
use crate::*;

use frame_benchmarking::whitelisted_caller;
use frame_support::traits::OnInitialize;
use frame_system::RawOrigin;

use ecosystem_aqua_dao::{Discount, DiscountRate, Subscription, SubscriptionState};

const STABLECOIN: CurrencyId = GetStableCurrencyId::get();
const ADAO_CURRENCY: CurrencyId = CurrencyId::Token(TokenSymbol::ADAO);

runtime_benchmarks! {
	{ Runtime, ecosystem_aqua_dao }

	create_subscription {
		let min_amount = dollar(STABLECOIN) * 10;
		let min_ratio = Ratio::saturating_from_rational(1, 10);
		let amount = dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 100_000;
		let discount = Discount {
			max: DiscountRate::saturating_from_rational(8, 10),
			interval: 1,
			inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
			dec_per_unit: DiscountRate::saturating_from_rational(1, 1_000),
		};
	}: _(RawOrigin::Root, STABLECOIN, 1_000, min_amount, min_ratio, amount, discount)

	update_subscription {
		// create subscription first
		let min_amount = dollar(STABLECOIN) * 10;
		let min_ratio = Ratio::saturating_from_rational(1, 10);
		let amount = dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 100_000;
		let discount = Discount {
			max: DiscountRate::saturating_from_rational(8, 10),
			interval: 1,
			inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
			dec_per_unit: DiscountRate::saturating_from_rational(1, 1_000),
		};
		AquaDao::create_subscription(RawOrigin::Root.into(), STABLECOIN, 1_000, min_amount, min_ratio, amount, discount)?;
	}: _(
		RawOrigin::Root,
		0,
		Some(2_000),
		Some(dollar(STABLECOIN) * 20),
		Some(Price::one() + Price::one()),
		Some(dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 200_000),
		Some(discount)
	)

	close_subscription {
		// create subscription first
		let min_amount = dollar(STABLECOIN) * 10;
		let min_ratio = Ratio::saturating_from_rational(1, 10);
		let amount = dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 100_000;
		let discount = Discount {
			max: DiscountRate::saturating_from_rational(8, 10),
			interval: 1,
			inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
			dec_per_unit: DiscountRate::saturating_from_rational(1, 1_000),
		};
		AquaDao::create_subscription(RawOrigin::Root.into(), STABLECOIN, 1_000, min_amount, min_ratio, amount, discount)?;
	}: _(RawOrigin::Root, 0)

	subscribe {
		let alice = whitelisted_caller();
		// setup balances
		set_balance(STABLECOIN, &alice, 2_000_000 * dollar(STABLECOIN));
		set_balance(ADAO_CURRENCY, &alice, 1_000_000 * dollar(ADAO_CURRENCY));
		// setup DEX
		Dex::add_liquidity(
			Origin::signed(AccountId::from(alice.clone())),
			ADAO_CURRENCY,
			STABLECOIN,
			1_000 * dollar(ADAO_CURRENCY),
			10_000 * dollar(STABLECOIN),
			0,
			false,
		)?;
		DexOracle::enable_average_price(
			Origin::root(),
			ADAO_CURRENCY,
			STABLECOIN,
			1
		)?;
		DexOracle::on_initialize(1);

		// create subscription
		let min_amount = dollar(STABLECOIN) * 10;
		let min_ratio = Ratio::saturating_from_rational(1, 10);
		let amount = dollar(CurrencyId::Token(TokenSymbol::ADAO)) * 100_000;
		let discount = Discount {
			max: DiscountRate::saturating_from_rational(8, 10),
			interval: 1,
			inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
			dec_per_unit: DiscountRate::saturating_from_rational(1, 1_000),
		};
		AquaDao::create_subscription(RawOrigin::Root.into(), STABLECOIN, 1_000, min_amount, min_ratio, amount, discount)?;
		let payment_amount = dollar(STABLECOIN) * 200;
	}: _(RawOrigin::Signed(alice), 0, payment_amount, 0)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
