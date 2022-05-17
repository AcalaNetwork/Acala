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
	input::{Input, InputT, Output},
	target_gas_limit,
};
use crate::WeightToGas;
use frame_support::{pallet_prelude::IsType, traits::Get};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::EVMAccountsManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
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
							output: "EvmAddress mapping not found".into(),
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
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;
		let cost = match action {
			Action::GetAccountId => {
				// EVMAccounts::Accounts (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::GetEvmAddress => {
				// EVMAccounts::EvmAddresses (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{alice_evm_addr, new_test_ext, Test};
	use frame_support::assert_noop;
	use hex_literal::hex;

	type EVMAccountsPrecompile = crate::precompile::EVMAccountsPrecompile<Test>;

	#[test]
	fn get_account_id_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getAccountId(address) -> 0xe0b490f7
			let input = hex! {"
				e0b490f7
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// expect output is `evm` padded address
			// evm: -> 0x65766d3a
			let expected_output = hex! {"
				65766d3a 1000000000000000000000000000000000000001 0000000000000000
			"};

			let resp = EVMAccountsPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_evm_address_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getEvmAddress(bytes32) -> 0x0232027e
			// evm: -> 0x65766d3a
			let input = hex! {"
				0232027e
				65766d3a 1000000000000000000000000000000000000001 0000000000000000
			"};

			// expect output is evm address
			let expected_output = hex! {"
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			let resp = EVMAccountsPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// evm address mapping not found
			// normal account_id: ALICE
			let input = hex! {"
				0232027e
				0101010101010101010101010101010101010101010101010101010101010101
			"};

			assert_noop!(
				EVMAccountsPrecompile::execute(&input, Some(10_000), &context, false),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "EvmAddress mapping not found".into(),
					cost: target_gas_limit(Some(10_000)).unwrap(),
				}
			);
		});
	}
}
