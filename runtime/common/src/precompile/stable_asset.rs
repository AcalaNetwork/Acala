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
	input::{Input, InputT, Output},
	target_gas_limit,
};
use crate::{precompile::input::InputPricer, WeightToGas};
use frame_support::traits::Get;
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::Erc20InfoMapping;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use nutsfinance_stable_asset::traits::StableAsset;
use nutsfinance_stable_asset::WeightInfo;
use primitives::{Balance, CurrencyId};
use sp_core::H160;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

pub struct StableAssetPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetStableAssetPoolTokens = "getStableAssetPoolTokens(uint32)",
	GetStableAssetPoolTotalSupply = "getStableAssetPoolTotalSupply(uint32)",
	GetStableAssetPoolPrecision = "getStableAssetPoolPrecision(uint32)",
	GetStableAssetPoolMintFee = "getStableAssetPoolMintFee(uint32)",
	GetStableAssetPoolSwapFee = "getStableAssetPoolSwapFee(uint32)",
	GetStableAssetPoolRedeemFee = "getStableAssetPoolRedeemFee(uint32)",
	StableAssetSwap = "stableAssetSwap(address,uint32,uint32,uint32,uint256,uint256,uint32)",
	StableAssetMint = "stableAssetMint(address,uint32,uint256[],uint256)",
	StableAssetRedeem = "stableAssetRedeem(address,uint32,uint256,uint256[])",
}

