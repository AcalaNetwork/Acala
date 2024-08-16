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
use module_support::EarningManager;

use ethabi::Token;
use frame_system::pallet_prelude::BlockNumberFor;
use module_earning::{BondingLedgerOf, WeightInfo};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::Balance;
use sp_core::U256;
use sp_runtime::{
	traits::{Convert, Zero},
	Permill, RuntimeDebug,
};
use sp_std::{marker::PhantomData, prelude::*};

/// The Earning precompile
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Bond. `input` bytes: `who`.
/// - Unbond. `input` bytes: `who`.
/// - UnbondInstant. `input` bytes: `who`.
/// - Rebond. `input` bytes: `who`.
/// - Withdraw unbonded. `input` bytes: `who`.
/// - Get bonding ledger. `input` bytes: `who`.
/// - Get minimum bond amount.
/// - Get unbonding period.
/// - Get maximum unbonding chunks amount.

pub struct EarningPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Bond = "bond(address,uint256)",
	Unbond = "unbond(address,uint256)",
	UnbondInstant = "unbondInstant(address,uint256)",
	Rebond = "rebond(address,uint256)",
	WithdrawUnbonded = "withdrawUnbonded(address)",
	GetBondingLedger = "getBondingLedger(address)",
	GetInstantUnstakeFee = "getInstantUnstakeFee()",
	GetMinBond = "getMinBond()",
	GetUnbondingPeriod = "getUnbondingPeriod()",
	GetMaxUnbondingChunks = "getMaxUnbondingChunks()",
}

