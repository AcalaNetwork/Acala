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
use module_dex::WeightInfo;
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use module_support::{DEXManager, SwapLimit};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

/// The `DEX` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get liquidity. Rest `input` bytes: `currency_id_a`, `currency_id_b`.
/// - Swap with exact supply. Rest `input` bytes: `who`, `currency_id_a`, `currency_id_b`,
///   `supply_amount`, `min_target_amount`.
pub struct DEXPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetLiquidityPool = "getLiquidityPool(address,address)",
	GetLiquidityTokenAddress = "getLiquidityTokenAddress(address,address)",
	GetSwapTargetAmount = "getSwapTargetAmount(address[],uint256)",
	GetSwapSupplyAmount = "getSwapSupplyAmount(address[],uint256)",
	SwapWithExactSupply = "swapWithExactSupply(address,address[],uint256,uint256)",
	SwapWithExactTarget = "swapWithExactTarget(address,address[],uint256,uint256)",
	AddLiquidity = "addLiquidity(address,address,address,uint256,uint256,uint256)",
	RemoveLiquidity = "removeLiquidity(address,address,address,uint256,uint256,uint256)",
}

impl<Runtime> Precompile for DEXPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_dex::Config + module_prices::Config,
	module_dex::Pallet<Runtime>: DEXManager<Runtime::AccountId, Balance, CurrencyId>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<
			Action,
			Runtime::AccountId,
			Runtime::AddressMapping,
			<Runtime as module_dex::Config>::Erc20InfoMapping,
		>::new(handle.input());

		let action = input.action()?;

		match action {
			Action::GetLiquidityPool => {
				let currency_id_a = input.currency_id_at(1)?;
				let currency_id_b = input.currency_id_at(2)?;
				log::debug!(
					target: "evm",
					"dex: get_liquidity_pool currency_id_a: {:?}, currency_id_b: {:?}",
					currency_id_a, currency_id_b
				);

				let (balance_a, balance_b) = <module_dex::Pallet<Runtime> as DEXManager<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::get_liquidity_pool(currency_id_a, currency_id_b);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint_tuple(vec![balance_a, balance_b]),
				})
			}
			Action::GetLiquidityTokenAddress => {
				let currency_id_a = input.currency_id_at(1)?;
				let currency_id_b = input.currency_id_at(2)?;
				log::debug!(
					target: "evm",
					"dex: get_liquidity_token address currency_id_a: {:?}, currency_id_b: {:?}",
					currency_id_a, currency_id_b
				);

				// If it does not exist, return address(0x0). Keep the behavior the same as mapping[key]
				let address = <module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::get_liquidity_token_address(currency_id_a, currency_id_b)
					.unwrap_or_default();

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_address(address),
				})
			}
			Action::GetSwapTargetAmount => {
				// solidity abi encode array will add an offset at input[1]
				let supply_amount = input.balance_at(2)?;
				let path_len = input.u32_at(3)?;
				let mut path = vec![];
				for i in 0..path_len {
					path.push(input.currency_id_at((4 + i) as usize)?);
				}
				log::debug!(
					target: "evm",
					"dex: get_swap_target_amount path: {:?}, supply_amount: {:?}",
					path, supply_amount
				);

				// If get_swap_amount fail, return 0.
				let target = <module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::get_swap_amount(&path, SwapLimit::ExactSupply(supply_amount, Balance::MIN))
					.map(|(_, target)| target)
					.unwrap_or_default();

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(target),
				})
			}
			Action::GetSwapSupplyAmount => {
				// solidity abi encode array will add an offset at input[1]
				let target_amount = input.balance_at(2)?;
				let path_len = input.u32_at(3)?;
				let mut path = vec![];
				for i in 0..path_len {
					path.push(input.currency_id_at((4 + i) as usize)?);
				}
				log::debug!(
					target: "evm",
					"dex: get_swap_supply_amount path: {:?}, target_amount: {:?}",
					path, target_amount
				);

				// If get_swap_amount fail, return 0.
				let supply = <module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::get_swap_amount(&path, SwapLimit::ExactTarget(Balance::MAX, target_amount))
					.map(|(supply, _)| supply)
					.unwrap_or_default();

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(supply),
				})
			}
			Action::SwapWithExactSupply => {
				let who = input.account_id_at(1)?;
				// solidity abi encode array will add an offset at input[2]
				let supply_amount = input.balance_at(3)?;
				let min_target_amount = input.balance_at(4)?;
				let path_len = input.u32_at(5)?;
				let mut path = vec![];
				for i in 0..path_len {
					path.push(input.currency_id_at((6 + i) as usize)?);
				}
				log::debug!(
					target: "evm",
					"dex: swap_with_exact_supply who: {:?}, path: {:?}, supply_amount: {:?}, min_target_amount: {:?}",
					who, path, supply_amount, min_target_amount
				);

				let (_, value) =
					<module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::swap_with_specific_path(&who, &path, SwapLimit::ExactSupply(supply_amount, min_target_amount))
					.map_err(|e|
						PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: Into::<&str>::into(e).as_bytes().to_vec(),
						}
					)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(value),
				})
			}
			Action::SwapWithExactTarget => {
				let who = input.account_id_at(1)?;
				// solidity abi encode array will add an offset at input[2]
				let target_amount = input.balance_at(3)?;
				let max_supply_amount = input.balance_at(4)?;
				let path_len = input.u32_at(5)?;
				let mut path = vec![];
				for i in 0..path_len {
					path.push(input.currency_id_at((6 + i) as usize)?);
				}
				log::debug!(
					target: "evm",
					"dex: swap_with_exact_target who: {:?}, path: {:?}, target_amount: {:?}, max_supply_amount: {:?}",
					who, path, target_amount, max_supply_amount
				);

				let (value, _) =
					<module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::swap_with_specific_path(&who, &path, SwapLimit::ExactTarget(max_supply_amount, target_amount))
					.map_err(|e|
						PrecompileFailure::Revert {
							exit_status: ExitRevert::Reverted,
							output: Output::encode_error_msg("DEX SwapWithExactTarget failed", e),
						}
					)?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_uint(value),
				})
			}
			Action::AddLiquidity => {
				let who = input.account_id_at(1)?;
				let currency_id_a = input.currency_id_at(2)?;
				let currency_id_b = input.currency_id_at(3)?;
				let max_amount_a = input.balance_at(4)?;
				let max_amount_b = input.balance_at(5)?;
				let min_share_increment = input.balance_at(6)?;

				log::debug!(
					target: "evm",
					"dex: add_liquidity who: {:?}, currency_id_a: {:?}, currency_id_b: {:?}, max_amount_a: {:?}, max_amount_b: {:?}, min_share_increment: {:?}",
					who, currency_id_a, currency_id_b, max_amount_a, max_amount_b, min_share_increment,
				);

				<module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::add_liquidity(
					&who,
					currency_id_a,
					currency_id_b,
					max_amount_a,
					max_amount_b,
					min_share_increment,
					false,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("DEX AddLiquidity failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
			Action::RemoveLiquidity => {
				let who = input.account_id_at(1)?;
				let currency_id_a = input.currency_id_at(2)?;
				let currency_id_b = input.currency_id_at(3)?;
				let remove_share = input.balance_at(4)?;
				let min_withdrawn_a = input.balance_at(5)?;
				let min_withdrawn_b = input.balance_at(6)?;

				log::debug!(
					target: "evm",
					"dex: remove_liquidity who: {:?}, currency_id_a: {:?}, currency_id_b: {:?}, remove_share: {:?}, min_withdrawn_a: {:?}, min_withdrawn_b: {:?}",
					who, currency_id_a, currency_id_b, remove_share, min_withdrawn_a, min_withdrawn_b,
				);

				<module_dex::Pallet<Runtime> as DEXManager<Runtime::AccountId, Balance, CurrencyId>>::remove_liquidity(
					&who,
					currency_id_a,
					currency_id_b,
					remove_share,
					min_withdrawn_a,
					min_withdrawn_b,
					false,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("DEX RemoveLiquidity failed", e),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: vec![],
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + module_dex::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<
			Action,
			Runtime::AccountId,
			Runtime::AddressMapping,
			<Runtime as module_dex::Config>::Erc20InfoMapping,
		>::new(handle.input());
		let action = input.action()?;

		let cost: u64 = match action {
			Action::GetLiquidityPool => {
				let currency_id_a = input.currency_id_at(1)?;
				let currency_id_b = input.currency_id_at(2)?;
				let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					.saturating_add(read_currency_a)
					.saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetLiquidityTokenAddress => {
				let currency_id_a = input.currency_id_at(1)?;
				let currency_id_b = input.currency_id_at(2)?;
				let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::TradingPairStatuses (r: 1)
				// primitives::currency::AssetMetadatas (r: 2)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(3);

				Self::BASE_COST
					.saturating_add(read_currency_a)
					.saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetSwapTargetAmount => {
				let path_len = input.u32_at(3)?;

				let mut read_currency = 0u64;
				for i in 0..path_len {
					let currency_id = input.currency_id_at((4 + i) as usize)?;
					read_currency += InputPricer::<Runtime>::read_currency(currency_id);
				}

				// DEX::TradingPairStatuses (r: 1 * (path_len - 1))
				// DEX::LiquidityPool (r: 1 * (path_len - 1))
				let weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(path_len.saturating_sub(1).saturating_mul(2).into());

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::GetSwapSupplyAmount => {
				let path_len = input.u32_at(3)?;

				let mut read_currency = 0u64;
				for i in 0..path_len {
					let currency_id = input.currency_id_at((4 + i) as usize)?;
					read_currency += InputPricer::<Runtime>::read_currency(currency_id);
				}

				// DEX::TradingPairStatuses (r: 1 * (path_len - 1))
				// DEX::LiquidityPool (r: 1 * (path_len - 1))
				let weight = <Runtime as frame_system::Config>::DbWeight::get()
					.reads(path_len.saturating_sub(1).saturating_mul(2).into());

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::SwapWithExactSupply => {
				let path_len = input.u32_at(5)?;

				let mut read_currency = 0u64;
				for i in 0..path_len {
					let currency_id = input.currency_id_at((6 + i) as usize)?;
					read_currency += InputPricer::<Runtime>::read_currency(currency_id);
				}

				let read_account = InputPricer::<Runtime>::read_accounts(1);

				let weight = <Runtime as module_dex::Config>::WeightInfo::swap_with_exact_supply(path_len);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(read_account)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::SwapWithExactTarget => {
				let path_len = input.u32_at(5)?;

				let mut read_currency = 0u64;
				for i in 0..path_len {
					let currency_id = input.currency_id_at((6 + i) as usize)?;
					read_currency += InputPricer::<Runtime>::read_currency(currency_id);
				}

				let read_account = InputPricer::<Runtime>::read_accounts(1);

				let weight = <Runtime as module_dex::Config>::WeightInfo::swap_with_exact_target(path_len);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(read_account)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::AddLiquidity => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id_a = input.currency_id_at(2)?;
				let currency_id_b = input.currency_id_at(3)?;

				let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				let weight = <Runtime as module_dex::Config>::WeightInfo::add_liquidity();

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency_a)
					.saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::RemoveLiquidity => {
				let read_account = InputPricer::<Runtime>::read_accounts(1);
				let currency_id_a = input.currency_id_at(2)?;
				let currency_id_b = input.currency_id_at(3)?;

				let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				let weight = <Runtime as module_dex::Config>::WeightInfo::remove_liquidity();

				Self::BASE_COST
					.saturating_add(read_account)
					.saturating_add(read_currency_a)
					.saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{alice_evm_addr, new_test_ext, DexModule, RuntimeOrigin, Test, ALICE, AUSD, DOT};
	use frame_support::{assert_noop, assert_ok};
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context, ExitRevert};

	type DEXPrecompile = crate::DEXPrecompile<Test>;

	#[test]
	fn get_liquidity_works() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getLiquidityPool(address,address) -> 0xf4f31ede
			// DOT
			// AUSD
			let input = hex! {"
				f4f31ede
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// 1_000
			// 1_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000000000000003e8
				00000000000000000000000000000000 000000000000000000000000000f4240
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_liquidity_token_address_works() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getLiquidityTokenAddress(address,address) -> 0xffd73c4a
			// DOT
			// AUSD
			let input = hex! {"
				ffd73c4a
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// LP_DOT_AUSD
			let expected_output = hex! {"
				000000000000000000000000 0000000000000000000200000000010000000002
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// getLiquidityTokenAddress(address,address) -> 0xffd73c4a
			// DOT
			// unknown token
			let input = hex! {"
				ffd73c4a
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 00000000000000000001000000000000000000ff
			"};

			assert_noop!(
				DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid currency id".into(),
				}
			);
		});
	}

	#[test]
	fn get_swap_target_amount_works() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getSwapTargetAmount(address[],uint256) -> 0x4d60beb1
			// offset
			// supply_amount
			// path_len
			// DOT
			// AUSD
			let input = hex! {"
				4d60beb1
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000002
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// 989
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000000000000003dd
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn get_swap_supply_amount_works() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// getSwapSupplyAmount(address[],uint256) -> 0xdbcd19a2
			// offset
			// target_amount
			// path_len
			// DOT
			// AUSD
			let input = hex! {"
				dbcd19a2
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000002
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// 1
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000001
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn swap_with_exact_supply_works() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// swapWithExactSupply(address,address[],uint256,uint256) -> 0x579baa18
			// who
			// offset
			// supply_amount
			// min_target_amount
			// path_len
			// DOT
			// AUSD
			let input = hex! {"
				579baa18
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000000000000000000000000000 00000002
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// 989
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000000000000003dd
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn dex_precompile_swap_with_exact_target_should_work() {
		new_test_ext().execute_with(|| {
			// enable DOT/AUSD
			assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

			assert_ok!(DexModule::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				DOT,
				AUSD,
				1_000,
				1_000_000,
				0,
				true
			));

			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};

			// swapWithExactSupply(address,address[],uint256,uint256) -> 0x9782ac81
			// who
			// offset
			// target_amount
			// max_supply_amount
			// path_len
			// DOT
			// AUSD
			let input = hex! {"
				9782ac81
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000002
				000000000000000000000000 0000000000000000000100000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
			"};

			// 1
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000001
			"};

			let resp = DEXPrecompile::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}
}
