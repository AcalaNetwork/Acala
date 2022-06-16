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
use frame_support::{log, traits::Get};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::HomaManager;

use module_homa::WeightInfo;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::Balance;
use sp_runtime::{traits::Convert, FixedPointNumber, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The Homa precompile
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Mint. Rest `input` bytes: `who`, `amount`.
/// - Request redeem. Rest `input` bytes: `who`, `amount`, `fast_match`.
/// - Get exchange rate.
/// - Get estimated reward rate.
/// - Get commission rate.
/// - Get fast match fee.

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
				let fast_match = input.bool_at(3)?;

				log::debug!(
					target: "evm",
					"homa: request_redeem, who: {:?}, amount: {:?}, fast_match: {:?}",
					&who, amount, fast_match
				);

				<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::request_redeem(
					who, amount, fast_match,
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
					output: Output::encode_uint(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetEstimatedRewardRate => {
				let rate = <module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_estimated_reward_rate();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetCommissionRate => {
				let rate =
					<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_commission_rate();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(rate.into_inner()),
					logs: Default::default(),
				})
			}
			Action::GetFastMatchFee => {
				let rate =
					<module_homa::Pallet<Runtime> as HomaManager<Runtime::AccountId, Balance>>::get_fast_match_fee();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(rate.into_inner()),
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
			Action::Mint => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_homa::Config>::WeightInfo::mint();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::RequestRedeem => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_homa::Config>::WeightInfo::request_redeem();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetExchangeRate => {
				// Homa::TotalVoidLiquid (r: 1)
				// Homa::ToBondPool (r: 1)
				// Tokens::TotalIssuance(r: 1)
				// Homa::TotalStakingBonded(r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(4))
			}
			Action::GetEstimatedRewardRate => {
				// Homa::EstimatedRewardRatePerEra (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::GetCommissionRate => {
				// Homa::CommissionRate (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::GetFastMatchFee => {
				// Homa::FastMatchFeeRate (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompile::mock::{
		alice, alice_evm_addr, new_test_ext, Currencies, Homa, HomaAdmin, Origin, StakingCurrencyId, Test, ACA,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use sp_runtime::FixedU128;

	type HomaPrecompile = super::HomaPrecompile<Test>;

	#[test]
	fn mint_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Homa::update_homa_params(
				Origin::signed(HomaAdmin::get()),
				Some(1_000_000_000_000),
				Some(FixedU128::saturating_from_rational(1, 10)),
				Some(FixedU128::saturating_from_rational(1, 10)),
				Some(FixedU128::saturating_from_rational(1, 10)),
			));

			assert_ok!(Currencies::update_balance(Origin::root(), alice(), ACA, 1_000_000_000));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				alice(),
				StakingCurrencyId::get(),
				1_000_000_000_000
			));

			// mint(address,uint256) -> 0x40c10f19
			// who
			// amount
			let input = hex! {"
				40c10f19
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 0000000000000000000000003b9aca00
			"};

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
		});
	}

	#[test]
	fn request_redeem_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(Homa::update_homa_params(
				Origin::signed(HomaAdmin::get()),
				Some(1_000_000_000_000),
				Some(FixedU128::saturating_from_rational(1, 10)),
				Some(FixedU128::saturating_from_rational(1, 10)),
				Some(FixedU128::saturating_from_rational(1, 10)),
			));

			assert_ok!(Currencies::update_balance(Origin::root(), alice(), ACA, 1_000_000_000));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				alice(),
				StakingCurrencyId::get(),
				1_000_000_000_000
			));

			assert_ok!(Homa::mint(Origin::signed(alice()), 1_000_000_000));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// requestRedeem(address,uint256,bool) => 0xc941744a
			// who
			// amount
			// fast_match
			let input = hex! {"
				c941744a
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000aca00
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
		});
	}

	#[test]
	fn get_exchange_rate_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getExchangeRate() -> 0xe6aa216c
			let input = hex! {
				"e6aa216c"
			};

			// encoded value of FixedU128::saturating_from_rational(1,10);
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000016345785d8a0000"}.to_vec();

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_estimated_reward_rate_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Homa::update_homa_params(
				Origin::signed(HomaAdmin::get()),
				None,
				Some(FixedU128::saturating_from_rational(1, 10)),
				None,
				None,
			));

			// getEstimatedRewardRate() -> 0xd313f77e
			let input = hex! {
				"d313f77e"
			};

			// encoded value of FixedU128::saturating_from_rational(1,10);
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000016345785d8a0000"}.to_vec();

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_commission_rate() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Homa::update_homa_params(
				Origin::signed(HomaAdmin::get()),
				None,
				None,
				Some(FixedU128::saturating_from_rational(1, 10)),
				None,
			));

			// getCommissionRate() => 0x3e4eb36c
			let input = hex! {"3e4eb36c"};

			// encoded value of FixedU128::saturating_from_rational(1,10);
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000016345785d8a0000"}.to_vec();

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_fast_match_fee_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Homa::update_homa_params(
				Origin::signed(HomaAdmin::get()),
				None,
				None,
				None,
				Some(FixedU128::saturating_from_rational(1, 10)),
			));

			// getFastMatchFee() => 0xc18290dd
			let input = hex! {"c18290dd"};

			// encoded value of FixedU128::saturating_from_rational(1,10);
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000016345785d8a0000"}.to_vec();

			let res = HomaPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}
}
