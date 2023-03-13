// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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
	input::{Input, InputPricer, InputT, Output, PER_PARAM_BYTES},
	target_gas_limit,
};
use crate::WeightToGas;
use frame_support::{
	log,
	pallet_prelude::{Decode, Encode},
	traits::Get,
};
use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};
use module_support::Erc20InfoMapping;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::XcmTransfer;
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};
use xcm::{
	latest::{MultiAsset, MultiAssets, MultiLocation},
	prelude::*,
};

/// The `Xtokens` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Transfer. Rest `input` bytes: `currency_id_a`, `currency_id_b`.
/// - TransferMultiasset. Rest `input` bytes: `who`, `currency_id_a`, `currency_id_b`,
///   `supply_amount`, `min_target_amount`.
pub struct XtokensPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Transfer = "transfer(address,address,uint256,bytes,uint64)",
	TransferMultiAsset = "transferMultiAsset(address,bytes,bytes,uint64)",
	TransferWithFee = "transferWithFee(address,address,uint256,uint256,bytes,uint64)",
	TransferMultiAssetWithFee = "transferMultiAssetWithFee(address,bytes,bytes,bytes,uint64)",
	TransferMultiCurrencies = "transferMultiCurrencies(address,(address,uint256)[],uint32,bytes,uint64)",
	TransferMultiAssets = "transferMultiAssets(address,bytes,bytes,bytes,uint64)",
}

