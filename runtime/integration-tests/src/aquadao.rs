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

use ecosystem_aqua_adao_manager::{Allocation, Strategy, StrategyKind};
use ecosystem_aqua_dao::{Discount, DiscountRate, Subscriptions};
use frame_support::traits::OnInitialize;
use mandala_runtime::{AquaAdaoManager, AquaStakedToken, DAYS};
use sp_runtime::{traits::One, FixedI128, FixedU128};

const ADAO_CURRENCY: CurrencyId = CurrencyId::Token(TokenSymbol::ADAO);
const SDAO_CURRENCY: CurrencyId = CurrencyId::Token(TokenSymbol::SDAO);
const AUSD_CURRENCY: CurrencyId = CurrencyId::Token(TokenSymbol::AUSD);
const ADAO_AUSD_LP: CurrencyId =
	CurrencyId::DexShare(DexShare::Token(TokenSymbol::AUSD), DexShare::Token(TokenSymbol::ADAO));

#[test]
fn subscription() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), USD_CURRENCY, 2_000_000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), ADAO_CURRENCY, 1_000_000 * dollar(ADAO_CURRENCY)),
			(AccountId::from(BOB), SDAO_CURRENCY, 1_000_000 * dollar(SDAO_CURRENCY)),
			(
				AquaStakedToken::account_id(),
				ADAO_CURRENCY,
				1_000_000 * dollar(ADAO_CURRENCY),
			),
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
			assert_ok!(DexOracle::enable_average_price(
				Origin::root(),
				ADAO_CURRENCY,
				USD_CURRENCY,
				1
			));
			DexOracle::on_initialize(1);

			// create subscription
			let units = 1_000_000;
			let amount = dollar(CurrencyId::Token(TokenSymbol::ADAO)) * units;
			let discount = Discount {
				max: DiscountRate::saturating_from_rational(2, 10),
				interval: 1,
				inc_on_idle: DiscountRate::saturating_from_rational(1, 1_000),
				dec_per_unit: DiscountRate::saturating_from_rational(20, units * 100),
			};
			assert_ok!(AquaDao::create_subscription(
				Origin::root(),
				USD_CURRENCY,
				1_000,
				dollar(ADAO_CURRENCY) * 10,
				Ratio::saturating_from_rational(1, 10),
				amount,
				discount
			));
			Subscriptions::<Runtime>::mutate(0, |maybe_subscription| {
				if let Some(subscription) = maybe_subscription {
					subscription.state.last_discount = FixedI128::saturating_from_rational(95, 100);
				}
			});

			// subscribe
			let alice = AccountId::from(ALICE);
			assert_ok!(AquaDao::subscribe(
				Origin::signed(alice.clone()),
				0,
				dollar(USD_CURRENCY) * 1_000,
				0
			));
			let subscription_amount = 124_998_000_000_000;
			System::assert_has_event(Event::AquaDao(ecosystem_aqua_dao::Event::Subscribed {
				who: alice.clone(),
				subscription_id: 0,
				payment_amount: dollar(USD_CURRENCY) * 1_000,
				subscription_amount,
			}));
			// default exchange rate: 1
			assert_eq!(Currencies::free_balance(SDAO, &alice), subscription_amount);

			// not claimable vesting yet
			assert_ok!(AquaStakedToken::claim(Origin::signed(alice.clone())));
			assert_noop!(
				Currencies::transfer(
					Origin::signed(alice.clone()),
					AccountId::from(BOB).into(),
					SDAO_CURRENCY,
					1
				),
				orml_tokens::Error::<Runtime>::LiquidityRestrictions
			);

			// inflation
			AquaStakedToken::on_initialize(DAYS);

			// claim && unstake
			set_relaychain_block_number(1001);
			assert_ok!(AquaStakedToken::claim(Origin::signed(alice.clone())));
			assert_ok!(AquaStakedToken::unstake(
				Origin::signed(alice.clone()),
				subscription_amount
			));
			assert_eq!(Currencies::free_balance(ADAO, &alice), 125_203_375_719_934);
		});
}

#[test]
fn inflation() {
	ExtBuilder::default()
		.balances(vec![
			(
				AquaStakedToken::account_id(),
				ADAO_CURRENCY,
				1_000 * dollar(ADAO_CURRENCY),
			),
			(AccountId::from(ALICE), SDAO, 1_000 * dollar(SDAO)),
		])
		.build()
		.execute_with(|| {
			// no inflation yet
			AquaStakedToken::on_initialize(1);
			assert_eq!(
				Currencies::free_balance(ADAO, &AquaStakedToken::account_id()),
				1_000 * dollar(ADAO_CURRENCY)
			);

			// inflation
			AquaStakedToken::on_initialize(DAYS);
			assert_eq!(
				Currencies::free_balance(ADAO, &AquaStakedToken::account_id()),
				1_001_027_397_260_273
			);
			assert_eq!(Currencies::free_balance(SDAO, &TreasuryAccount::get()), 102_739_726_027);
			assert_eq!(Currencies::free_balance(SDAO, &DaoAccount::get()), 102_739_726_027);
		});
}

#[test]
fn adao_manager_rebalance() {
	ExtBuilder::default()
		.balances(vec![
			(DaoAccount::get(), AUSD_CURRENCY, dollar(AUSD_CURRENCY) * 1_000_000),
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
				1_000 * dollar(USD_CURRENCY),
				0,
				false,
			));
			assert_ok!(DexOracle::enable_average_price(
				Origin::root(),
				ADAO_CURRENCY,
				USD_CURRENCY,
				1
			));
			DexOracle::on_initialize(1);

			// set_oracle_price(vec![(AUSD_CURRENCY, One::one()), (ADAO_CURRENCY, One::one()), (ADAO_AUSD_LP,
			// One::one())]);
			set_oracle_price(vec![(ADAO_CURRENCY, One::one())]);

			let allocation = Allocation {
				value: dollar(AUSD_CURRENCY) * 100,
				range: dollar(AUSD_CURRENCY) * 10,
			};
			assert_ok!(AquaAdaoManager::set_target_allocations(
				Origin::root(),
				vec![(AUSD_CURRENCY, Some(allocation)), (ADAO_AUSD_LP, Some(allocation))]
			));

			let strategy = Strategy {
				kind: StrategyKind::LiquidityProvisionAusdAdao,
				percent_per_trade: FixedU128::saturating_from_rational(1, 2),
				max_amount_per_trade: 1_000_000_000_000_000_000,
				min_amount_per_trade: -1_000_000_000_000,
			};
			assert_ok!(AquaAdaoManager::set_strategies(Origin::root(), vec![strategy]));

			AquaAdaoManager::on_initialize(11);
		});
}