impl<Runtime> Precompile for StableAssetPrecompile<Runtime>
where
	Runtime: module_evm::Config + nutsfinance_stable_asset::Config + module_prices::Config,
	nutsfinance_stable_asset::Pallet<Runtime>: StableAsset<
		AssetId = CurrencyId,
		AtLeast64BitUnsigned = Balance,
		Balance = Balance,
		AccountId = Runtime::AccountId,
		BlockNumber = Runtime::BlockNumber,
	>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			input,
			target_gas_limit(target_gas),
		);

		let mut gas_cost = Pricer::<Runtime>::cost(&input)?;

		if let Some(gas_limit) = target_gas {
			if gas_limit < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let action = input.action()?;

		match action {
			Action::GetStableAssetPoolTokens => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					// dynamic gas cost calculation
					// cost of reading asset currencies
					gas_cost = gas_cost.saturating_add(
						pool_info
							.assets
							.iter()
							.map(|x| InputPricer::<Runtime>::read_currency(*x))
							.sum::<u64>(),
					);
					// make sure there's enough gas
					if let Some(gas_limit) = target_gas {
						if gas_limit < gas_cost {
							return Err(PrecompileFailure::Error {
								exit_status: ExitError::OutOfGas,
							});
						}
					}

					let assets: Vec<H160> = pool_info
						.assets
						.iter()
						.flat_map(|x| <Runtime as module_prices::Config>::Erc20InfoMapping::encode_evm_address(*x))
						.collect();

					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_address_array(assets),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::GetStableAssetPoolTotalSupply => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_uint(pool_info.total_supply),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::GetStableAssetPoolPrecision => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_uint(pool_info.precision),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::GetStableAssetPoolMintFee => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_uint(pool_info.mint_fee),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::GetStableAssetPoolSwapFee => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_uint(pool_info.swap_fee),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::GetStableAssetPoolRedeemFee => {
				let pool_id = input.u32_at(1)?;

				if let Some(pool_info) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::pool(pool_id) {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Output::encode_uint(pool_info.redeem_fee),
						logs: Default::default(),
					})
				} else {
					Ok(PrecompileOutput {
						exit_status: ExitSucceed::Returned,
						cost: gas_cost,
						output: Default::default(),
						logs: Default::default(),
					})
				}
			}
			Action::StableAssetSwap => {
				let who = input.account_id_at(1)?;
				let pool_id = input.u32_at(2)?;
				let i = input.u32_at(3)?;
				let j = input.u32_at(4)?;
				let dx = input.balance_at(5)?;
				let min_dy = input.balance_at(6)?;
				let asset_length = input.u32_at(7)?;

				let (input, output) = <nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::swap(
					&who,
					pool_id,
					i,
					j,
					dx,
					min_dy,
					asset_length,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Into::<&str>::into(e).as_bytes().to_vec(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint_tuple(vec![input, output]),
					logs: Default::default(),
				})
			}
			Action::StableAssetMint => {
				let who = input.account_id_at(1)?;
				let pool_id = input.u32_at(2)?;
				// solidity abi encode array will add an offset at input[3]
				let min_mint_amount = input.balance_at(4)?;
				let amount_len = input.u32_at(5)?;
				let mut amounts = vec![];
				for i in 0..amount_len {
					amounts.push(input.balance_at((6 + i) as usize)?);
				}

				<nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::mint(
					&who,
					pool_id,
					amounts,
					min_mint_amount,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Into::<&str>::into(e).as_bytes().to_vec(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Default::default(),
					logs: Default::default(),
				})
			}
			Action::StableAssetRedeem => {
				let who = input.account_id_at(1)?;
				let pool_id = input.u32_at(2)?;
				let redeem_amount = input.balance_at(3)?;
				// solidity abi encode array will add an offset at input[4]
				let amount_len = input.u32_at(5)?;
				let mut amounts = vec![];
				for i in 0..amount_len {
					amounts.push(input.balance_at((6 + i) as usize)?);
				}

				<nutsfinance_stable_asset::Pallet<Runtime> as StableAsset>::redeem_proportion(
					&who,
					pool_id,
					redeem_amount,
					amounts,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Into::<&str>::into(e).as_bytes().to_vec(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Default::default(),
					logs: Default::default(),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + nutsfinance_stable_asset::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<
			Action,
			Runtime::AccountId,
			Runtime::AddressMapping,
			<Runtime as module_prices::Config>::Erc20InfoMapping,
		>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::GetStableAssetPoolTokens => {
				// StableAsset::Pools (r: 1)
				let cost = WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1));
				// read asset currencies is calculation dynamically after reading pool_info
				Self::BASE_COST.saturating_add(cost)
			}
			Action::GetStableAssetPoolTotalSupply
			| Action::GetStableAssetPoolPrecision
			| Action::GetStableAssetPoolMintFee
			| Action::GetStableAssetPoolSwapFee
			| Action::GetStableAssetPoolRedeemFee => {
				// StableAsset::Pools (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);
				Self::BASE_COST.saturating_add(WeightToGas::convert(weight))
			}
			Action::StableAssetSwap => {
				let account_read = InputPricer::<Runtime>::read_accounts(1);
				let path_len = input.u32_at(7)?;
				let weight = <Runtime as nutsfinance_stable_asset::Config>::WeightInfo::swap(path_len);
				Self::BASE_COST
					.saturating_add(account_read)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::StableAssetMint => {
				let account_read = InputPricer::<Runtime>::read_accounts(1);
				let path_len = input.u32_at(5)?;
				let weight = <Runtime as nutsfinance_stable_asset::Config>::WeightInfo::mint(path_len);
				Self::BASE_COST
					.saturating_add(account_read)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::StableAssetRedeem => {
				let account_read = InputPricer::<Runtime>::read_accounts(1);
				let path_len = input.u32_at(5)?;
				let weight = <Runtime as nutsfinance_stable_asset::Config>::WeightInfo::redeem_proportion(path_len);
				Self::BASE_COST
					.saturating_add(account_read)
					.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompile::mock::{alice_evm_addr, new_test_ext, Origin, StableAsset, Test, ALICE, AUSD, RENBTC};
	use frame_support::assert_ok;
	use hex_literal::hex;

	type StableAssetPrecompile = crate::StableAssetPrecompile<Test>;

	#[test]
	fn get_stable_asset_pool_tokens_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolTokens(uint32) -> 0xfb0f0f34
			// poolId: 0
			let input = hex! {"
				fb0f0f34
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000020
				00000000000000000000000000000000 00000000000000000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000014
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolTokens(uint32) -> 0xfb0f0f34
			// poolId: 1
			let input = hex! {"
				fb0f0f34
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn get_stable_asset_total_supply_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			assert_ok!(StableAsset::mint(
				Origin::signed(ALICE),
				0,
				vec![1_000_000u128, 1_000_000u128],
				0u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolTotalSupply(uint32) -> 0x7172c6aa
			// poolId: 0
			let input = hex! {"
				7172c6aa
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 000000000000000000000000001e8480
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolTotalSupply(uint32) -> 0x7172c6aa
			// poolId: 1
			let input = hex! {"
				7172c6aa
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn get_stable_asset_precision_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolPrecision(uint32) -> 0x9ccdcf91
			// poolId: 0
			let input = hex! {"
				9ccdcf91
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000001
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolPrecision(uint32) -> 0x9ccdcf91
			// poolId: 1
			let input = hex! {"
				9ccdcf91
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn get_stable_asset_mint_fee_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolMintFee(uint32) -> 0x62ff9875
			// poolId: 0
			let input = hex! {"
				62ff9875
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000002
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolMintFee(uint32) -> 0x62ff9875
			// poolId: 1
			let input = hex! {"
				62ff9875
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn get_stable_asset_swap_fee_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolSwapFee(uint32) -> 0x68410f61
			// poolId: 0
			let input = hex! {"
				68410f61
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000003
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolSwapFee(uint32) -> 0x68410f61
			// poolId: 1
			let input = hex! {"
				68410f61
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn get_stable_asset_redeem_fee_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// getStableAssetPoolRedeemFee(uint32) -> 0x7f2f11ca
			// poolId: 0
			let input = hex! {"
				7f2f11ca
				00000000000000000000000000000000000000000000000000000000 00000000
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000004
			"};
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// empty output if pool doesn't exists

			// getStableAssetPoolRedeemFee(uint32) -> 0x7f2f11ca
			// poolId: 1
			let input = hex! {"
				7f2f11ca
				00000000000000000000000000000000000000000000000000000000 00000001
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert!(resp.output.is_empty());
		});
	}

	#[test]
	fn stable_asset_mint_and_redeem_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// stableAssetMint(address,uint32,uint256[],uint256) -> 0x2acdb2ec
			// who
			// poolId
			// amounts_offset
			// min_mint_amount
			// amounts_len
			// amount
			// amount
			let mint_input = hex! {"
				2acdb2ec
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000000000000000000000000000 0000000000000000
				00000000000000000000000000000000 00000000000000000000000000000080
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 000000000000000000000000000f4240
				00000000000000000000000000000000 000000000000000000000000000f4240
			"};
			let mint_resp = StableAssetPrecompile::execute(&mint_input, None, &context, false).unwrap();
			assert_eq!(mint_resp.exit_status, ExitSucceed::Returned);
			assert!(mint_resp.output.is_empty());

			// stableAssetRedeem(address,uint32,uint256,uint256[]) -> 0xaa538d34
			// who
			// poolId
			// amount
			// offset
			// length
			// amount
			// amount
			let redeem_input = hex! {"
				aa538d34
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000000000000000000000000000 0000000000000000
				00000000000000000000000000000000 0000000000000000000000000007a120
				00000000000000000000000000000000 00000000000000000000000000000080
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
			"};
			let redeem_resp = StableAssetPrecompile::execute(&redeem_input, None, &context, false).unwrap();
			assert_eq!(redeem_resp.exit_status, ExitSucceed::Returned);
			assert!(redeem_resp.output.is_empty());
		});
	}

	#[test]
	fn stable_asset_swap_works() {
		new_test_ext().execute_with(|| {
			assert_ok!(StableAsset::create_pool(
				Origin::signed(ALICE),
				CurrencyId::StableAssetPoolToken(0),
				vec![AUSD, RENBTC],
				vec![1, 1],
				2u128,
				3u128,
				4u128,
				10000,
				ALICE,
				ALICE,
				1u128
			));
			assert_ok!(StableAsset::mint(
				Origin::signed(ALICE),
				0,
				vec![1_000_000u128, 1_000_000u128],
				0u128
			));
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			// stableAssetSwap(address,uint32,uint32,uint32,uint256,uint256,uint32) -> 0xff9bc03c
			// who
			// poolId
			// i
			// j
			// dx
			// min_dy
			// asset_len
			let input = hex! {"
				ff9bc03c
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000000
				00000000000000000000000000000000000000000000000000000000 00000000
				00000000000000000000000000000000000000000000000000000000 00000001
				00000000000000000000000000000000 0000000000000000000000000007a120
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000000000000000000000000000 00000002
			"};

			// 500000
			// 498355
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000000007a120
				00000000000000000000000000000000 00000000000000000000000000079ab3
			"};
			let resp = StableAssetPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output);

			// revert if pool doesn't exists

			// stableAssetSwap(address,uint32,uint32,uint32,uint256,uint256,uint32) -> 0xff9bc03c
			// who
			// poolId
			// i
			// j
			// dx
			// min_dy
			// asset_len
			let input = hex! {"
				ff9bc03c
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000 00000001
				00000000000000000000000000000000000000000000000000000000 00000000
				00000000000000000000000000000000000000000000000000000000 00000001
				00000000000000000000000000000000 0000000000000000000000000007a120
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000000000000000000000000000 00000002
			"};
			let resp = StableAssetPrecompile::execute(&input, Some(200_000), &context, false)
				.err()
				.unwrap();
			assert_eq!(
				resp,
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: b"PoolNotFound".to_vec(),
					cost: target_gas_limit(Some(200_000)).unwrap_or_default()
				}
			);
		});
	}
}
