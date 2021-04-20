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
use sha3::{Digest, Keccak256};
use support::AddressMapping;

#[test]
fn method_hash_works() {
	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"name()");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_NAME.to_be_bytes().to_vec());

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"symbol()");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_SYMBOL.to_be_bytes().to_vec());

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"decimals()");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_DECIMALS.to_be_bytes().to_vec());

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"totalSupply()");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_TOTAL_SUPPLY.to_be_bytes().to_vec());

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"balanceOf(address)");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_BALANCE_OF.to_be_bytes().to_vec());

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(b"transfer(address,uint256)");
	// read hash digest
	let result = hasher.finalize();
	assert_eq!(result[..4], METHOD_TRANSFER.to_be_bytes().to_vec());
}

#[test]
fn should_read_name() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			EvmBridgeModule::name(InvokeContext {
				contract: erc20_address(),
				sender: Default::default(),
				origin: Default::default(),
			}),
			Ok(b"long string name, long string name, long string name, long string name, long string name".to_vec())
		);
	});
}

#[test]
fn should_read_symbol() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			EvmBridgeModule::symbol(InvokeContext {
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
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			EvmBridgeModule::decimals(InvokeContext {
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
