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
use frame_support::traits::Get;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_incentives::WeightInfo;
use module_support::{IncentivesManager, PoolId};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Convert, FixedPointNumber, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

pub struct IncentivesPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetIncentiveRewardAmount = "getIncentiveRewardAmount(PoolId,address,address)",
	GetDexRewardRate = "getDexRewardRate(PoolId,address)",
	DepositDexShare = "depositDexShare(address,address,uint128)",
	WithdrawDexShare = "withdrawDexShare(address,address,uint128)",
	ClaimRewards = "claimRewards(address,PoolId,address)",
}

impl<Runtime> Precompile for IncentivesPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_incentives::Config + module_prices::Config,
	module_incentives::Pallet<Runtime>: IncentivesManager<Runtime::AccountId, Balance, CurrencyId, PoolId>,
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
			Action::GetIncentiveRewardAmount => {
				let pool = input.u32_at(1)?;
				let pool_currency_id = input.currency_id_at(2)?;
				let reward_currency_id = input.currency_id_at(3)?;
				let pool_id = init_pool_id(pool, pool_currency_id, target_gas)?;

				let value = <module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::get_incentive_reward_amount(pool_id, reward_currency_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(value),
					logs: Default::default(),
				})
			}
			Action::GetDexRewardRate => {
				let pool = input.u32_at(1)?;
				let pool_currency_id = input.currency_id_at(2)?;
				let pool_id = init_pool_id(pool, pool_currency_id, target_gas)?;

				let value = <module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::get_dex_reward_rate(pool_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::default().encode_u128(value.into_inner()),
					logs: Default::default(),
				})
			}
			Action::DepositDexShare => {
				let who = input.account_id_at(1)?;
				let lp_currency_id = input.currency_id_at(2)?;
				let amount = input.balance_at(3)?;

				<module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::deposit_dex_share(&who, lp_currency_id, amount)
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
			Action::WithdrawDexShare => {
				let who = input.account_id_at(1)?;
				let lp_currency_id = input.currency_id_at(2)?;
				let amount = input.balance_at(3)?;

				<module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::withdraw_dex_share(&who, lp_currency_id, amount)
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
			Action::ClaimRewards => {
				let who = input.account_id_at(1)?;
				let pool = input.u32_at(2)?;
				let pool_currency_id = input.currency_id_at(3)?;
				let pool_id = init_pool_id(pool, pool_currency_id, target_gas)?;

				<module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::claim_rewards(who, pool_id)
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
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_incentives::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::GetIncentiveRewardAmount => {
				let pool_currency_id = input.currency_id_at(2)?;
				let reward_currency_id = input.currency_id_at(3)?;
				let read_pool_currency = InputPricer::<Runtime>::read_currency(pool_currency_id);
				let read_reward_currency = InputPricer::<Runtime>::read_currency(reward_currency_id);

				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_pool_currency)
					.saturating_add(read_reward_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetDexRewardRate => {
				let pool_currency_id = input.currency_id_at(2)?;
				let read_pool_currency = InputPricer::<Runtime>::read_currency(pool_currency_id);

				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_pool_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::DepositDexShare => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let lp_currency_id = input.currency_id_at(2)?;
				let read_lp_currency = InputPricer::<Runtime>::read_currency(lp_currency_id);

				let weight = <Runtime as module_incentives::Config>::WeightInfo::deposit_dex_share();

				Self::BASE_COST
					.saturating_add(read_lp_currency)
					.saturating_add(read_account)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::WithdrawDexShare => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let lp_currency_id = input.currency_id_at(2)?;
				let read_lp_currency = InputPricer::<Runtime>::read_currency(lp_currency_id);

				let weight = <Runtime as module_incentives::Config>::WeightInfo::withdraw_dex_share();

				Self::BASE_COST
					.saturating_add(read_lp_currency)
					.saturating_add(read_account)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::ClaimRewards => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let pool_currency_id = input.currency_id_at(3)?;
				let read_pool_currency = InputPricer::<Runtime>::read_currency(pool_currency_id);

				let weight = <Runtime as module_incentives::Config>::WeightInfo::claim_rewards();

				Self::BASE_COST
					.saturating_add(read_pool_currency)
					.saturating_add(read_account)
					.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

fn init_pool_id(
	pool_id_number: u32,
	pool_currency_id: CurrencyId,
	target_gas: Option<u64>,
) -> Result<PoolId, PrecompileFailure> {
	match pool_id_number {
		0 => Ok(PoolId::Loans(pool_currency_id)),
		1 => Ok(PoolId::Dex(pool_currency_id)),
		// Shouldn't happen as solidity compiler should not allow nonexistent enum value
		_ => Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "Incentives: Invalid enum value".into(),
			cost: target_gas_limit(target_gas).unwrap_or_default(),
		}),
	}
}
