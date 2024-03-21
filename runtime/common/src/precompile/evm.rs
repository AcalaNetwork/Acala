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

use super::{
	input::{Input, InputPricer, InputT, Output},
	weights::PrecompileWeights,
};
use crate::WeightToGas;
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult, WeightInfo,
};
use module_support::EVMManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::Balance;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `EVM` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - QueryNewContractExtraBytes.
/// - QueryStorageDepositPerByte.
/// - QueryMaintainer.
/// - QueryDeveloperDeposit.
/// - QueryPublicationFee.
/// - TransferMaintainer. Rest `input` bytes: `from`, `contract`, `new_maintainer`.
pub struct EVMPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryNewContractExtraBytes = "newContractExtraBytes()",
	QueryStorageDepositPerByte = "storageDepositPerByte()",
	QueryMaintainer = "maintainerOf(address)",
	QueryDeveloperDeposit = "developerDeposit()",
	QueryPublicationFee = "publicationFee()",
	TransferMaintainer = "transferMaintainer(address,address,address)",
	EnableDeveloperAccount = "developerEnable(address)",
	DisableDeveloperAccount = "developerDisable(address)",
	QueryDeveloperStatus = "developerStatus(address)",
	PublishContract = "publishContract(address,address)",
}

impl<Runtime> Precompile for EVMPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config,
	module_evm::Pallet<Runtime>: EVMManager<Runtime::AccountId, Balance>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		match action {
			Action::QueryNewContractExtraBytes => {
				let output = module_evm::Pallet::<Runtime>::query_new_contract_extra_bytes();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(output),
				})
			}
			Action::QueryStorageDepositPerByte => {
				let deposit = module_evm::Pallet::<Runtime>::query_storage_deposit_per_byte();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(deposit),
				})
			}
			Action::QueryMaintainer => {
				let contract = input.evm_address_at(1)?;

				let maintainer = module_evm::Pallet::<Runtime>::query_maintainer(&contract).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_address(maintainer),
				})
			}
			Action::QueryDeveloperDeposit => {
				let deposit = module_evm::Pallet::<Runtime>::query_developer_deposit();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(deposit),
				})
			}
			Action::QueryPublicationFee => {
				let fee = module_evm::Pallet::<Runtime>::query_publication_fee();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(fee),
				})
			}
			Action::TransferMaintainer => {
				let from = input.account_id_at(1)?;
				let contract = input.evm_address_at(2)?;
				let new_maintainer = input.evm_address_at(3)?;

				log::debug!(
					target: "evm",
					"evm: from: {:?}, contract: {:?}, new_maintainer: {:?}",
					from, contract, new_maintainer,
				);

				<module_evm::Pallet<Runtime> as EVMManager<Runtime::AccountId, Balance>>::transfer_maintainer(
					from,
					contract,
					new_maintainer,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("Evm TransferMaintainer failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::PublishContract => {
				let who = input.account_id_at(1)?;
				let contract_address = input.evm_address_at(2)?;
				<module_evm::Pallet<Runtime>>::publish_contract_precompile(who, contract_address).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Evm PublishContract failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::DisableDeveloperAccount => {
				let who = input.account_id_at(1)?;
				<module_evm::Pallet<Runtime>>::disable_account_contract_development(&who).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Evm DisableDeveloperAccount failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::EnableDeveloperAccount => {
				let who = input.account_id_at(1)?;
				<module_evm::Pallet<Runtime>>::enable_account_contract_development(&who).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Evm EnableDeveloperAccount failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::QueryDeveloperStatus => {
				let who = input.account_id_at(1)?;
				let developer_status = <module_evm::Pallet<Runtime>>::query_developer_status(&who);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bool(developer_status),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config,
{
	const BASE_COST: u64 = 50;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);
		let action = input.action()?;
		let cost = match action {
			Action::QueryNewContractExtraBytes => {
				let weight = PrecompileWeights::<Runtime>::evm_query_new_contract_extra_bytes();
				WeightToGas::convert(weight)
			}
			Action::QueryStorageDepositPerByte => {
				let weight = PrecompileWeights::<Runtime>::evm_query_storage_deposit_per_byte();
				WeightToGas::convert(weight)
			}
			Action::QueryMaintainer => {
				let weight = PrecompileWeights::<Runtime>::evm_query_maintainer();
				WeightToGas::convert(weight)
			}
			Action::QueryDeveloperDeposit => {
				let weight = PrecompileWeights::<Runtime>::evm_query_developer_deposit();
				WeightToGas::convert(weight)
			}
			Action::QueryPublicationFee => {
				let weight = PrecompileWeights::<Runtime>::evm_query_publication_fee();
				WeightToGas::convert(weight)
			}
			Action::TransferMaintainer => {
				let read_accounts = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_evm::Config>::WeightInfo::transfer_maintainer();
				Self::BASE_COST
					.saturating_add(read_accounts)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::PublishContract => {
				let read_accounts = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_evm::Config>::WeightInfo::publish_contract();
				Self::BASE_COST
					.saturating_add(read_accounts)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::DisableDeveloperAccount => {
				let read_accounts = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_evm::Config>::WeightInfo::disable_contract_development();
				Self::BASE_COST
					.saturating_add(read_accounts)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::EnableDeveloperAccount => {
				let read_accounts = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_evm::Config>::WeightInfo::enable_contract_development();
				Self::BASE_COST
					.saturating_add(read_accounts)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::QueryDeveloperStatus => {
				let weight = PrecompileWeights::<Runtime>::evm_query_developer_status();
				WeightToGas::convert(weight)
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		alice, alice_evm_addr, bob, bob_evm_addr, new_test_ext, EVMModule, RuntimeEvent as TestEvent, RuntimeOrigin,
		System, Test,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context, ExitError, ExitReason, Runner};
	use sp_core::H160;

	type EVMPrecompile = crate::EVMPrecompile<Test>;

	#[test]
	fn developer_status_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// developerStatus(address) -> 0x710f50ff
			// who
			let input = hex! {"
				710f50ff
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// expect output is false as alice has not put a deposit down
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// developerEnable(address) -> 0x504eb6b5
			// who
			let input = hex! {"
				504eb6b5
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			// query developer status again but this time it is enabled

			// developerStatus(address) -> 0x710f50ff
			// who
			let input = hex! {"
				710f50ff
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// expect output is now true as alice now is enabled for developer mode
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000001
			"};

			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// disable alice account for developer mode

			// developerDisable(address) -> 0x757c54c9
			// who
			let input = hex! {"
				757c54c9
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			// query developer status

			// developerStatus(address) -> 0x710f50ff
			// who
			let input = hex! {"
				710f50ff
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// expect output is now false as alice now is disabled again for developer mode
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn publish_contract_works() {
		new_test_ext().execute_with(|| {
			// pragma solidity ^0.5.0;
			//
			// contract Test {
			//	 function multiply(uint a, uint b) public pure returns(uint) {
			// 	 	return a * b;
			// 	 }
			// }
			let contract = hex! {"
				608060405234801561001057600080fd5b5060b88061001f6000396000f3fe60
				80604052348015600f57600080fd5b506004361060285760003560e01c806316
				5c4a1614602d575b600080fd5b606060048036036040811015604157600080fd
				5b8101908080359060200190929190803590602001909291905050506076565b
				6040518082815260200191505060405180910390f35b60008183029050929150
				5056fea265627a7a723158201f3db7301354b88b310868daf4395a6ab6cd42d1
				6b1d8e68cdf4fdd9d34fffbf64736f6c63430005110032
			"};

			assert_ok!(EVMModule::enable_account_contract_development(&alice()));

			// create contract
			let info = <Test as module_evm::Config>::Runner::create(
				alice_evm_addr(),
				contract.to_vec(),
				0,
				21_000_000,
				21_000_000,
				vec![],
				<Test as module_evm::Config>::config(),
			)
			.unwrap();
			let contract_address = info.value;

			assert_eq!(
				contract_address,
				H160::from(hex!("5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d"))
			);

			// multiply(2, 3)
			let multiply = hex! {"
				165c4a16
				0000000000000000000000000000000000000000000000000000000000000002
				0000000000000000000000000000000000000000000000000000000000000003
			"};

			// call method `multiply` will fail, not published yet.
			// The error is shown in the last event.
			// The call extrinsic still succeeds, the evm emits a executed failed event
			assert_ok!(EVMModule::call(
				RuntimeOrigin::signed(bob()),
				contract_address,
				multiply.to_vec(),
				0,
				1000000,
				1000000,
				vec![],
			));
			System::assert_last_event(TestEvent::EVMModule(module_evm::Event::ExecutedFailed {
				from: bob_evm_addr(),
				contract: contract_address,
				exit_reason: ExitReason::Error(ExitError::Other(
					Into::<&str>::into(module_evm::Error::<Test>::NoPermission).into(),
				)),
				output: vec![],
				logs: vec![],
				used_gas: 1000000,
				used_storage: 0,
			}));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// publishContract(address,address) -> 0x3b594ce8
			// maintainer
			// contract_address
			let input = hex! {"
				3b594ce8
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 5f8bd49cd9f0cb2bd5bb9d4320dfe9b61023249d
			"};

			// publish contract with precompile
			let resp = EVMPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			// Same call as above now works as contract is now published
			assert_ok!(EVMModule::call(
				RuntimeOrigin::signed(bob()),
				contract_address,
				multiply.to_vec(),
				0,
				1000000,
				1000000,
				vec![],
			));
			System::assert_last_event(TestEvent::EVMModule(module_evm::Event::Executed {
				from: bob_evm_addr(),
				contract: contract_address,
				logs: vec![],
				used_gas: 21659,
				used_storage: 0,
			}));
		});
	}
}
