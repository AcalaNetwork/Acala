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

//! Unit tests for the evm-bridge module.

#![cfg(test)]

use super::*;
use frame_support::{assert_err, assert_ok};
use mock::{alice, bob, erc20_address, EvmBridgeModule, ExtBuilder, Runtime};
use primitives::evm::AddressMapping;

#[test]
fn should_read_total_supply() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			EvmBridgeModule::total_supply(InvokeContext {
				contract: erc20_address(),
				sender: Default::default(),
				origin: Default::default(),
			}),
			Ok(u128::max_value())
		);
	});
}

#[test]
fn should_read_balance_of() {
	ExtBuilder::default().build().execute_with(|| {
		let context = InvokeContext {
			contract: erc20_address(),
			sender: Default::default(),
			origin: Default::default(),
		};

		assert_eq!(EvmBridgeModule::balance_of(context, bob()), Ok(0));

		assert_eq!(EvmBridgeModule::balance_of(context, alice()), Ok(u128::max_value()));

		assert_eq!(EvmBridgeModule::balance_of(context, bob()), Ok(0));
	});
}

#[test]
fn should_transfer() {
	ExtBuilder::default()
		.balances(vec![
			(
				<Runtime as module_evm::Config>::AddressMapping::get_account_id(&alice()),
				100000,
			),
			(
				<Runtime as module_evm::Config>::AddressMapping::get_account_id(&bob()),
				100000,
			),
		])
		.build()
		.execute_with(|| {
			assert_err!(
				EvmBridgeModule::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob(),
						origin: bob(),
					},
					alice(),
					10
				),
				Error::<Runtime>::ExecutionRevert
			);

			assert_ok!(EvmBridgeModule::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: alice(),
					origin: alice(),
				},
				bob(),
				100
			));
			assert_eq!(
				EvmBridgeModule::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice(),
						origin: alice(),
					},
					bob()
				),
				Ok(100)
			);

			assert_ok!(EvmBridgeModule::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: bob(),
					origin: bob(),
				},
				alice(),
				10
			));

			assert_eq!(
				EvmBridgeModule::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice(),
						origin: bob(),
					},
					bob()
				),
				Ok(90)
			);

			assert_err!(
				EvmBridgeModule::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob(),
						origin: bob(),
					},
					alice(),
					100
				),
				Error::<Runtime>::ExecutionRevert
			);
		});
}