impl<Runtime> Precompile for XtokensPrecompile<Runtime>
where
	Runtime: module_evm::Config + orml_xtokens::Config + module_prices::Config,
	orml_xtokens::Pallet<Runtime>: XcmTransfer<Runtime::AccountId, Balance, CurrencyId>,
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
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let currency_address = input.evm_address_at(2)?;
				let amount = input.balance_at(3)?;
				// solidity abi encode bytes will add an offset at input[4]
				let weight = input.u64_at(5)?;
				let dest_bytes_len = input.u32_at(6)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(7, dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;

				let currency_id = Runtime::Erc20InfoMapping::decode_evm_address(currency_address).ok_or_else(|| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid currency id".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					}
				})?;

				log::debug!(
					target: "evm",
					"xtokens: Transfer from: {:?}, currency_id: {:?}, amount: {:?}, dest: {:?}, weight: {:?}",
					from, currency_id, amount, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer(from, currency_id, amount, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken Transfer failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
			Action::TransferMultiAsset => {
				let from = input.account_id_at(1)?;
				// solidity abi encode bytes will add an offset at input[2]
				let asset_offset = input.u64_at(2)?;
				let asset_index = (asset_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let asset_bytes_len = input.u64_at(asset_index)?;
				let mut asset_bytes: &[u8] =
					&input.bytes_at(asset_index.saturating_add(1), asset_bytes_len as usize)?[..];
				let asset: MultiAsset = Decode::decode(&mut asset_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid multi asset".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				// solidity abi encode bytes will add an offset at input[3]
				let dest_offset = input.u64_at(3)?;
				let dest_index = (dest_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let dest_bytes_len = input.u32_at(dest_index)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(dest_index.saturating_add(1), dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				let weight = input.u64_at(4)?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAsset from: {:?}, asset: {:?}, dest: {:?}, weight: {:?}",
					from, asset, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiasset(from, asset, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAsset failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
			Action::TransferWithFee => {
				let from = input.account_id_at(1)?;
				let currency_address = input.evm_address_at(2)?;
				let amount = input.balance_at(3)?;
				let fee = input.balance_at(4)?;
				// solidity abi encode bytes will add an offset at input[5]
				let dest_offset = input.u32_at(5)?;
				let dest_index = (dest_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let dest_bytes_len = input.u32_at(dest_index)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(dest_index.saturating_add(1), dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				let weight = input.u64_at(6)?;

				let currency_id = Runtime::Erc20InfoMapping::decode_evm_address(currency_address).ok_or_else(|| {
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid currency id".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					}
				})?;

				log::debug!(
					target: "evm",
					"xtokens: Transfer from: {:?}, currency_id: {:?}, amount: {:?}, fee: {:?}, dest: {:?}, weight: {:?}",
					from, currency_id, amount, fee, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_with_fee(from, currency_id, amount, fee, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferWithFee failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
			Action::TransferMultiAssetWithFee => {
				let from = input.account_id_at(1)?;
				// solidity abi encode bytes will add an offset at input[2]
				let asset_offset = input.u32_at(2)?;
				let asset_index = (asset_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let asset_bytes_len = input.u32_at(asset_index)?;
				let mut asset_bytes: &[u8] =
					&input.bytes_at(asset_index.saturating_add(1), asset_bytes_len as usize)?[..];
				let asset: MultiAsset = Decode::decode(&mut asset_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid multi asset".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;

				// solidity abi encode bytes will add an offset at input[3]
				let fee_offset = input.u32_at(3)?;
				let fee_index = (fee_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let fee_bytes_len = input.u32_at(fee_index)?;
				let mut fee_bytes: &[u8] = &input.bytes_at(fee_index.saturating_add(1), fee_bytes_len as usize)?[..];
				let fee: MultiAsset = Decode::decode(&mut fee_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid fee asset".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;

				// solidity abi encode bytes will add an offset at input[4]
				let dest_offset = input.u32_at(4)?;
				let dest_index = (dest_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let dest_bytes_len = input.u32_at(dest_index)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(dest_index.saturating_add(1), dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				let weight = input.u64_at(5)?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAssetWithFee from: {:?}, asset: {:?}, fee: {:?}, dest: {:?}, weight: {:?}",
					from, asset, fee, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiasset_with_fee(from, asset, fee, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAssetWithFee failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
			Action::TransferMultiCurrencies => {
				let from = input.account_id_at(1)?;
				let currencies_offset = input.u32_at(2)?;
				let currencies_index = (currencies_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let currencies_len = input.u32_at(currencies_index)? as usize;

				let mut currencies = Vec::with_capacity(currencies_len);
				for i in 0..currencies_len {
					let index = currencies_index.saturating_add(i.saturating_mul(2)); // address + amount
					let currency_address = input.evm_address_at(index.saturating_add(1))?;
					let currency_id =
						Runtime::Erc20InfoMapping::decode_evm_address(currency_address).ok_or_else(|| {
							PrecompileFailure::Revert {
								exit_status: ExitRevert::Reverted,
								output: "invalid currency id".into(),
								cost: target_gas_limit(target_gas).unwrap_or_default(),
							}
						})?;

					let amount = input.balance_at(index.saturating_add(2))?;

					currencies.push((currency_id, amount));
				}

				let fee_item = input.u32_at(3)?;

				// solidity abi encode bytes will add an offset at input[4]
				let dest_offset = input.u32_at(4)?;
				let dest_index = (dest_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let dest_bytes_len = input.u32_at(dest_index)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(dest_index.saturating_add(1), dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				let weight = input.u64_at(5)?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiCurrencies from: {:?}, currencies: {:?}, fee_item: {:?}, dest: {:?}, weight: {:?}",
					from, currencies, fee_item, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multicurrencies(from, currencies, fee_item, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiCurrencies failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
			Action::TransferMultiAssets => {
				let from = input.account_id_at(1)?;
				// solidity abi encode bytes will add an offset at input[2]
				let assets_offset = input.u32_at(2)?;
				let assets_index = (assets_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let assets_bytes_len = input.u32_at(assets_index)?;
				let mut assets_bytes: &[u8] =
					&input.bytes_at(assets_index.saturating_add(1), assets_bytes_len as usize)?[..];
				let assets: MultiAssets = Decode::decode(&mut assets_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid multi assets".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;

				// solidity abi encode bytes will add an offset at input[3]
				let fee_offset = input.u32_at(3)?;
				let fee_index = (fee_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let fee_bytes_len = input.u32_at(fee_index)?;
				let mut fee_bytes: &[u8] = &input.bytes_at(fee_index.saturating_add(1), fee_bytes_len as usize)?[..];
				let fee: MultiAsset = Decode::decode(&mut fee_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid fee asset".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;

				// solidity abi encode bytes will add an offset at input[4]
				let dest_offset = input.u32_at(4)?;
				let dest_index = (dest_offset as usize).saturating_div(PER_PARAM_BYTES).saturating_add(1);
				let dest_bytes_len = input.u32_at(dest_index)?;
				let mut dest_bytes: &[u8] = &input.bytes_at(dest_index.saturating_add(1), dest_bytes_len as usize)?[..];
				let dest: MultiLocation = Decode::decode(&mut dest_bytes).or_else(|_| {
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid dest".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})
				})?;
				let weight = input.u64_at(5)?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAssets from: {:?}, assets: {:?}, fee: {:?}, dest: {:?}, weight: {:?}",
					from, assets, fee, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiassets(from, assets, fee, dest, Limited(weight))
				.map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAssets failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
					logs: Default::default(),
				})
			}
		}
	}
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + orml_xtokens::Config + module_prices::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		let cost: u64 = match action {
			Action::Transfer => {
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
			Action::TransferMultiAsset => {
				// let currency_id_a = input.currency_id_at(1)?;
				// let currency_id_b = input.currency_id_at(2)?;
				// let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				// let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					// .saturating_add(read_currency_a)
					// .saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferWithFee => {
				// let currency_id_a = input.currency_id_at(1)?;
				// let currency_id_b = input.currency_id_at(2)?;
				// let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				// let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					// .saturating_add(read_currency_a)
					// .saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiAssetWithFee => {
				// let currency_id_a = input.currency_id_at(1)?;
				// let currency_id_b = input.currency_id_at(2)?;
				// let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				// let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					// .saturating_add(read_currency_a)
					// .saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiCurrencies => {
				// let currency_id_a = input.currency_id_at(1)?;
				// let currency_id_b = input.currency_id_at(2)?;
				// let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				// let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					// .saturating_add(read_currency_a)
					// .saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiAssets => {
				// let currency_id_a = input.currency_id_at(1)?;
				// let currency_id_b = input.currency_id_at(2)?;
				// let read_currency_a = InputPricer::<Runtime>::read_currency(currency_id_a);
				// let read_currency_b = InputPricer::<Runtime>::read_currency(currency_id_b);

				// DEX::LiquidityPool (r: 1)
				let weight = <Runtime as frame_system::Config>::DbWeight::get().reads(1);

				Self::BASE_COST
					// .saturating_add(read_currency_a)
					// .saturating_add(read_currency_b)
					.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{alice_evm_addr, new_test_ext, Test, BOB};
	use hex_literal::hex;
	use module_evm::ExitRevert;

	type XtokensPrecompile = crate::precompile::XtokensPrecompile<Test>;

	#[test]
	fn transfer_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transfer(address,address,uint256,bytes,uint64) -> 0xdd2a3599
			// from
			// currency
			// amount
			// dest offset
			// weight
			// dest length
			// dest
			let input = hex! {"
				dd2a3599
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 00000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken Transfer failed".into(),
					cost: 0,
				})
			);
		});
	}

	#[test]
	fn transfer_multi_asset_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let asset: MultiAsset = (Here, 1_000_000_000_000).into();
			assert_eq!(asset.encode(), hex!("00000000070010a5d4e8"));

			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transferMultiAsset(address,bytes,bytes,uint64) -> 0xc94c06e7
			// from
			// asset offset
			// dest offset
			// weight
			// asset length
			// asset
			// dest length
			// dest
			let input = hex! {"
				c94c06e7
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000080
				00000000000000000000000000000000 000000000000000000000000000000c0
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 0000000000000000000000000000000a
				00000000070010a5d4e8000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAsset failed".into(),
					cost: 0,
				})
			);
		});
	}

	#[test]
	fn transfer_with_fee_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transferWithFee(address,address,uint256,uint256,bytes,uint64) -> 0x014f858e
			// from
			// currency
			// amount
			// fee
			// dest offset
			// weight
			// dest length
			// dest
			let input = hex! {"
				014f858e
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 000000000000000000000000000000c0
				00000000000000000000000000000000 00000000000000000000000000000003
				00000000000000000000000000000000 00000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferWithFee failed".into(),
					cost: 0,
				})
			);
		});
	}

	#[test]
	fn transfer_multi_asset_with_fee_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let asset: MultiAsset = (Here, 1_000_000_000_000).into();
			assert_eq!(asset.encode(), hex!("00000000070010a5d4e8"));

			let fee: MultiAsset = (Here, 1_000_000).into();
			assert_eq!(fee.encode(), hex!("0000000002093d00"));

			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transferMultiAssetWithFee(address,bytes,bytes,bytes,uint64) -> 0x7c9d2ad5
			// from
			// asset offset
			// fee offset
			// dest offset
			// weight
			// asset length
			// asset
			// fee length
			// fee
			// dest length
			// dest
			let input = hex! {"
				7c9d2ad5
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 000000000000000000000000000000e0
				00000000000000000000000000000000 00000000000000000000000000000120
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 0000000000000000000000000000000a
				00000000070010a5d4e800000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000008
				0000000002093d00000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAssetWithFee failed".into(),
					cost: 0,
				})
			);
		});
	}

	#[test]
	fn transfer_multi_currencies_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transferMultiCurrencies(address,(address,uint256)[],uint32,bytes,uint64) -> 0x78ff822f
			// from
			// currencies offset
			// fee item
			// dest offset
			// weight
			// currencies length
			// address1
			// amount1
			// address2
			// amount2
			// dest length
			// dest
			let input = hex! {"
				78ff822f
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000000000a0
				0000000000000000000000000000000000000000000000000000000000000001
				0000000000000000000000000000000000000000000000000000000000000140
				0000000000000000000000000000000000000000000000000000000000000002
				0000000000000000000000000000000000000000000000000000000000000002
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
				0000000000000000000000000000000000000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiCurrencies failed".into(),
					cost: 0,
				})
			);
		});
	}

	#[test]
	fn transfer_multi_assets_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let assets: MultiAssets = vec![(Here, 1_000_000_000_000).into()].into();
			assert_eq!(assets.encode(), hex!("0400000000070010a5d4e8"));

			let fee: MultiAsset = (Here, 1_000_000).into();
			assert_eq!(fee.encode(), hex!("0000000002093d00"));

			let dest: MultiLocation = Junction::AccountId32 {
				network: NetworkId::Any,
				id: BOB.into(),
			}
			.into();
			assert_eq!(
				dest.encode(),
				hex!("000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			// transferMultiAssets(address,bytes,bytes,bytes,uint64) -> 0x3ab249cd
			// from
			// assets offset
			// fee offset
			// dest offset
			// weight
			// assets length
			// assets
			// fee length
			// fee
			// dest length
			// dest
			let input = hex! {"
				3ab249cd
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000000000000000000000000000000000a0
				00000000000000000000000000000000000000000000000000000000000000e0
				0000000000000000000000000000000000000000000000000000000000000120
				00000000000000000000000000000000 00000000000000000000000000000001
				000000000000000000000000000000000000000000000000000000000000000b
				0400000000070010a5d4e8000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000008
				0000000002093d00000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000024
				0001010002020202020202020202020202020202020202020202020202020202
				0202020200000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				XtokensPrecompile::execute(&input, None, &context, false),
				Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Xtoken TransferMultiAssets failed".into(),
					cost: 0,
				})
			);
		});
	}
}
