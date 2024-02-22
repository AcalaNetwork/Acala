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

//! Unit tests for Honzon Bridge module.

#![cfg(test)]

use crate::mock::*;
use frame_support::{assert_noop, assert_ok};
use module_support::EVMAccountsManager;
use module_support::EVM as EVMTrait;

#[test]
fn set_bridged_stable_coin_address_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(ACA, &alice()), dollar(1_000_000));
		assert_eq!(Currencies::free_balance(KUSD, &alice()), dollar(1_000_000));
		deploy_contracts();
		assert_ok!(HonzonBridge::set_bridged_stable_coin_address(
			RuntimeOrigin::root(),
			erc20_address()
		));

		System::assert_last_event(RuntimeEvent::HonzonBridge(
			crate::Event::BridgedStableCoinCurrencyIdSet {
				bridged_stable_coin_currency_id: CurrencyId::Erc20(erc20_address()),
			},
		));
	});
}

#[test]
fn to_bridged_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(ACA, &alice()), dollar(1_000_000));
		assert_eq!(Currencies::free_balance(KUSD, &alice()), dollar(1_000_000));

		assert_noop!(
			HonzonBridge::from_bridged(RuntimeOrigin::signed(alice()), dollar(5_000)),
			module_honzon_bridge::Error::<Runtime>::BridgedStableCoinCurrencyIdNotSet
		);

		deploy_contracts();
		assert_ok!(HonzonBridge::set_bridged_stable_coin_address(
			RuntimeOrigin::root(),
			erc20_address()
		));
		// ensure the honzon-bridge pallet account bind the evmaddress
		<EVM as EVMTrait<AccountId>>::set_origin(EvmAccountsModule::get_account_id(&alice_evm_addr()));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(alice()),
			HonzonBridgeAccount::get(),
			HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
			dollar(1_000_000)
		));

		assert_eq!(Currencies::free_balance(KUSD, &alice()), dollar(1_000_000));
		assert_eq!(
			Currencies::free_balance(KUSD, &HonzonBridgeAccount::get()),
			dollar(1_000_000)
		);
		assert_eq!(
			Currencies::free_balance(HonzonBridge::bridged_stable_coin_currency_id().unwrap(), &alice()),
			ALICE_BALANCE - dollar(1_000_000)
		);
		assert_eq!(
			Currencies::free_balance(
				HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
				&HonzonBridgeAccount::get()
			),
			dollar(1_000_000)
		);

		assert_ok!(HonzonBridge::to_bridged(RuntimeOrigin::signed(alice()), dollar(5_000)));

		assert_eq!(
			Currencies::free_balance(KUSD, &alice()),
			dollar(1_000_000) - dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(KUSD, &HonzonBridgeAccount::get()),
			dollar(1_000_000) + dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(HonzonBridge::bridged_stable_coin_currency_id().unwrap(), &alice()),
			ALICE_BALANCE - dollar(1_000_000) + dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(
				HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
				&HonzonBridgeAccount::get()
			),
			dollar(1_000_000) - dollar(5_000)
		);

		System::assert_last_event(RuntimeEvent::HonzonBridge(crate::Event::ToBridged {
			who: alice(),
			amount: dollar(5000),
		}));
	});
}

#[test]
fn from_bridged_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(ACA, &alice()), dollar(1_000_000));
		assert_eq!(Currencies::free_balance(KUSD, &alice()), dollar(1_000_000));

		assert_noop!(
			HonzonBridge::from_bridged(RuntimeOrigin::signed(alice()), dollar(5_000)),
			module_honzon_bridge::Error::<Runtime>::BridgedStableCoinCurrencyIdNotSet
		);

		deploy_contracts();
		assert_ok!(HonzonBridge::set_bridged_stable_coin_address(
			RuntimeOrigin::root(),
			erc20_address()
		));
		// ensure the honzon-bridge pallet account bind the evmaddress
		<EVM as EVMTrait<AccountId>>::set_origin(EvmAccountsModule::get_account_id(&alice_evm_addr()));
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(alice()),
			HonzonBridgeAccount::get(),
			HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
			dollar(1_000_000)
		));

		assert_eq!(Currencies::free_balance(KUSD, &alice()), dollar(1_000_000));
		assert_eq!(
			Currencies::free_balance(KUSD, &HonzonBridgeAccount::get()),
			dollar(1_000_000)
		);
		assert_eq!(
			Currencies::free_balance(HonzonBridge::bridged_stable_coin_currency_id().unwrap(), &alice()),
			ALICE_BALANCE - dollar(1_000_000)
		);
		assert_eq!(
			Currencies::free_balance(
				HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
				&HonzonBridgeAccount::get()
			),
			dollar(1_000_000)
		);

		assert_ok!(HonzonBridge::from_bridged(
			RuntimeOrigin::signed(alice()),
			dollar(5_000)
		));

		assert_eq!(
			Currencies::free_balance(KUSD, &alice()),
			dollar(1_000_000) + dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(KUSD, &HonzonBridgeAccount::get()),
			dollar(1_000_000) - dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(HonzonBridge::bridged_stable_coin_currency_id().unwrap(), &alice()),
			ALICE_BALANCE - dollar(1_000_000) - dollar(5_000)
		);
		assert_eq!(
			Currencies::free_balance(
				HonzonBridge::bridged_stable_coin_currency_id().unwrap(),
				&HonzonBridgeAccount::get()
			),
			dollar(1_000_000) + dollar(5_000)
		);

		System::assert_last_event(RuntimeEvent::HonzonBridge(crate::Event::FromBridged {
			who: alice(),
			amount: dollar(5000),
		}));
	});
}
