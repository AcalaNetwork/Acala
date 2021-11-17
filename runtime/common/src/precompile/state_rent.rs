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

use crate::precompile::PrecompileOutput;
use frame_support::log;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_runtime::RuntimeDebug;
use sp_std::{borrow::Cow, fmt::Debug, marker::PhantomData, prelude::*, result};

use module_support::{AddressMapping as AddressMappingT, EVMStateRentTrait, Erc20InfoMapping as Erc20InfoMappingT};

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
/// - QueryDeploymentFee.
/// - TransferMaintainer. Rest `input` bytes: `from`, `contract`, `new_maintainer`.
pub struct StateRentPrecompile<AccountId, AddressMapping, Erc20InfoMapping, EVM>(
	PhantomData<(AccountId, AddressMapping, Erc20InfoMapping, EVM)>,
);

#[module_evm_utiltity_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryNewContractExtraBytes = "newContractExtraBytes()",
	QueryStorageDepositPerByte = "storageDepositPerByte()",
	QueryMaintainer = "maintainerOf(address)",
	QueryDeveloperDeposit = "developerDeposit()",
	QueryDeploymentFee = "deploymentFee()",
	TransferMaintainer = "transferMaintainer(address,address,address)",
}

impl<AccountId, AddressMapping, Erc20InfoMapping, EVM> Precompile
	for StateRentPrecompile<AccountId, AddressMapping, Erc20InfoMapping, EVM>
where
	AccountId: Clone + Debug,
	AddressMapping: AddressMappingT<AccountId>,
	Erc20InfoMapping: Erc20InfoMappingT,
	EVM: EVMStateRentTrait<AccountId, Balance>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, AccountId, AddressMapping, Erc20InfoMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::QueryNewContractExtraBytes => {
				let output = EVM::query_new_contract_extra_bytes();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u32(output),
					logs: Default::default(),
				})
			}
			Action::QueryStorageDepositPerByte => {
				let deposit = EVM::query_storage_deposit_per_byte();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(deposit),
					logs: Default::default(),
				})
			}
			Action::QueryMaintainer => {
				let contract = input.evm_address_at(1)?;

				let maintainer =
					EVM::query_maintainer(contract).map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_address(&maintainer),
					logs: Default::default(),
				})
			}
			Action::QueryDeveloperDeposit => {
				let deposit = EVM::query_developer_deposit();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(deposit),
					logs: Default::default(),
				})
			}
			Action::QueryDeploymentFee => {
				let fee = EVM::query_deployment_fee();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
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
					"state_rent: from: {:?}, contract: {:?}, new_maintainer: {:?}",
					from, contract, new_maintainer,
				);

				EVM::transfer_maintainer(from, contract, new_maintainer)
					.map_err(|e| ExitError::Other(Cow::Borrowed(e.into())))?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: vec![],
					logs: Default::default(),
				})
			}
		}
	}
}
