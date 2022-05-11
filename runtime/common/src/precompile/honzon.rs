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
};
use crate::WeightToGas;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::HonzonManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::RuntimeDebug;
use sp_std::{marker::PhantomData, prelude::*};

pub struct HonzonPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	AdjustLoan = "adjustLoan(address,address,int256,int256)",
	CloseLoanByDex = "closeLoanByDex(address,address,int256,int256)",
}

impl<Runtime> Precompile for HonzonPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_honzon::Config + module_prices::Config,
	module_honzon::Pallet<Runtime>: HonzonManager<Runtime::AccountId, CurrencyId, Amount, Balance>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, context: &Context, is_static: bool) -> PrecompileResult {
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
			Action::AdjustLoan => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: gas_cost,
				output: vec![],
				logs: Default::default(),
			}),
			Action::CloseLoanByDex => Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: gas_cost,
				output: vec![],
				logs: Default::default(),
			}),
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_honzon::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::AdjustLoan => 1,
			Action::CloseLoanByDex => 1,
		};
		Ok(cost)
	}
}
