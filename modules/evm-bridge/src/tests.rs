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
use mock::{alice, alice_evm_addr, bob, bob_evm_addr, deploy_contracts, erc20_address, ExtBuilder, Runtime};

#[test]
fn should_read_name() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::name(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(
					b"long string name, long string name, long string name, long string name, long string name"
						.to_vec()
				)
			);
		});
}

#[test]
fn should_read_symbol() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::symbol(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(b"TestToken".to_vec())
			);
		});
}

#[test]
fn should_read_decimals() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::decimals(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(17)
			);
		});
}

#[test]
fn should_read_total_supply() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_eq!(
				EVMBridge::<Runtime>::total_supply(InvokeContext {
					contract: erc20_address(),
					sender: Default::default(),
					origin: Default::default(),
				}),
				Ok(10000)
			);
		});
}

#[test]
fn should_read_balance_of() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			let context = InvokeContext {
				contract: erc20_address(),
				sender: Default::default(),
				origin: Default::default(),
			};

			assert_eq!(EVMBridge::<Runtime>::balance_of(context, bob_evm_addr()), Ok(0));

			assert_eq!(EVMBridge::<Runtime>::balance_of(context, alice_evm_addr()), Ok(10000));

			assert_eq!(EVMBridge::<Runtime>::balance_of(context, bob_evm_addr()), Ok(0));
		});
}

#[test]
fn should_transfer() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000), (bob(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_err!(
				EVMBridge::<Runtime>::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob_evm_addr(),
						origin: bob_evm_addr(),
					},
					alice_evm_addr(),
					10
				),
				Error::<Runtime>::ExecutionRevert
			);

			assert_ok!(EVMBridge::<Runtime>::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: alice_evm_addr(),
					origin: alice_evm_addr(),
				},
				bob_evm_addr(),
				100
			));
			assert_eq!(
				EVMBridge::<Runtime>::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice_evm_addr(),
						origin: alice_evm_addr(),
					},
					bob_evm_addr()
				),
				Ok(100)
			);

			assert_ok!(EVMBridge::<Runtime>::transfer(
				InvokeContext {
					contract: erc20_address(),
					sender: bob_evm_addr(),
					origin: bob_evm_addr(),
				},
				alice_evm_addr(),
				10
			));

			assert_eq!(
				EVMBridge::<Runtime>::balance_of(
					InvokeContext {
						contract: erc20_address(),
						sender: alice_evm_addr(),
						origin: bob_evm_addr(),
					},
					bob_evm_addr()
				),
				Ok(90)
			);

			assert_err!(
				EVMBridge::<Runtime>::transfer(
					InvokeContext {
						contract: erc20_address(),
						sender: bob_evm_addr(),
						origin: bob_evm_addr(),
					},
					alice_evm_addr(),
					100
				),
				Error::<Runtime>::ExecutionRevert
			);
		});
}
