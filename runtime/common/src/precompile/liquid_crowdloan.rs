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
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use module_liquid_crowdloan::WeightInfo;
use module_support::Erc20InfoMapping as _;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_core::Get;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `LiquidCrowdloan` impl precompile.
pub struct LiquidCrowdloanPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Redeem = "redeem(address,uint256)",
	GetRedeemCurrency = "getRedeemCurrency()",
}

impl<Runtime> Precompile for LiquidCrowdloanPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_liquid_crowdloan::Config,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		match action {
			Action::Redeem => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				let redeem_amount =
					<module_liquid_crowdloan::Pallet<Runtime>>::do_redeem(&who, amount).map_err(|e| {
						PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: Output::encode_error_msg("LiquidCrowdloan redeem failed", e),
						}
					})?;

				log::debug!(target: "evm", "liuqid_crowdloan: Redeem who: {:?}, amount: {:?}, output: {:?}", who, amount, redeem_amount);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(redeem_amount),
				})
			}
			Action::GetRedeemCurrency => {
				let currency_id = <module_liquid_crowdloan::Pallet<Runtime>>::redeem_currency();
				let address = <Runtime as module_prices::Config>::Erc20InfoMapping::encode_evm_address(currency_id)
					.unwrap_or_default();

				log::debug!(target: "evm", "liuqid_crowdloan: GetRedeemCurrency output: {:?}", address);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_address(address),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_liquid_crowdloan::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);
		let action = input.action()?;

		let cost = match action {
			Action::Redeem => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_liquid_crowdloan::Config>::WeightInfo::redeem();

				read_account.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetRedeemCurrency => {
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);
				WeightToGas::convert(weight)
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompile::mock::{
		bob, bob_evm_addr, new_test_ext, Currencies, LiquidCrowdloan, LiquidCrowdloanPalletId, RuntimeOrigin, Test,
		DOT, LCDOT, LDOT,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context};
	use orml_traits::MultiCurrency;
	use sp_runtime::traits::AccountIdConversion;

	type LiquidCrowdloanPrecompile = crate::precompile::LiquidCrowdloanPrecompile<Test>;

	#[test]
	fn redeem_dot() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: bob_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				bob(),
				LCDOT,
				1_000_000_000
			));

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				LiquidCrowdloanPalletId::get().into_account_truncating(),
				DOT,
				1_000_000_000
			));

			// redeem(address,uint256) -> 1e9a6950
			// who
			// amount 1e9
			let input = hex! {"
				1e9a6950
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000 0000000000000000000000003b9aca00
			"};

			// 1e9
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000003b9aca00
			"};

			let res = LiquidCrowdloanPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false))
				.unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());

			assert_eq!(Currencies::free_balance(DOT, &bob()), 1_000_000_000);
			assert_eq!(Currencies::free_balance(LCDOT, &bob()), 0);
		});
	}

	#[test]
	fn redeem_ldot() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: bob_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				bob(),
				LCDOT,
				1_000_000_000
			));

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				LiquidCrowdloanPalletId::get().into_account_truncating(),
				LDOT,
				11_000_000_000
			));

			assert_ok!(LiquidCrowdloan::set_redeem_currency_id(RuntimeOrigin::root(), LDOT));

			// redeem(address,uint256) -> 1e9a6950
			// who
			// amount 1e9
			let input = hex! {"
				1e9a6950
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000 0000000000000000000000003b9aca00
			"};

			// 11e9
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000028fa6ae00
			"};

			let res = LiquidCrowdloanPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false))
				.unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());

			assert_eq!(Currencies::free_balance(LDOT, &bob()), 11_000_000_000);
			assert_eq!(Currencies::free_balance(LCDOT, &bob()), 0);
		});
	}

	#[test]
	fn redeem_currency() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: bob_evm_addr(),
				apparent_value: Default::default(),
			};

			// getRedeemCurrency() -> 785ad4c3
			let input = hex!("785ad4c3");

			// DOT
			let expected_output = hex! {"
				000000000000000000000000 0000000000000000000100000000000000000002
			"};

			let res = LiquidCrowdloanPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false))
				.unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());

			assert_ok!(LiquidCrowdloan::set_redeem_currency_id(RuntimeOrigin::root(), LDOT));

			// LDOT
			let expected_output = hex! {"
				000000000000000000000000 0000000000000000000100000000000000000003
			"};

			let res = LiquidCrowdloanPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false))
				.unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output.to_vec());
		});
	}
}
