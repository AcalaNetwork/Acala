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

use crate::WeightToGas;
use frame_support::{log, traits::Get};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed, WeightInfo,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

use module_support::EVMManager;

use super::input::{Input, InputT, Output};
use primitives::Balance;

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
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			input, target_gas,
		);

		let gas_cost = Pricer::<Runtime>::cost(&input)?;

		if let Some(gas_limit) = target_gas {
			if gas_limit < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let action = input.action()?;

		match action {
			Action::QueryNewContractExtraBytes => {
				let output = module_evm::Pallet::<Runtime>::query_new_contract_extra_bytes();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u32(output),
					logs: Default::default(),
				})
			}
			Action::QueryStorageDepositPerByte => {
				let deposit = module_evm::Pallet::<Runtime>::query_storage_deposit_per_byte();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(deposit),
					logs: Default::default(),
				})
			}
			Action::QueryMaintainer => {
				let contract = input.evm_address_at(1)?;

				let maintainer = module_evm::Pallet::<Runtime>::query_maintainer(contract).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas.unwrap_or_default(),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_address(&maintainer),
					logs: Default::default(),
				})
			}
			Action::QueryDeveloperDeposit => {
				let deposit = module_evm::Pallet::<Runtime>::query_developer_deposit();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(deposit),
					logs: Default::default(),
				})
			}
			Action::QueryPublicationFee => {
				let fee = module_evm::Pallet::<Runtime>::query_publication_fee();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(fee),
					logs: Default::default(),
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
					output: Into::<&str>::into(e).as_bytes().to_vec(),
					cost: target_gas.unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::PublishContract => {
				let who = input.account_id_at(1)?;
				let contract_address = input.evm_address_at(2)?;
				<module_evm::Pallet<Runtime>>::publish_contract_precompile(who, contract_address).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas.unwrap_or_default(),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::DisableDeveloperAccount => {
				let who = input.account_id_at(1)?;
				<module_evm::Pallet<Runtime>>::disable_account_contract_development(who).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas.unwrap_or_default(),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::EnableDeveloperAccount => {
				let who = input.account_id_at(1)?;
				<module_evm::Pallet<Runtime>>::enable_account_contract_development(who).map_err(|e| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas.unwrap_or_default(),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::QueryDeveloperStatus => {
				let who = input.account_id_at(1)?;
				let developer_status = <module_evm::Pallet<Runtime>>::query_developer_status(who);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_bool(developer_status),
					logs: Default::default(),
				})
			}
		}
	}
}

pub struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config,
{
	pub const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;
		let cost = match action {
			Action::QueryNewContractExtraBytes | Action::QueryStorageDepositPerByte => Self::BASE_COST,
			Action::QueryMaintainer => {
				// EVM::Accounts
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);
				WeightToGas::convert(weight)
			}
			Action::QueryDeveloperDeposit | Action::QueryPublicationFee => Self::BASE_COST * 2,
			Action::TransferMaintainer => {
				let weight = <Runtime as module_evm::Config>::WeightInfo::transfer_maintainer();
				WeightToGas::convert(weight)
			}
			Action::PublishContract => {
				let weight = <Runtime as module_evm::Config>::WeightInfo::publish_contract();
				WeightToGas::convert(weight)
			}
			Action::DisableDeveloperAccount => {
				let weight = <Runtime as module_evm::Config>::WeightInfo::disable_contract_development();
				WeightToGas::convert(weight)
			}
			Action::EnableDeveloperAccount => {
				let weight = <Runtime as module_evm::Config>::WeightInfo::enable_contract_development();
				WeightToGas::convert(weight)
			}
			Action::QueryDeveloperStatus => {
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);
				WeightToGas::convert(weight)
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}