impl<Runtime> Precompile for EarningPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_earning::Config + module_prices::Config,
	module_earning::Pallet<Runtime>: EarningManager<
		Runtime::AccountId,
		Balance,
		BondingLedgerOf<Runtime>,
		FeeRatio = Permill,
		Moment = BlockNumberFor<Runtime>,
	>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<
			Action,
			Runtime::AccountId,
			<Runtime as module_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>::new(handle.input());

		let action = input.action()?;

		match action {
			Action::Bond => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"earning: bond, who: {:?}, amount: {:?}",
					&who, amount
				);

				let bonded_amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::bond(who, amount)
					.map_err(|e| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Earning bond failed", e),
					})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(bonded_amount),
				})
			}
			Action::Unbond => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"earning: unbond, who: {:?}, amount: {:?}",
					&who, amount
				);

				let unbonded_amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::unbond(who, amount)
					.map_err(|e| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Earning unbond failed", e),
					})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(unbonded_amount),
				})
			}
			Action::UnbondInstant => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"earning: unbond_instant, who: {:?}, amount: {:?}",
					&who, amount
				);

				let unbonded_amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::unbond_instant(
					who, amount,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("Earning unbond instantly failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(unbonded_amount),
				})
			}
			Action::Rebond => {
				let who = input.account_id_at(1)?;
				let amount = input.balance_at(2)?;

				log::debug!(
					target: "evm",
					"earning: rebond, who: {:?}, amount: {:?}",
					&who, amount
				);

				let rebonded_amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::rebond(who, amount)
					.map_err(|e| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Earning rebond failed", e),
					})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(rebonded_amount),
				})
			}
			Action::WithdrawUnbonded => {
				let who = input.account_id_at(1)?;

				log::debug!(
					target: "evm",
					"earning: withdraw_unbonded, who: {:?}",
					&who
				);

				let withdrawed_amount =
					<module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::withdraw_unbonded(who).map_err(
						|e| PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: Output::encode_error_msg("Earning withdraw unbonded failed", e),
						},
					)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(withdrawed_amount),
				})
			}
			Action::GetBondingLedger => {
				let who = input.account_id_at(1)?;
				let ledger = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::get_bonding_ledger(who);
				let unlocking_token: Vec<Token> = ledger
					.unlocking()
					.iter()
					.cloned()
					.map(|(value, unlock_at)| {
						Token::Tuple(vec![
							Token::Uint(Into::<U256>::into(value)),
							Token::Uint(Into::<U256>::into(unlock_at)),
						])
					})
					.collect();
				let ledger_token: Token = Token::Tuple(vec![
					Token::Uint(Into::<U256>::into(ledger.total())),
					Token::Uint(Into::<U256>::into(ledger.active())),
					Token::Array(unlocking_token),
				]);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: ethabi::encode(&[ledger_token]),
				})
			}
			Action::GetInstantUnstakeFee => {
				let (ratio, accuracy) = if let Some(ratio) =
					<module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::get_instant_unstake_fee()
				{
					(ratio.deconstruct(), Permill::one().deconstruct())
				} else {
					(Zero::zero(), Zero::zero())
				};

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint_tuple(vec![ratio, accuracy]),
				})
			}
			Action::GetMinBond => {
				let amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::get_min_bond();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(amount),
				})
			}
			Action::GetUnbondingPeriod => {
				let period = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::get_unbonding_period();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(Into::<U256>::into(period)),
				})
			}
			Action::GetMaxUnbondingChunks => {
				let amount = <module_earning::Pallet<Runtime> as EarningManager<_, _, _>>::get_max_unbonding_chunks();
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(amount),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_earning::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);
		let action = input.action()?;

		let cost: u64 = match action {
			Action::Bond => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_earning::Config>::WeightInfo::bond();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::Unbond => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_earning::Config>::WeightInfo::unbond();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::UnbondInstant => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_earning::Config>::WeightInfo::unbond_instant();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::Rebond => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_earning::Config>::WeightInfo::rebond();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::WithdrawUnbonded => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				let weight = <Runtime as module_earning::Config>::WeightInfo::withdraw_unbonded();

				cost.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetBondingLedger => {
				// Earning::Leger (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::GetInstantUnstakeFee => {
				// Runtime Config
				Default::default()
			}
			Action::GetMinBond => {
				// Runtime Config
				Default::default()
			}
			Action::GetUnbondingPeriod => {
				// Runtime Config
				Default::default()
			}
			Action::GetMaxUnbondingChunks => {
				// Runtime Config
				Default::default()
			}
		};
		Ok(Self::BASE_COST.saturating_add(cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompile::mock::{
		alice, alice_evm_addr, new_test_ext, Currencies, Earning, RuntimeOrigin, System, Test, UnbondingPeriod, ACA,
	};
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context};
	use orml_traits::MultiCurrency;

	type EarningPrecompile = super::EarningPrecompile<Test>;

	#[test]
	fn bond_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));

			assert_eq!(Currencies::free_balance(ACA, &alice()), 100_000_000_000_000);

			// bond(address,uint256) -> 0xa515366a
			// who 0x1000000000000000000000000000000000000001
			// amount 20_000_000_000_000
			let input = hex! {"
                a515366a
                000000000000000000000000 1000000000000000000000000000000000000001
                00000000000000000000000000000000 0000000000000000000012309ce54000
            "};

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 20_000_000_000_000);

			// encoded value of 20_000_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000012309ce54000"}.to_vec();
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn unbond_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));
			assert_ok!(Earning::bond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 20_000_000_000_000);

			// unbond(address,uint256) -> 0xa5d059ca
			// who 0x1000000000000000000000000000000000000001
			// amount 20_000_000_000_000
			let input = hex! {"
                a5d059ca
                000000000000000000000000 1000000000000000000000000000000000000001
                00000000000000000000000000000000 0000000000000000000012309ce54000
            "};

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 0);

			// encoded value of 20_000_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000012309ce54000"}.to_vec();
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn unbond_instant_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));
			assert_ok!(Earning::bond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 20_000_000_000_000);

			// unbondInstant(address,uint256) -> 0xd15a4d60
			// who 0x1000000000000000000000000000000000000001
			// amount 20_000_000_000_000
			let input = hex! {"
                d15a4d60
                000000000000000000000000 1000000000000000000000000000000000000001
                00000000000000000000000000000000 0000000000000000000012309ce54000
            "};

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 0);

			// encoded value of 20_000_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000012309ce54000"}.to_vec();
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn rebond_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));
			assert_ok!(Earning::bond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_ok!(Earning::unbond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_eq!(Earning::ledger(&alice()).unwrap().total(), 20_000_000_000_000);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 0);

			// rebond(address,uint256) -> 0x92d1b784
			// who 0x1000000000000000000000000000000000000001
			// amount 20_000_000_000_000
			let input = hex! {"
                92d1b784
                000000000000000000000000 1000000000000000000000000000000000000001
                00000000000000000000000000000000 0000000000000000000012309ce54000
            "};

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();

			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(Earning::ledger(&alice()).unwrap().total(), 20_000_000_000_000);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 20_000_000_000_000);

			// encoded value of 20_000_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000012309ce54000"}.to_vec();
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn withdraw_unbonded_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));
			assert_ok!(Earning::bond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_ok!(Earning::unbond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));
			assert_eq!(Earning::ledger(&alice()).unwrap().total(), 20_000_000_000_000);
			assert_eq!(Earning::ledger(&alice()).unwrap().active(), 0);

			System::set_block_number(1 + 2 * UnbondingPeriod::get());

			// withdrawUnbonded(address) -> 0xaeffaa47
			// who 0x1000000000000000000000000000000000000001
			let input = hex! {"
                aeffaa47
                000000000000000000000000 1000000000000000000000000000000000000001
            "};

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);

			// encoded value of 20_000_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000012309ce54000"}.to_vec();
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_min_bond_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getMinBond() -> 0x5990dc2b
			let input = hex! {
				"5990dc2b"
			};

			// encoded value of 1_000_000_000;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000000003b9aca00"}.to_vec();

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_instant_unstake_fee_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getInstantUnstakeFee() -> 0xc3e07c04
			let input = hex! {
				"c3e07c04"
			};

			// encoded value of Permill::from_percent(10);
			let expected_output = hex! {"
                00000000000000000000000000000000 000000000000000000000000000186a0
                00000000000000000000000000000000 000000000000000000000000000f4240
            "}
			.to_vec();

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_unbonding_period_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getUnbondingPeriod() -> 0x6fd2c80b
			let input = hex! {
				"6fd2c80b"
			};

			// encoded value of 10_000;
			let expected_output = hex! {"00000000000000000000000000000000 00000000000000000000000000002710"}.to_vec();

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_max_unbonding_chunks_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getMaxUnbondingChunks() -> 0x09bfc8a1
			let input = hex! {
				"09bfc8a1"
			};

			// encoded value of 10;
			let expected_output = hex! {"00000000000000000000000000000000 0000000000000000000000000000000a"}.to_vec();

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}

	#[test]
	fn get_bonding_ledger_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				alice(),
				ACA,
				99_000_000_000_000
			));
			assert_ok!(Earning::bond(RuntimeOrigin::signed(alice()), 20_000_000_000_000));

			// getBondingLedger(address) -> 0x361592d7
			// who 0x1000000000000000000000000000000000000001
			let input = hex! {"
                361592d7
                000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// encoded value of ledger of alice;
			let expected_output = hex! {"
                0000000000000000000000000000000000000000000000000000000000000020
                000000000000000000000000000000000000000000000000000012309ce54000
                000000000000000000000000000000000000000000000000000012309ce54000
                0000000000000000000000000000000000000000000000000000000000000060
                0000000000000000000000000000000000000000000000000000000000000000
            "}
			.to_vec();

			let res =
				EarningPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(res.exit_status, ExitSucceed::Returned);
			assert_eq!(res.output, expected_output);
		});
	}
}
