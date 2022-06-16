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
use frame_support::{log, sp_runtime::FixedPointNumber};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitSucceed,
};
use module_support::{Erc20InfoMapping as Erc20InfoMappingT, PriceProvider as PriceProviderT};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `Oracle` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get price. Rest `input` bytes: `currency_id`.
pub struct OraclePrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetPrice = "getPrice(address)",
}

impl<Runtime> Precompile for OraclePrecompile<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config,
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
			Action::GetPrice => {
				let currency_id = input.currency_id_at(1)?;
				let mut price =
					<module_prices::RealTimePriceProvider<Runtime>>::get_price(currency_id).unwrap_or_default();

				let maybe_decimals = Runtime::Erc20InfoMapping::decimals(currency_id);
				let decimals = match maybe_decimals {
					Some(decimals) => decimals,
					None => {
						// If the option is none, let price = 0 to return 0.
						// Solidity should handle the situation of price 0.
						price = Default::default();
						Default::default()
					}
				};

				let maybe_adjustment_multiplier = 10u128.checked_pow((18 - decimals).into());
				let adjustment_multiplier = match maybe_adjustment_multiplier {
					Some(adjustment_multiplier) => adjustment_multiplier,
					None => {
						// If the option is none, let price = 0 to return 0.
						// Solidity should handle the situation of price 0.
						price = Default::default();
						Default::default()
					}
				};

				let output = price.into_inner().wrapping_div(adjustment_multiplier);

				log::debug!(target: "evm", "oracle: getPrice currency_id: {:?}, price: {:?}, adjustment_multiplier: {:?}, output: {:?}", currency_id, price, adjustment_multiplier, output);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(output),
					logs: Default::default(),
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
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost = match action {
			Action::GetPrice => {
				let currency_id = input.currency_id_at(1)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);
				let get_price = WeightToGas::convert(PrecompileWeights::<Runtime>::oracle_get_price());
				WeightToGas::convert(read_currency).saturating_add(get_price)
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{alice_evm_addr, new_test_ext, Oracle, Price, Test, ALICE, RENBTC};
	use frame_support::{assert_noop, assert_ok};
	use hex_literal::hex;
	use module_evm::ExitRevert;
	use orml_traits::DataFeeder;

	type OraclePrecompile = crate::OraclePrecompile<Test>;

	#[test]
	fn get_price_work() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			let price = Price::from(30_000);

			// getPrice(address) -> 0x41976e09
			// RENBTC
			let input = hex! {"
				41976e09
				000000000000000000000000 0000000000000000000100000000000000000014
			"};

			// no price yet
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = OraclePrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			assert_ok!(Oracle::feed_value(ALICE, RENBTC, price));
			assert_eq!(
				Oracle::get(&RENBTC),
				Some(orml_oracle::TimestampedValue {
					value: price,
					timestamp: 1
				})
			);

			// returned price
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000065a4da25d3016c00000
			"};

			let resp = OraclePrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn oracle_precompile_should_handle_invalid_input() {
		new_test_ext().execute_with(|| {
			assert_noop!(
				OraclePrecompile::execute(
					&[0u8; 0],
					Some(1000),
					&Context {
						address: Default::default(),
						caller: alice_evm_addr(),
						apparent_value: Default::default()
					},
					false
				),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid input".into(),
					cost: target_gas_limit(Some(1000)).unwrap(),
				}
			);

			assert_noop!(
				OraclePrecompile::execute(
					&[0u8; 3],
					Some(1000),
					&Context {
						address: Default::default(),
						caller: alice_evm_addr(),
						apparent_value: Default::default()
					},
					false
				),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid input".into(),
					cost: target_gas_limit(Some(1000)).unwrap(),
				}
			);

			assert_noop!(
				OraclePrecompile::execute(
					&[1u8; 32],
					Some(1000),
					&Context {
						address: Default::default(),
						caller: alice_evm_addr(),
						apparent_value: Default::default()
					},
					false
				),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid action".into(),
					cost: target_gas_limit(Some(1000)).unwrap(),
				}
			);
		});
	}
}
