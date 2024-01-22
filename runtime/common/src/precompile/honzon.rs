// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use super::input::{Input, InputPricer, InputT, Output};
use crate::WeightToGas;
use frame_support::traits::Get;
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use module_honzon::WeightInfo;
use module_support::HonzonManager;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Amount, Balance, CurrencyId, Position};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The Honzon precomnpile
///
/// `input` data starts with `action`.
///
/// Actions:
///  - Adjust loan. `input` bytes: `who`, `currency_id`, `collateral_adjustment`,
///    `debit_adjustment`.
///  - Close loan by dex. `input` bytes: `who`, `currency_id`, `max_collateral_amount`.
///  - Get position. `input` bytes: `who`, `currency_id`.
///  - Get liquidation ratio. `input` bytes: `currency_id`.
///  - Get current collateral ratio. `input` bytes: `who`, `currency_id`.
pub struct HonzonPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	AdjustLoan = "adjustLoan(address,address,int128,int128)",
	CloseLoanByDex = "closeLoanByDex(address,address,uint256)",
	GetPosition = "getPosition(address,address)",
	GetCollateralParameters = "getCollateralParameters(address)",
	GetCurrentCollateralRatio = "getCurrentCollateralRatio(address,address)",
	GetDebitExchangeRate = "getDebitExchangeRate(address)",
}

