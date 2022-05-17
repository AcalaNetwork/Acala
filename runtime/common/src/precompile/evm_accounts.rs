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

use super::{
	input::{Input, InputPricer, InputT, Output},
	target_gas_limit,
	weights::PrecompileWeights,
};
use crate::WeightToGas;
use codec::Encode;
use frame_support::pallet_prelude::IsType;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed, WeightInfo,
};
use module_support::EVMAccountsManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::Balance;
use sp_runtime::{traits::Convert, AccountId32, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `EVMAccounts` impl precompile.
///
/// `input` data starts with `action`.
///
/// Actions:
/// - GetAccountId.
/// - GetEvmAddress.
pub struct EVMAccountsPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetAccountId = "getAccountId(address)",
	GetEvmAddress = "getEvmAddress(bytes32)",
}

impl<Runtime> Precompile for EVMAccountsPrecompile<Runtime>
where
	Runtime::AccountId: IsType<AccountId32>,
	Runtime: module_evm_accounts::Config + module_prices::Config,
	module_evm_accounts::Pallet<Runtime>: EVMAccountsManager<Runtime::AccountId>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			input,
			target_gas_limit(target_gas),
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
			Action::GetAccountId => {
				let address = input.evm_address_at(1)?;

				let output = module_evm_accounts::Pallet::<Runtime>::get_account_id(&address);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_fixed_bytes(&output.into().as_ref()),
					logs: Default::default(),
				})
			}
			Action::GetEvmAddress => {
				// bytes32
				let input_data = input.bytes_at(1, 32)?;

				let mut buf = [0u8; 32];
				buf.copy_from_slice(&input_data[..]);
				let account_id: Runtime::AccountId = AccountId32::from(buf).into();

				let address =
					module_evm_accounts::Pallet::<Runtime>::get_evm_address(&account_id).ok_or_else(|| {
						PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: "Get EvmAddress failed".into(),
							cost: target_gas_limit(target_gas).unwrap_or_default(),
						}
					})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_address(&address),
					logs: Default::default(),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm_accounts::Config + module_prices::Config,
{
	const BASE_COST: u64 = 50;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;
		let cost = match action {
			Action::GetAccountId => {
				let weight = PrecompileWeights::<Runtime>::evm_query_new_contract_extra_bytes();
				WeightToGas::convert(weight)
			}
			Action::GetEvmAddress => {
				let weight = PrecompileWeights::<Runtime>::evm_query_storage_deposit_per_byte();
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
		alice_evm_addr, bob, bob_evm_addr, new_test_ext, EVMModule, Event as TestEvent, Origin, System, Test,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm::{ExitReason, Runner};
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

			let resp = EVMPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// developerEnable(address) -> 0x504eb6b5
			// who
			let input = hex! {"
				504eb6b5
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			let resp = EVMPrecompile::execute(&input, None, &context, false).unwrap();
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

			let resp = EVMPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// disable alice account for developer mode

			// developerDisable(address) -> 0x757c54c9
			// who
			let input = hex! {"
				757c54c9
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			let resp = EVMPrecompile::execute(&input, None, &context, false).unwrap();
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

			let resp = EVMPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}
}
