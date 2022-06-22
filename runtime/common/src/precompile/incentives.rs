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

/// The Incentives precompile
///
/// `input` data starts with `action`.
///
/// Actions:
///  - GetIncentiveRewardAmount `input` bytes: `pool`, `pool_currency_id`, `reward_currency_id`.
///  - GetDexRewardAmount `input` bytes: `lp_currency_id`.
///  - DepositDexShare `input` bytes: `who`, `lp_currency_id`, `amount`.
///  - WithdrawDexShare `input` bytes: `who`, `lp_currency_id`, `amount`.
///  - ClaimRewards `input` bytes: `who`, `pool`, `pool_currency_id`.
///  - GetClaimRewardDeductionRate `input` bytes: `pool`, `pool_currency_id`.
///  - GetPendingRewards `input` bytes: `reward_currencies`, `pool`, `pool_currency_id`, `who`.
pub struct IncentivesPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetIncentiveRewardAmount = "getIncentiveRewardAmount(PoolId,address,address)",
	GetDexRewardRate = "getDexRewardRate(address)",
	DepositDexShare = "depositDexShare(address,address,uint256)",
	WithdrawDexShare = "withdrawDexShare(address,address,uint256)",
	ClaimRewards = "claimRewards(address,PoolId,address)",
	GetClaimRewardDeductionRate = "getClaimRewardDeductionRate(PoolId,address)",
	GetPendingRewards = "getPendingRewards(address[],PoolId,address,address)",
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
					output: Output::encode_uint(value),
					logs: Default::default(),
				})
			}
			Action::GetDexRewardRate => {
				let pool_currency_id = input.currency_id_at(1)?;
				let pool_id = PoolId::Dex(pool_currency_id);

				let value = <module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::get_dex_reward_rate(pool_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(value.into_inner()),
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
			Action::GetClaimRewardDeductionRate => {
				let pool = input.u32_at(1)?;
				let pool_currency_id = input.currency_id_at(2)?;
				let pool_id = init_pool_id(pool, pool_currency_id, target_gas)?;

				let value = <module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::get_claim_reward_deduction_rate(pool_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(value.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetPendingRewards => {
				// solidity abi encode array will add an offset at input[1]
				let pool = input.u32_at(2)?;
				let pool_currency_id = input.currency_id_at(3)?;
				let pool_id = init_pool_id(pool, pool_currency_id, target_gas)?;
				let who = input.account_id_at(4)?;
				let reward_currency_ids_len = input.u32_at(5)?;
				let mut reward_currency_ids = vec![];
				for i in 0..reward_currency_ids_len {
					reward_currency_ids.push(input.currency_id_at((6 + i) as usize)?);
				}

				let value = <module_incentives::Pallet<Runtime> as IncentivesManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
					PoolId,
				>>::get_pending_rewards(pool_id, who, reward_currency_ids);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint_array(value),
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
				let pool_currency_id = input.currency_id_at(1)?;
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
			Action::GetClaimRewardDeductionRate => {
				let pool_currency_id = input.currency_id_at(2)?;
				let read_pool_currency = InputPricer::<Runtime>::read_currency(pool_currency_id);

				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_pool_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetPendingRewards => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let pool_currency_id = input.currency_id_at(3)?;
				let mut read_currency = InputPricer::<Runtime>::read_currency(pool_currency_id);
				let reward_currency_ids_len = input.u32_at(5)?;

				for i in 0..reward_currency_ids_len {
					let currency_id = input.currency_id_at((6 + i) as usize)?;
					read_currency = read_currency.saturating_add(InputPricer::<Runtime>::read_currency(currency_id));
				}

				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency)
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

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompile::mock::{
		alice, alice_evm_addr, bob, new_test_ext, Currencies, Incentives, Origin, Rewards, Test, Tokens, ACA, ALICE,
		AUSD, DOT, LP_ACA_AUSD,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_support::Rate;
	use orml_rewards::PoolInfo;
	use orml_traits::MultiCurrency;
	use sp_runtime::FixedU128;

	type IncentivesPrecompile = super::IncentivesPrecompile<Test>;

	#[test]
	fn get_incentive_reward_amount_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Incentives::update_incentive_rewards(
				Origin::signed(ALICE),
				vec![(PoolId::Loans(DOT), vec![(DOT, 100)])]
			));

			// getIncetiveRewardAmount(PoolId,address,addres) => 0x7469000d
			// pool
			// pool_currency_id
			// reward_currency_id
			let input = hex! {"
				7469000d
				00000000000000000000000000000000 00000000000000000000000000000000
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			// value of 100
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000064
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_dex_reward_rate_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Incentives::update_dex_saving_rewards(
				Origin::signed(ALICE),
				vec![(PoolId::Dex(LP_ACA_AUSD), FixedU128::saturating_from_rational(1, 10))]
			));

			// getDexRewardRate(address) => 0x7ec93136
			// lp_currency_id
			let input = hex! {"
				7ec93136
				000000000000000000000000 0000000000000000000200000000000000000001
			"};

			// value for FixedU128::saturating_from_rational(1,10)
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000016345785d8a0000
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn deposit_dex_share_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::deposit(LP_ACA_AUSD, &alice(), 1_000_000_000));

			// depositDexShare(address,address,uint256) => 0xc17ca2a6
			// who
			// lp_currency_id
			// amount
			let input = hex! {"
				c17ca2a6
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000200000000000000000001
				00000000000000000000000000000000 00000000000000000000000000100000
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);

			assert_eq!(
				Rewards::pool_infos(PoolId::Dex(LP_ACA_AUSD)),
				PoolInfo {
					total_shares: 1048576,
					..Default::default()
				}
			);
			assert_eq!(
				Rewards::shares_and_withdrawn_rewards(PoolId::Dex(LP_ACA_AUSD), alice()),
				(1048576, Default::default())
			);
		});
	}

	#[test]
	fn withdraw_dex_share_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::deposit(LP_ACA_AUSD, &alice(), 1_000_000_000));
			assert_ok!(Incentives::deposit_dex_share(
				Origin::signed(alice()),
				LP_ACA_AUSD,
				100_000
			));

			// withdrawDexShare(address,address,uint256) => 0xdae3ac69
			// who
			// lp_currency_id
			// amount
			let input = hex! {"
				dae3ac69
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000200000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000100
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);

			assert_eq!(
				Rewards::pool_infos(PoolId::Dex(LP_ACA_AUSD)),
				PoolInfo {
					total_shares: 99744,
					..Default::default()
				}
			);
			assert_eq!(
				Rewards::shares_and_withdrawn_rewards(PoolId::Dex(LP_ACA_AUSD), alice()),
				(99744, Default::default())
			);
		});
	}

	#[test]
	fn claim_rewards_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Tokens::deposit(ACA, &alice(), 1_000));
			assert_ok!(Tokens::deposit(ACA, &bob(), 1_000));
			assert_ok!(Tokens::deposit(ACA, &Incentives::account_id(), 1_000_000));
			assert_ok!(Tokens::deposit(AUSD, &Incentives::account_id(), 1_000_000));

			assert_ok!(Incentives::update_claim_reward_deduction_rates(
				Origin::signed(ALICE),
				vec![(PoolId::Loans(ACA), Rate::saturating_from_rational(50, 100)),]
			));
			Rewards::add_share(&alice(), &PoolId::Loans(ACA), 100);
			assert_ok!(Rewards::accumulate_reward(&PoolId::Loans(ACA), ACA, 1_000));
			Rewards::add_share(&bob(), &PoolId::Loans(ACA), 100);
			assert_ok!(Rewards::accumulate_reward(&PoolId::Loans(ACA), ACA, 1_000));

			assert_eq!(
				Rewards::pool_infos(PoolId::Loans(ACA)),
				PoolInfo {
					total_shares: 200,
					rewards: vec![(ACA, (3_000, 1_000))].into_iter().collect(),
				}
			);

			// claimRewards(address,PoolId,address) => 0xe12eab9b
			// who
			// pool
			// pool_currency_id
			let input = hex! {"
				e12eab9b
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000000
				000000000000000000000000 0000000000000000000100000000000000000000
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);

			assert_eq!(
				Rewards::pool_infos(PoolId::Loans(ACA)),
				PoolInfo {
					total_shares: 200,
					rewards: vec![(ACA, (3_750, 2_500))].into_iter().collect(),
				}
			);
			assert_eq!(
				Rewards::shares_and_withdrawn_rewards(PoolId::Loans(ACA), alice()),
				(100, vec![(ACA, 1_500)].into_iter().collect())
			);
		});
	}

	#[test]
	fn get_claim_reward_deduction_rate_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Incentives::update_claim_reward_deduction_rates(
				Origin::signed(ALICE),
				vec![(PoolId::Dex(LP_ACA_AUSD), FixedU128::saturating_from_rational(1, 10))]
			));

			// getClaimRewardDeductionRate(PoolId,address) => 0xa2e2fc8e
			// pool
			// pool_currency_id
			let input = hex! {"
				a2e2fc8e
				00000000000000000000000000000000 00000000000000000000000000000001
				000000000000000000000000 0000000000000000000200000000000000000001
			"};

			// value for FixedU128::saturating_from_rational(1,10)
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000016345785d8a0000
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_pending_rewards_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Tokens::deposit(ACA, &alice(), 1_000));
			assert_ok!(Tokens::deposit(ACA, &bob(), 1_000));
			assert_ok!(Tokens::deposit(ACA, &Incentives::account_id(), 1_000_000));
			assert_ok!(Tokens::deposit(AUSD, &Incentives::account_id(), 1_000_000));

			assert_ok!(Incentives::update_claim_reward_deduction_rates(
				Origin::signed(ALICE),
				vec![(PoolId::Loans(ACA), Rate::saturating_from_rational(50, 100)),]
			));
			Rewards::add_share(&alice(), &PoolId::Loans(ACA), 100);
			assert_ok!(Rewards::accumulate_reward(&PoolId::Loans(ACA), ACA, 1_000));
			Rewards::add_share(&bob(), &PoolId::Loans(ACA), 100);
			assert_ok!(Rewards::accumulate_reward(&PoolId::Loans(ACA), AUSD, 1_000));
			Rewards::remove_share(&alice(), &PoolId::Loans(ACA), 100);

			assert_eq!(
				Incentives::get_pending_rewards(PoolId::Loans(ACA), alice(), vec![ACA, AUSD]),
				vec![1000, 500]
			);

			// getPendingRewards(address[],PoolId,address,address) -> 0x0eb797b1
			// offset
			// pool_id
			// pool_currency_id
			// who
			// currency_ids_len
			// ACA
			// AUSD
			let input = hex! {"
				0eb797b1
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000000
				000000000000000000000000 0000000000000000000100000000000000000000
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000002
				000000000000000000000000 0000000000000000000100000000000000000000
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// encoded array of [1000, 500]
			// offset
			// array_len
			// value_1
			// value_2
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000020
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 000000000000000000000000000003e8
				00000000000000000000000000000000 000000000000000000000000000001f4
			"};

			let res = IncentivesPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		})
	}
}