impl<Runtime> Precompile for HonzonPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_honzon::Config + module_prices::Config,
	module_honzon::Pallet<Runtime>: HonzonManager<Runtime::AccountId, CurrencyId, Amount, Balance>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		match action {
			Action::AdjustLoan => {
				let who = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let collateral_adjustment = input.i128_at(3)?;
				let debit_adjustment = input.i128_at(4)?;

				log::debug!(
					target: "evm",
					"honzon: adjust_loan who: {:?}, currency_id: {:?}, collateral_adjustment: {:?}, debit_adjustment: {:?}",
					who, currency_id, collateral_adjustment, debit_adjustment
				);

				<module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::adjust_loan(&who, currency_id, collateral_adjustment, debit_adjustment).map_err(|e|
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Honzon AdjustLoan failed", e),
					}
				)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
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

				<module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::close_loan_by_dex(who, currency_id, max_collateral_amount).map_err(|e|
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Honzon CloseLoanByDex failed", e),
					}
				)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::GetPosition => {
				let who = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;

				let Position { collateral, debit } = <module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::get_position(&who, currency_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint_tuple(vec![collateral, debit]),
				})
			}
			Action::GetCollateralParameters => {
				let currency_id = input.currency_id_at(1)?;
				let params = <module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::get_collateral_parameters(currency_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint_array(params),
				})
			}
			Action::GetCurrentCollateralRatio => {
				let who = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let ratio = <module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::get_current_collateral_ratio(&who, currency_id)
				.unwrap_or_default();

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(ratio.into_inner()),
				})
			}
			Action::GetDebitExchangeRate => {
				let currency_id = input.currency_id_at(1)?;
				let exchange_rate = <module_honzon::Pallet<Runtime> as HonzonManager<
					Runtime::AccountId,
					CurrencyId,
					Amount,
					Balance,
				>>::get_debit_exchange_rate(currency_id);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(exchange_rate.into_inner()),
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

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);
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
			Action::GetPosition => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetCollateralParameters => {
				let currency_id = input.currency_id_at(1)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetCurrentCollateralRatio => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);
				let weight = <Runtime as module_honzon::Config>::WeightInfo::precompile_get_current_collateral_ratio();

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetDebitExchangeRate => {
				let currency_id = input.currency_id_at(1)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
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
		alice, alice_evm_addr, new_test_ext, CDPEngine, Currencies, DexModule, Honzon, Loans, One, RuntimeOrigin, Test,
		AUSD, BOB, DOT,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context};
	use module_support::{Rate, Ratio};
	use orml_traits::Change;
	use sp_runtime::FixedPointNumber;

	type HonzonPrecompile = super::HonzonPrecompile<Test>;

	#[test]
	fn adjust_loan_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(10000)
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// adjustLoan(address,address,int128,int128) => 0xd20a1c87
			// who
			// currency_id
			// collateral_adjustment
			// debit_adjustment
			let input = hex! {"
				d20a1c87
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000002
				00000000000000000000000000000000 00000000000000000000000010000000
				00000000000000000000000000000000 00000000000000000000000000001000
			"};

			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(Loans::positions(DOT, alice()).collateral, 268435456);
			assert_eq!(Loans::positions(DOT, alice()).debit, 4096)
		})
	}

	#[test]
	fn close_loan_by_dex_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice()),
				DOT,
				100_000_000_000,
				1_000_000
			));

			assert_ok!(DexModule::enable_trading_pair(
				RuntimeOrigin::signed(One::get()),
				DOT,
				AUSD
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				BOB,
				AUSD,
				1_000_000_000_000
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				BOB,
				DOT,
				1_000_000_000_000
			));
			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(BOB),
				DOT,
				AUSD,
				1_000_000_000,
				1_000_000_000,
				0,
				false
			));

			assert_eq!(Loans::positions(DOT, alice()).debit, 1_000_000);
			assert_eq!(Loans::positions(DOT, alice()).collateral, 100_000_000_000);

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// closeLoanByDex(address,address,uint256) => 0xbf0ea731
			// who
			// currency_id
			// max_collateral_amount
			let input = hex! {"
				bf0ea731
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000002
				00000000000000000000000000000000 00000000000000000000000100000000
			"};

			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);

			assert_eq!(Loans::positions(DOT, alice()).debit, 0);
			assert_eq!(Loans::positions(DOT, alice()).collateral, 0);
		});
	}

	#[test]
	fn get_position_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice()),
				DOT,
				100_000_000_000,
				1_000_000
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getPosition(address,address) => 0xb33dc190
			// who
			// currency_id
			let input = hex! {"
				b33dc190
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			// 100_000_000_000
			// 1_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000174876e800
				00000000000000000000000000000000 000000000000000000000000000f4240
			"};
			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_collateral_parameters_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getCollateralParameters(address) => 0xe8b96662
			// currency_id
			let input = hex! {"
				e8b96662
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			// offset to where array starts (32 bytes)
			// Number of elements encoded in array
			// `maximum_total_debit_value`: 1_000_000_000
			// `interest_rate_per_sec`: `FixedU128` for 1/10_000
			// `liquidation_ratio`: `FixedU128` for 3/2
			// `liquidation_penalty`: `FixedU128` for 2/10
			// `required_collateral_ratio`: `FixedU128` for 9/5
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000020
				00000000000000000000000000000000 00000000000000000000000000000005
				00000000000000000000000000000000 0000000000000000000000003b9aca00
				00000000000000000000000000000000 0000000000000000000009184e72a000
				00000000000000000000000000000000 000000000000000014d1120d7b160000
				00000000000000000000000000000000 000000000000000002c68af0bb140000
				00000000000000000000000000000000 000000000000000018fae27693b40000
			"};

			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_current_collateral_ratio_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				DOT,
				1_000_000_000_000
			));
			assert_ok!(Honzon::adjust_loan(
				RuntimeOrigin::signed(alice()),
				DOT,
				100_000_000_000,
				1_000_000
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getCurrentCollateralRatio(address,address) => 0x1384ed17
			// who
			// currency_id
			let input = hex! {"
				1384ed17
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			// value for FixedU128 of 100_000
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000152d02c7e14af6800000
			"};
			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_debit_exchange_rate_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(CDPEngine::set_collateral_params(
				RuntimeOrigin::signed(One::get()),
				DOT,
				Change::NewValue(Some(Rate::saturating_from_rational(1, 100000))),
				Change::NewValue(Some(Ratio::saturating_from_rational(3, 2))),
				Change::NewValue(Some(Rate::saturating_from_rational(2, 10))),
				Change::NewValue(Some(Ratio::saturating_from_rational(9, 5))),
				Change::NewValue(1_000_000_000)
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getDebitExchangeRate(address) => 0xd018f091
			// currency_id
			let input = hex! {"
				d018f091
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			// value for FixedU128 of 1, default value for exchange rate
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000de0b6b3a7640000
			"};
			let res = HonzonPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		})
	}
}
