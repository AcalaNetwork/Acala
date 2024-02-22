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
use sp_runtime::MultiAddress;

type SystemError = frame_system::Error<Runtime>;

#[test]
fn proxy_behavior_correct() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), NATIVE_CURRENCY, 100 * dollar(NATIVE_CURRENCY)),
			(AccountId::from(BOB), NATIVE_CURRENCY, 100 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// proxy fails for account with no NATIVE_CURRENCY
			assert_noop!(
				Proxy::add_proxy(
					RuntimeOrigin::signed(AccountId::from([21; 32])),
					MultiAddress::Id(AccountId::from(ALICE)),
					ProxyType::Any,
					0
				),
				pallet_balances::Error::<Runtime, _>::InsufficientBalance
			);
			let call = Box::new(RuntimeCall::Currencies(module_currencies::Call::transfer {
				dest: AccountId::from(ALICE).into(),
				currency_id: NATIVE_CURRENCY,
				amount: 10 * dollar(NATIVE_CURRENCY),
			}));

			// Alice has all Bob's permissions now
			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				ProxyType::Any,
				0
			));
			// takes deposit from bobs account for proxy
			assert!(Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(BOB)) < 100 * dollar(NATIVE_CURRENCY));

			// alice can now make calls for bob's account
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				None,
				call.clone()
			));
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(ALICE)),
				110 * dollar(NATIVE_CURRENCY)
			);

			// alice cannot make calls for bob's account anymore
			assert_ok!(Proxy::remove_proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				ProxyType::Any,
				0
			));
			assert_noop!(
				Proxy::proxy(
					RuntimeOrigin::signed(AccountId::from(ALICE)),
					MultiAddress::Id(AccountId::from(BOB)),
					None,
					call.clone()
				),
				pallet_proxy::Error::<Runtime>::NotProxy
			);
			// bob's deposit is returned
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(BOB)),
				90000000000000
			);
		});
}

