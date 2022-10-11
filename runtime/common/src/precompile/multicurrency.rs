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
use frame_support::{
	log,
	traits::{Currency, Get},
};
use module_currencies::WeightInfo;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::Erc20InfoMapping as Erc20InfoMappingT;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::MultiCurrency as MultiCurrencyT;
use primitives::{currency::DexShare, Balance, CurrencyId};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `MultiCurrency` impl precompile.
///
///
/// `input` data starts with `action` and `currency_id`.
///
/// Actions:
/// - Query total issuance.
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Transfer. Rest `input` bytes: `from`, `to`, `amount`.
pub struct MultiCurrencyPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryName = "name()",
	QuerySymbol = "symbol()",
	QueryDecimals = "decimals()",
	QueryTotalIssuance = "totalSupply()",
	QueryBalance = "balanceOf(address)",
	Transfer = "transfer(address,address,uint256)",
}

impl<Runtime> Precompile for MultiCurrencyPrecompile<Runtime>
where
	Runtime:
		module_currencies::Config + module_evm::Config + module_prices::Config + module_transaction_payment::Config,
	module_currencies::Pallet<Runtime>: MultiCurrencyT<Runtime::AccountId, CurrencyId = CurrencyId, Balance = Balance>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<
			Action,
			Runtime::AccountId,
			<Runtime as module_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>::new(input, target_gas_limit(target_gas));

		let currency_id =
			Runtime::Erc20InfoMapping::decode_evm_address(context.caller).ok_or_else(|| PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid currency id".into(),
				cost: target_gas_limit(target_gas).unwrap_or_default(),
			})?;

		let gas_cost = Pricer::<Runtime>::cost(&input, currency_id)?;

		if let Some(gas_limit) = target_gas {
			if gas_limit < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let action = input.action()?;

		log::debug!(target: "evm", "multicurrency: currency id: {:?}", currency_id);

		match action {
			Action::QueryName => {
				let name = Runtime::Erc20InfoMapping::name(currency_id).ok_or_else(|| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Get name failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;
				log::debug!(target: "evm", "multicurrency: name: {:?}", name);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes(&name),
					logs: Default::default(),
				})
			}
			Action::QuerySymbol => {
				let symbol =
					Runtime::Erc20InfoMapping::symbol(currency_id).ok_or_else(|| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Get symbol failed".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})?;
				log::debug!(target: "evm", "multicurrency: symbol: {:?}", symbol);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes(&symbol),
					logs: Default::default(),
				})
			}
			Action::QueryDecimals => {
				let decimals =
					Runtime::Erc20InfoMapping::decimals(currency_id).ok_or_else(|| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Get decimals failed".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})?;
				log::debug!(target: "evm", "multicurrency: decimals: {:?}", decimals);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(decimals),
					logs: Default::default(),
				})
			}
			Action::QueryTotalIssuance => {
				let total_issuance =
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_issuance(currency_id);
				log::debug!(target: "evm", "multicurrency: total issuance: {:?}", total_issuance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(total_issuance),
					logs: Default::default(),
				})
			}
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;
				let balance = if currency_id == <Runtime as module_transaction_payment::Config>::NativeCurrencyId::get()
				{
					<Runtime as module_evm::Config>::Currency::free_balance(&who)
				} else {
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_balance(currency_id, &who)
				};
				log::debug!(target: "evm", "multicurrency: who: {:?}, balance: {:?}", who, balance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(balance),
					logs: Default::default(),
				})
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;
				let amount = input.balance_at(3)?;
				log::debug!(target: "evm", "multicurrency: transfer from: {:?}, to: {:?}, amount: {:?}", from, to, amount);

				<module_currencies::Pallet<Runtime> as MultiCurrencyT<Runtime::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					amount,
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
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime:
		module_currencies::Config + module_evm::Config + module_prices::Config + module_transaction_payment::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<
			Action,
			Runtime::AccountId,
			<Runtime as module_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>,
		currency_id: CurrencyId,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		// Decode CurrencyId from EvmAddress
		let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

		let cost = match action {
			Action::QueryName | Action::QuerySymbol | Action::QueryDecimals => Self::erc20_info(currency_id),
			Action::QueryTotalIssuance => {
				// Currencies::TotalIssuance (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::QueryBalance => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				// Currencies::Balance (r: 1)
				cost.saturating_add(WeightToGas::convert(
					<Runtime as frame_system::Config>::DbWeight::get().reads(2),
				))
			}
			Action::Transfer => {
				let cost = InputPricer::<Runtime>::read_accounts(2);

				// transfer weight
				let weight = if currency_id == <Runtime as module_transaction_payment::Config>::NativeCurrencyId::get()
				{
					<Runtime as module_currencies::Config>::WeightInfo::transfer_native_currency()
				} else {
					<Runtime as module_currencies::Config>::WeightInfo::transfer_non_native_currency()
				};

				cost.saturating_add(WeightToGas::convert(weight))
			}
		};

		Ok(Self::BASE_COST.saturating_add(read_currency).saturating_add(cost))
	}

	fn dex_share_read_cost(share: DexShare) -> u64 {
		match share {
			DexShare::Erc20(_) | DexShare::ForeignAsset(_) => WeightToGas::convert(Runtime::DbWeight::get().reads(1)),
			_ => Self::BASE_COST,
		}
	}

	fn erc20_info(currency_id: CurrencyId) -> u64 {
		match currency_id {
			CurrencyId::Erc20(_) | CurrencyId::StableAssetPoolToken(_) | CurrencyId::ForeignAsset(_) => {
				WeightToGas::convert(Runtime::DbWeight::get().reads(1))
			}
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				Self::dex_share_read_cost(symbol_0).saturating_add(Self::dex_share_read_cost(symbol_1))
			}
			_ => Self::BASE_COST,
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		aca_evm_address, alice, ausd_evm_address, bob, erc20_address_not_exists, lp_aca_ausd_evm_address, new_test_ext,
		Balances, Test,
	};
	use frame_support::assert_noop;
	use hex_literal::hex;

	type MultiCurrencyPrecompile = crate::MultiCurrencyPrecompile<Test>;

	#[test]
	fn handles_invalid_currency_id() {
		new_test_ext().execute_with(|| {
			// call with not exists erc20
			let context = Context {
				address: Default::default(),
				caller: erc20_address_not_exists(),
				apparent_value: Default::default(),
			};

			// symbol() -> 0x95d89b41
			let input = hex! {"
				95d89b41
			"};

			assert_noop!(
				MultiCurrencyPrecompile::execute(&input, Some(10_000), &context, false),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid currency id".into(),
					cost: target_gas_limit(Some(10_000)).unwrap(),
				}
			);
		});
	}

	#[test]
	fn name_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// name() -> 0x06fdde03
			let input = hex! {"
				06fdde03
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000005
				4163616c61000000000000000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000017
				4c50204163616c61202d204163616c6120446f6c6c6172000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn symbol_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// symbol() -> 0x95d89b41
			let input = hex! {"
				95d89b41
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000003
				4143410000000000000000000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				000000000000000000000000000000000000000000000000000000000000000b
				4c505f4143415f41555344000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn decimals_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// decimals() -> 0x313ce567
			let input = hex! {"
				313ce567
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000000000000c
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn total_supply_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// totalSupply() -> 0x18160ddd
			let input = hex! {"
				18160ddd
			"};

			// Token
			context.caller = ausd_evm_address();

			// 2_000_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000077359400
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn balance_of_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// balanceOf(address) -> 0x70a08231
			// account
			let input = hex! {"
				70a08231
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// Token
			context.caller = aca_evm_address();

			// INITIAL_BALANCE = 1_000_000_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000e8d4a51000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		})
	}

	#[test]
	fn transfer_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// transfer(address,address,uint256) -> 0xbeabacc8
			// from
			// to
			// amount
			let input = hex! {"
				beabacc8
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000 00000000000000000000000000000001
			"};

			let from_balance = Balances::free_balance(alice());
			let to_balance = Balances::free_balance(bob());

			// Token
			context.caller = aca_evm_address();

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			assert_eq!(Balances::free_balance(alice()), from_balance - 1);
			assert_eq!(Balances::free_balance(bob()), to_balance + 1);

			// DexShare
			context.caller = lp_aca_ausd_evm_address();
			assert_noop!(
				MultiCurrencyPrecompile::execute(&input, Some(100_000), &context, false),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "BalanceTooLow".into(),
					cost: target_gas_limit(Some(100_000)).unwrap(),
				}
			);
		})
	}
}
