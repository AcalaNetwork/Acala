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

//! Unit tests for the chainsafe transfer module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::BadOrigin;

#[test]
fn register_resource_id_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_eq!(ChainSafeTransfer::resource_ids(ACA::get()), None);
		assert_eq!(ChainSafeTransfer::currency_ids(ACAResourceId::get()), None);

		assert_noop!(
			ChainSafeTransfer::register_resource_id(Origin::signed(ALICE), ACAResourceId::get(), ACA::get()),
			BadOrigin,
		);

		assert_noop!(
			ChainSafeTransfer::register_resource_id(
				Origin::signed(RegistorOrigin::get()),
				ACAResourceId::get(),
				WETH::get()
			),
			Error::<Runtime>::ResourceIdCurrencyIdNotMatch,
		);

		assert_noop!(
			ChainSafeTransfer::register_resource_id(
				Origin::signed(RegistorOrigin::get()),
				WETHResourceId::get(),
				ACA::get()
			),
			Error::<Runtime>::ResourceIdCurrencyIdNotMatch,
		);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			ACAResourceId::get(),
			ACA::get()
		));
		let register_event =
			Event::ChainSafeTransfer(crate::Event::RegisterResourceId(ACAResourceId::get(), ACA::get()));
		assert!(System::events().iter().any(|record| record.event == register_event));

		assert_eq!(ChainSafeTransfer::resource_ids(ACA::get()), Some(ACAResourceId::get()));
		assert_eq!(ChainSafeTransfer::currency_ids(ACAResourceId::get()), Some(ACA::get()));

		assert_noop!(
			ChainSafeTransfer::register_resource_id(
				Origin::signed(RegistorOrigin::get()),
				ACAResourceId::get(),
				ACA::get()
			),
			Error::<Runtime>::ResourceIdAlreadyRegistered,
		);
	});
}

#[test]
fn remove_resource_id_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			ACAResourceId::get(),
			ACA::get()
		));
		assert_eq!(ChainSafeTransfer::resource_ids(ACA::get()), Some(ACAResourceId::get()));
		assert_eq!(ChainSafeTransfer::currency_ids(ACAResourceId::get()), Some(ACA::get()));

		assert_noop!(
			ChainSafeTransfer::remove_resource_id(Origin::signed(ALICE), ACAResourceId::get()),
			BadOrigin,
		);

		assert_ok!(ChainSafeTransfer::remove_resource_id(
			Origin::signed(RegistorOrigin::get()),
			ACAResourceId::get()
		));
		let unregister_event =
			Event::ChainSafeTransfer(crate::Event::UnregisterResourceId(ACAResourceId::get(), ACA::get()));
		assert!(System::events().iter().any(|record| record.event == unregister_event));
	});
}

#[test]
fn is_origin_chain_resource_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(ChainSafeTransfer::is_origin_chain_resource(ACAResourceId::get()), true);
		assert_eq!(
			ChainSafeTransfer::is_origin_chain_resource(WETHResourceId::get()),
			false
		);
	});
}

#[test]
fn do_transfer_to_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		let dest_chain_id: chainbridge::ChainId = 0;

		assert_noop!(
			ChainSafeTransfer::do_transfer_to_bridge(&ALICE, ACA::get(), dest_chain_id, vec![1], 10),
			Error::<Runtime>::InvalidDestChainId,
		);

		assert_ok!(ChainBridge::whitelist_chain(
			Origin::signed(AdminOrigin::get()),
			dest_chain_id
		));
		assert_noop!(
			ChainSafeTransfer::do_transfer_to_bridge(&ALICE, ACA::get(), dest_chain_id, vec![1], 10),
			Error::<Runtime>::ResourceIdNotRegistered,
		);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			ACAResourceId::get(),
			ACA::get()
		));
		assert_eq!(Tokens::total_issuance(ACA::get()), 1000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ALICE), 1000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ChainBridge::account_id()), 0);

		assert_ok!(ChainSafeTransfer::do_transfer_to_bridge(
			&ALICE,
			ACA::get(),
			dest_chain_id,
			vec![1],
			10
		));
		assert_eq!(Tokens::total_issuance(ACA::get()), 1000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ALICE), 990);
		assert_eq!(Tokens::free_balance(ACA::get(), &ChainBridge::account_id()), 10);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			WETHResourceId::get(),
			WETH::get()
		));
		assert_ok!(Tokens::deposit(WETH::get(), &ALICE, 1000));
		assert_eq!(Tokens::total_issuance(WETH::get()), 1000);
		assert_eq!(Tokens::free_balance(WETH::get(), &ALICE), 1000);
		assert_eq!(Tokens::free_balance(WETH::get(), &ChainBridge::account_id()), 0);

		assert_ok!(ChainSafeTransfer::do_transfer_to_bridge(
			&ALICE,
			WETH::get(),
			dest_chain_id,
			vec![1],
			20
		));
		assert_eq!(Tokens::total_issuance(WETH::get()), 980);
		assert_eq!(Tokens::free_balance(WETH::get(), &ALICE), 980);
		assert_eq!(Tokens::free_balance(WETH::get(), &ChainBridge::account_id()), 0);
	});
}

#[test]
fn transfer_from_bridge_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			ChainSafeTransfer::transfer_from_bridge(Origin::signed(ALICE), ALICE, 500, ACAResourceId::get()),
			BadOrigin,
		);

		assert_noop!(
			ChainSafeTransfer::transfer_from_bridge(
				Origin::signed(ChainBridge::account_id()),
				ALICE,
				500,
				ACAResourceId::get()
			),
			Error::<Runtime>::ResourceIdNotRegistered,
		);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			ACAResourceId::get(),
			ACA::get()
		));
		assert_ok!(Tokens::deposit(ACA::get(), &ChainBridge::account_id(), 1000));
		assert_eq!(Tokens::total_issuance(ACA::get()), 2000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ALICE), 1000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ChainBridge::account_id()), 1000);

		assert_ok!(ChainSafeTransfer::transfer_from_bridge(
			Origin::signed(ChainBridge::account_id()),
			ALICE,
			500,
			ACAResourceId::get()
		));
		assert_eq!(Tokens::total_issuance(ACA::get()), 2000);
		assert_eq!(Tokens::free_balance(ACA::get(), &ALICE), 1500);
		assert_eq!(Tokens::free_balance(ACA::get(), &ChainBridge::account_id()), 500);

		assert_ok!(ChainSafeTransfer::register_resource_id(
			Origin::signed(RegistorOrigin::get()),
			WETHResourceId::get(),
			WETH::get()
		));
		assert_eq!(Tokens::total_issuance(WETH::get()), 0);
		assert_eq!(Tokens::free_balance(WETH::get(), &ALICE), 0);
		assert_eq!(Tokens::free_balance(WETH::get(), &ChainBridge::account_id()), 0);

		assert_ok!(ChainSafeTransfer::transfer_from_bridge(
			Origin::signed(ChainBridge::account_id()),
			ALICE,
			500,
			WETHResourceId::get()
		));
		assert_eq!(Tokens::total_issuance(WETH::get()), 500);
		assert_eq!(Tokens::free_balance(WETH::get(), &ALICE), 500);
		assert_eq!(Tokens::free_balance(WETH::get(), &ChainBridge::account_id()), 0);
	});
}