#[test]
fn proxy_permissions_correct() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), NATIVE_CURRENCY, 100 * dollar(NATIVE_CURRENCY)),
			(AccountId::from(BOB), NATIVE_CURRENCY, 100 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 100 * dollar(USD_CURRENCY)),
			(AccountId::from(ALICE), USD_CURRENCY, 100 * dollar(USD_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// runtimes have different minimum debit dust requirements
			let min_debit: Balance = 100 * MinimumDebitValue::get();
			set_oracle_price(vec![(RELAY_CHAIN_CURRENCY, Price::saturating_from_rational(100, 1))]);
			assert_ok!(CdpEngine::set_collateral_params(
				RuntimeOrigin::root(),
				RELAY_CHAIN_CURRENCY,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 10000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(Some(Rate::saturating_from_rational(20, 100))),
				Change::NewValue(Some(Ratio::saturating_from_rational(200, 100))),
				Change::NewValue(1_000_000 * dollar(USD_CURRENCY)),
			));
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				5 * dollar(RELAY_CHAIN_CURRENCY),
				10 * dollar(USD_CURRENCY),
				0,
				false,
			));
			// Alice has all Bob's permissions now
			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				ProxyType::Any,
				0
			));
			let root_call = Box::new(RuntimeCall::Currencies(module_currencies::Call::update_balance {
				who: AccountId::from(ALICE).into(),
				currency_id: NATIVE_CURRENCY,
				amount: 1000 * dollar(NATIVE_CURRENCY) as i128,
			}));
			let gov_call = Box::new(RuntimeCall::Tips(pallet_tips::Call::report_awesome {
				reason: b"bob is awesome".to_vec(),
				who: MultiAddress::Id(AccountId::from(BOB)),
			}));
			let transfer_call = Box::new(RuntimeCall::Currencies(module_currencies::Call::transfer {
				dest: AccountId::from(BOB).into(),
				currency_id: NATIVE_CURRENCY,
				amount: 10 * dollar(NATIVE_CURRENCY),
			}));
			let adjust_loan_call = Box::new(RuntimeCall::Honzon(module_honzon::Call::adjust_loan {
				currency_id: RELAY_CHAIN_CURRENCY,
				collateral_adjustment: 10 * dollar(RELAY_CHAIN_CURRENCY) as i128,
				debit_adjustment: min_debit as i128,
			}));
			let authorize_loan_call = Box::new(RuntimeCall::Honzon(module_honzon::Call::authorize {
				currency_id: RELAY_CHAIN_CURRENCY,
				to: AccountId::from(BOB).into(),
			}));
			let dex_swap_call = Box::new(RuntimeCall::Dex(module_dex::Call::swap_with_exact_target {
				path: vec![RELAY_CHAIN_CURRENCY, USD_CURRENCY],
				target_amount: dollar(USD_CURRENCY),
				max_supply_amount: dollar(RELAY_CHAIN_CURRENCY),
			}));
			let dex_add_liquidity_call = Box::new(RuntimeCall::Dex(module_dex::Call::add_liquidity {
				currency_id_a: RELAY_CHAIN_CURRENCY,
				currency_id_b: USD_CURRENCY,
				max_amount_a: 10 * dollar(RELAY_CHAIN_CURRENCY),
				max_amount_b: 10 * dollar(USD_CURRENCY),
				min_share_increment: 0,
				stake_increment_share: false,
			}));

			// Proxy calls do not bypass root permision
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				None,
				root_call.clone()
			));
			// while the proxy call executes the call being proxied fails
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(ALICE)),
				100 * dollar(NATIVE_CURRENCY)
			);

			// Alice's gives governance permissions to Bob
			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				ProxyType::Governance,
				0
			));
			// Bob can be a proxy for alice gov call
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Governance),
				gov_call.clone()
			));
			let hash = BlakeTwo256::hash_of(&(BlakeTwo256::hash(b"bob is awesome"), AccountId::from(BOB)));
			// last event was sucessful tip call
			assert_eq!(
				System::events()
					.into_iter()
					.map(|r| r.event)
					.filter_map(|e| if let RuntimeEvent::Tips(inner) = e {
						Some(inner)
					} else {
						None
					})
					.last()
					.unwrap(),
				pallet_tips::Event::<Runtime>::NewTip { tip_hash: hash }
			);

			// Bob can't proxy for alice in a non gov call, once again proxy call works but nested call fails
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Governance),
				transfer_call.clone()
			));
			// the transfer call fails as Bob only had governence permission for alice
			assert!(Currencies::free_balance(NATIVE_CURRENCY, &AccountId::from(BOB)) < 100 * dollar(NATIVE_CURRENCY));

			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				ProxyType::Loan,
				0
			));
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Loan),
				adjust_loan_call.clone()
			));
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				10 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				min_debit
			);
			// authorize call is part of the Honzon module but is not in the Loan ProxyType filter
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Loan),
				authorize_loan_call.clone()
			));
			// hence the failure
			System::assert_last_event(
				pallet_proxy::Event::ProxyExecuted {
					result: Err(SystemError::CallFiltered.into()),
				}
				.into(),
			);

			// gives Bob ability to proxy alice's account for dex swaps
			assert_ok!(Proxy::add_proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				ProxyType::Swap,
				0
			));

			let pre_swap = Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE));
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Swap),
				dex_swap_call.clone()
			));
			let post_swap = Currencies::free_balance(USD_CURRENCY, &AccountId::from(ALICE));
			assert_eq!(post_swap - pre_swap, dollar(USD_CURRENCY));

			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Swap),
				dex_add_liquidity_call.clone()
			));
			// again add liquidity call is part of the Dex module but is not allowed in the Swap ProxyType
			// filter
			System::assert_last_event(
				pallet_proxy::Event::ProxyExecuted {
					result: Err(SystemError::CallFiltered.into()),
				}
				.into(),
			);

			// Tests that adding more ProxyType permssions does not effect others
			assert_ok!(Proxy::proxy(
				RuntimeOrigin::signed(AccountId::from(BOB)),
				MultiAddress::Id(AccountId::from(ALICE)),
				Some(ProxyType::Loan),
				adjust_loan_call.clone()
			));
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).collateral,
				20 * dollar(RELAY_CHAIN_CURRENCY)
			);
			assert_eq!(
				Loans::positions(RELAY_CHAIN_CURRENCY, AccountId::from(ALICE)).debit,
				2 * min_debit
			);

			// remove proxy works
			assert_ok!(Proxy::remove_proxy(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				MultiAddress::Id(AccountId::from(BOB)),
				ProxyType::Loan,
				0
			));
			assert_noop!(
				Proxy::proxy(
					RuntimeOrigin::signed(AccountId::from(BOB)),
					MultiAddress::Id(AccountId::from(ALICE)),
					Some(ProxyType::Loan),
					adjust_loan_call.clone()
				),
				pallet_proxy::Error::<Runtime>::NotProxy
			);
		});
}
