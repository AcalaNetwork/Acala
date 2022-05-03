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
use frame_support::log;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::HomaManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Balance, CurrencyId};
use sp_runtime::{FixedPointNumber, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The Homa precompile
///
/// `input` data starts with `action`.
///
/// Actions:
/// - mint

pub struct HomaPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Mint = "mint(address,uint256)",
	RequestRedeem = "requestRedeem(address,uint256,bool)",
	GetExchangeRate = "getExchangeRate()",
	GetEstimatedRewardRate = "getEstimatedRewardRate()",
	GetCommissionRate = "getCommissionRate()",
	GetFastMatchFee = "getFastMatchFee()",
}

impl<Runtime> Precompile for HomaPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_homa::Config + module_prices::Config,
	module_homa::Pallet<Runtime>: HomaManager<Runtime::AccountId, Balance>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<
			Action,
			Runtime::AccountId,
			<Runtime as module_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>::new(input, target_gas_limit(target_gas));

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
			Action::Mint => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"homa: mint, who: {:?}, amount: {:?}",
					&who, amount
				);

				<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::mint(who, amount).map_err(
					|e| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Into::<&str>::into(e).as_bytes().to_vec(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					},
				)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::RequestRedeem => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"homa: mint, who: {:?}, amount: {:?}",
					&who, amount
				);

				<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::request_redeem(
					who, amount, false,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Into::<&str>::into(e).as_bytes().to_vec(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::GetExchangeRate => {
				let rate =
					<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_exchange_rate();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetEstimatedRewardRate => {
				let rate = <module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_estimated_reward_rate();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetCommissionRate => {
				let rate =
					<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_commission_rate();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetFastMatchFee => {
				let rate =
					<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_fast_match_fee();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(rate.into_inner()),
					logs: Default::default(),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_homa::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::Mint => 1,
			Action::RequestRedeem => 1,
			Action::GetExchangeRate => 1,
			Action::GetEstimatedRewardRate => 1,
			Action::GetCommissionRate => 1,
			Action::GetFastMatchFee => 1,
		};
		Ok(cost)
	}
}
