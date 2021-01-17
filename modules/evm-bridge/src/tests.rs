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
