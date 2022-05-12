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
use frame_support::log;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_honzon::WeightInfo;
use module_support::HonzonManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

pub struct HonzonPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	AdjustLoan = "adjustLoan(address,address,int256,int256)",
	CloseLoanByDex = "closeLoanByDex(address,address,uint256)",
}

impl<Runtime> Precompile for HonzonPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_honzon::Config + module_prices::Config,
	module_honzon::Pallet<Runtime>: HonzonManager<Runtime::AccountId, CurrencyId, Amount, Balance>,
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
			Action::AdjustLoan => {
				let who = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let collateral_adjustment = 0;
				let debit_adjustment = 0;

				log::debug!(
					target: "evm",
					"honzon: adjust_loan who: {:?}, currency_id: {:?}, collateral_adjustment: {:?}, debit_adjustment: {:?}",
					who, currency_id, collateral_adjustment, debit_adjustment
				);

				<module_honzon::Pallet<Runtime> as HonzonManager<Runtime::AccountId, CurrencyId, Amount, Balance>>::adjust_loan(&who, currency_id, collateral_adjustment, debit_adjustment).map_err(|e|
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: Into::<&str>::into(e).as_bytes().to_vec(),
                        cost: target_gas_limit(target_gas).unwrap_or_default(),
                    }
                )?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
			Action::CloseLoanByDex => {
				let who = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let max_collateral_amount = input.balance_at(3)?;

				log::debug!(
					target: "evm",
					"honzon: close_loan_by_dex who: {:?}, currency_id: {:?}, max_collateral_adjustment: {:?}",
					who, currency_id, max_collateral_amount
				);

				<module_honzon::Pallet<Runtime> as HonzonManager<Runtime::AccountId, CurrencyId, Amount, Balance>>::close_loan_by_dex(who, currency_id, max_collateral_amount).map_err(|e|
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: Into::<&str>::into(e).as_bytes().to_vec(),
                        cost: target_gas_limit(target_gas).unwrap_or_default(),
                    }
                )?;

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
	Runtime: module_evm::Config + module_honzon::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::AdjustLoan => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

				let weight = <Runtime as module_honzon::Config>::WeightInfo::adjust_loan();

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::CloseLoanByDex => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

				let weight = <Runtime as module_honzon::Config>::WeightInfo::close_loan_has_debit_by_dex();

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		alice, alice_evm_addr, new_test_ext, CDPEngine, Currencies, Honzon, One, Origin, Test, DOT,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_support::{Rate, Ratio};
	use orml_traits::Change;
	use sp_runtime::FixedPointNumber;

	type HonzonPrecompile = super::HonzonPrecompile<Test>;

	#[test]
	fn adjust_loan_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				Origin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10000)
			));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let input = hex! {"
                f4f31ede
            "};

			let res = HonzonPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
		})
	}

	#[test]
	fn close_loan_by_dex_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				Origin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));
			assert_ok!(Honzon::adjust_loan(
				Origin::signed(alice()),
				DOT,
				100_000_000_000,
				1_000_000
			));
		});
	}
}
