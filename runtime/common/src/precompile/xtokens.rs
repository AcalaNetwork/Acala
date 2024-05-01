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

use super::input::{Input, InputPricer, InputT, Output, PER_PARAM_BYTES};
use crate::WeightToGas;
use frame_support::pallet_prelude::{Decode, Encode, IsType};
use module_evm::{
	precompiles::Precompile, ExitRevert, ExitSucceed, PrecompileFailure, PrecompileHandle, PrecompileOutput,
	PrecompileResult,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::{XcmTransfer, XtokensWeightInfo};
use orml_xtokens::XtokensWeight;
use primitives::{Balance, CurrencyId};
use sp_core::Get;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};
use xcm::{
	prelude::*,
	v4::{Asset, Assets, Location},
};

/// The `Xtokens` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Transfer. Rest `input` bytes: `who`, `currency_id`, `amount`, `dest`, `weight`.
/// - TransferMultiasset. Rest `input` bytes: `who`, `asset`, `dest`, `weight`.
/// - TransferWithFee. Rest `input` bytes: `who`, `currency_id`, `amount`, `fee`, `dest`, `weight`.
/// - TransferMultiAssetWithFee. Rest `input` bytes: `who`, `asset`, `fee`, `dest`, `weight`.
/// - TransferMultiCurrencies. Rest `input` bytes: `who`, `currencies`, `fee_item`, `dest`,
///   `weight`.
/// - TransferMultiAssets. Rest `input` bytes: `who`, `assets`, `fee_item`, `dest`, `weight`.
pub struct XtokensPrecompile<R>(PhantomData<R>);

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Transfer = "transfer(address,address,uint256,bytes,bytes)",
	TransferMultiAsset = "transferMultiAsset(address,bytes,bytes,bytes)",
	TransferWithFee = "transferWithFee(address,address,uint256,uint256,bytes,bytes)",
	TransferMultiAssetWithFee = "transferMultiAssetWithFee(address,bytes,bytes,bytes,bytes)",
	TransferMultiCurrencies = "transferMultiCurrencies(address,(address,uint256)[],uint32,bytes,bytes)",
	TransferMultiAssets = "transferMultiAssets(address,bytes,uint32,bytes,bytes)",
}

impl<Runtime> Precompile for XtokensPrecompile<Runtime>
where
	Runtime: module_evm::Config + orml_xtokens::Config + module_prices::Config,
	orml_xtokens::Pallet<Runtime>: XcmTransfer<Runtime::AccountId, Balance, CurrencyId>,
	<Runtime as orml_xtokens::Config>::CurrencyId: IsType<CurrencyId>,
	<Runtime as orml_xtokens::Config>::Balance: IsType<Balance>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let gas_cost = Pricer::<Runtime>::cost(handle)?;
		handle.record_cost(gas_cost)?;

		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		match action {
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let amount = input.balance_at(3)?;

				let dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(5)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
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
				>>::transfer(from, currency_id, amount, dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: Transfer failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken Transfer failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
			Action::TransferMultiAsset => {
				let from = input.account_id_at(1)?;

				let asset_bytes: &[u8] = &input.bytes_at(2)?[..];
				let asset: Asset = decode_asset(asset_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi asset".into(),
				})?;

				let dest_bytes: &[u8] = &input.bytes_at(3)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(4)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
				})?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAsset from: {:?}, asset: {:?}, dest: {:?}, weight: {:?}",
					from, asset, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiasset(from, asset, dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: TransferMultiAsset failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken TransferMultiAsset failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
			Action::TransferWithFee => {
				let from = input.account_id_at(1)?;
				let currency_id = input.currency_id_at(2)?;
				let amount = input.balance_at(3)?;
				let fee = input.balance_at(4)?;

				let dest_bytes: &[u8] = &input.bytes_at(5)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(6)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
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
				>>::transfer_with_fee(from, currency_id, amount, fee, dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: TransferWithFee failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken TransferWithFee failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
			Action::TransferMultiAssetWithFee => {
				let from = input.account_id_at(1)?;

				let asset_bytes: &[u8] = &input.bytes_at(2)?[..];
				let asset: Asset = decode_asset(asset_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi asset".into(),
				})?;

				let fee_bytes: &[u8] = &input.bytes_at(3)?[..];
				let fee: Asset = decode_asset(fee_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid fee asset".into(),
				})?;

				let dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(5)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
				})?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAssetWithFee from: {:?}, asset: {:?}, fee: {:?}, dest: {:?}, weight: {:?}",
					from, asset, fee, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiasset_with_fee(from, asset, fee, dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: TransferMultiAssetWithFee failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken TransferMultiAssetWithFee failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
			Action::TransferMultiCurrencies => {
				let from = input.account_id_at(1)?;
				let currencies_offset = input.u32_at(2)?;
				let currencies_index = (currencies_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let currencies_len = input.u32_at(currencies_index)? as usize;

				if currencies_len > <Runtime as orml_xtokens::Config>::MaxAssetsForTransfer::get() {
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid currencies size".into(),
					});
				}

				let mut currencies = Vec::with_capacity(currencies_len);
				for i in 0..currencies_len {
					let index = currencies_index.saturating_add(i.saturating_mul(2)); // address + amount
					let currency_id = input.currency_id_at(index.saturating_add(1))?;
					let amount = input.balance_at(index.saturating_add(2))?;

					currencies.push((currency_id, amount));
				}

				let fee_item = input.u32_at(3)?;

				let dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(5)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
				})?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiCurrencies from: {:?}, currencies: {:?}, fee_item: {:?}, dest: {:?}, weight: {:?}",
					from, currencies, fee_item, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multicurrencies(from, currencies, fee_item, dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: TransferMultiCurrencies failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken TransferMultiCurrencies failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
			Action::TransferMultiAssets => {
				let from = input.account_id_at(1)?;

				let assets_bytes: &[u8] = &input.bytes_at(2)?[..];
				let assets: Assets = decode_assets(assets_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi assets".into(),
				})?;

				let fee_item = input.u32_at(3)?;
				let fee: &Asset = assets.get(fee_item as usize).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid fee index".into(),
				})?;

				let dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest: Location = decode_location(dest_bytes).ok_or(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let mut weight_bytes: &[u8] = &input.bytes_at(5)?[..];
				let weight = WeightLimit::decode(&mut weight_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid weight".into(),
				})?;

				log::debug!(
					target: "evm",
					"xtokens: TransferMultiAssets from: {:?}, assets: {:?}, fee: {:?}, dest: {:?}, weight: {:?}",
					from, assets, fee, dest, weight
				);

				let transferred = <orml_xtokens::Pallet<Runtime> as XcmTransfer<
					Runtime::AccountId,
					Balance,
					CurrencyId,
				>>::transfer_multiassets(from, assets.clone(), fee.clone(), dest, weight)
				.map_err(|e| {
					log::debug!(
						target: "evm",
						"xtokens: TransferMultiAssets failed: {:?}",
						e
					);
					PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: Output::encode_error_msg("Xtoken TransferMultiAssets failed", e),
					}
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					output: Output::encode_bytes_tuple(vec![&transferred.assets.encode(), &transferred.fee.encode()]),
				})
			}
		}
	}
}

fn decode_asset(mut bytes: &[u8]) -> Option<Asset> {
	VersionedAsset::decode(&mut bytes).ok()?.try_into().ok()
}

fn decode_assets(mut bytes: &[u8]) -> Option<Assets> {
	VersionedAssets::decode(&mut bytes).ok()?.try_into().ok()
}

fn decode_location(mut bytes: &[u8]) -> Option<Location> {
	VersionedLocation::decode(&mut bytes).ok()?.try_into().ok()
}

struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime: module_evm::Config + orml_xtokens::Config + module_prices::Config,
	<Runtime as orml_xtokens::Config>::CurrencyId: IsType<CurrencyId>,
	<Runtime as orml_xtokens::Config>::Balance: IsType<Balance>,
{
	const BASE_COST: u64 = 200;

	fn cost(handle: &mut impl PrecompileHandle) -> Result<u64, PrecompileFailure> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(
			handle.input(),
		);

		let action = input.action()?;

		let cost: u64 = match action {
			Action::Transfer => {
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

				let amount = input.balance_at(3)?;

				let mut dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight = XtokensWeight::<Runtime>::weight_of_transfer(currency_id.into(), amount.into(), &dest);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiAsset => {
				let mut asset_bytes: &[u8] = &input.bytes_at(2)?[..];
				let asset = VersionedAsset::decode(&mut asset_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi asset".into(),
				})?;

				let mut dest_bytes: &[u8] = &input.bytes_at(3)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight = XtokensWeight::<Runtime>::weight_of_transfer_multiasset(&asset, &dest);

				Self::BASE_COST.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferWithFee => {
				let currency_id = input.currency_id_at(2)?;
				let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

				let amount = input.balance_at(3)?;

				let mut dest_bytes: &[u8] = &input.bytes_at(5)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight = XtokensWeight::<Runtime>::weight_of_transfer(currency_id.into(), amount.into(), &dest);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiAssetWithFee => {
				let mut asset_bytes: &[u8] = &input.bytes_at(2)?[..];
				let asset = VersionedAsset::decode(&mut asset_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi asset".into(),
				})?;

				let mut dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight = XtokensWeight::<Runtime>::weight_of_transfer_multiasset(&asset, &dest);

				Self::BASE_COST.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiCurrencies => {
				let currencies_offset = input.u32_at(2)?;
				let currencies_index = (currencies_offset as usize)
					.saturating_div(PER_PARAM_BYTES)
					.saturating_add(1);
				let currencies_len = input.u32_at(currencies_index)? as usize;

				if currencies_len > <Runtime as orml_xtokens::Config>::MaxAssetsForTransfer::get() {
					return Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "invalid currencies size".into(),
					});
				}

				let mut currencies = Vec::with_capacity(currencies_len);
				let mut read_currency: u64 = 0;

				for i in 0..currencies_len {
					let index = currencies_index.saturating_add(i.saturating_mul(2)); // address + amount
					let currency_id = input.currency_id_at(index.saturating_add(1))?;
					let amount = input.balance_at(index.saturating_add(2))?;

					currencies.push((currency_id.into(), amount.into()));
					read_currency = read_currency.saturating_add(InputPricer::<Runtime>::read_currency(currency_id));
				}

				let fee_item = input.u32_at(3)?;

				let mut dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight =
					XtokensWeight::<Runtime>::weight_of_transfer_multicurrencies(&currencies, &fee_item, &dest);

				Self::BASE_COST
					.saturating_add(read_currency)
					.saturating_add(WeightToGas::convert(weight))
			}
			Action::TransferMultiAssets => {
				let mut assets_bytes: &[u8] = &input.bytes_at(2)?[..];
				let assets = VersionedAssets::decode(&mut assets_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid multi asset".into(),
				})?;

				let fee_item = input.u32_at(3)?;

				let mut dest_bytes: &[u8] = &input.bytes_at(4)?[..];
				let dest = VersionedLocation::decode(&mut dest_bytes).map_err(|_| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid dest".into(),
				})?;

				let weight = XtokensWeight::<Runtime>::weight_of_transfer_multiassets(&assets, &fee_item, &dest);

				Self::BASE_COST.saturating_add(WeightToGas::convert(weight))
			}
		};
		Ok(cost)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{alice_evm_addr, new_test_ext, Test, BOB};
	use frame_support::weights::Weight;
	use hex_literal::hex;
	use module_evm::{precompiles::tests::MockPrecompileHandle, Context, ExitRevert};

	use orml_utilities::with_transaction_result;

	type XtokensPrecompile = crate::precompile::XtokensPrecompile<Test>;

	#[test]
	fn transfer_works() {
		new_test_ext().execute_with(|| {
			let context = Context {
				address: Default::default(),
				caller: alice_evm_addr(),
				apparent_value: Default::default(),
			};
			let dest: VersionedLocation = VersionedLocation::V4(Location::new(
				1,
				[
					Parachain(2002),
					Junction::AccountId32 {
						network: None,
						id: BOB.into(),
					},
				],
			));
			assert_eq!(
				dest.encode(),
				hex!("04010200491f01000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Unlimited;
			assert_eq!(weight.encode(), hex!("00"));

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// transfer(address,address,uint256,bytes,bytes) -> 0xc78fed04
			// from
			// currency
			// amount
			// dest offset
			// weight offset
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				c78fed04
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 00000000000000000000000000000100
				00000000000000000000000000000000 00000000000000000000000000000028
				03010200491f0100020202020202020202020202020202020202020202020202
				0202020202020202000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken Transfer failed: NotCrossChainTransferableCurrency".into(),
					})
				);
				Ok(())
			});
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
			let asset: VersionedAsset = (Here, 1_000_000_000_000u128).into();
			assert_eq!(asset.encode(), hex!("04000000070010a5d4e8"));

			let dest: VersionedLocation = VersionedLocation::V4(
				Junction::AccountId32 {
					network: None,
					id: BOB.into(),
				}
				.into(),
			);
			assert_eq!(
				dest.encode(),
				hex!("04000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// transferMultiAsset(address,bytes,bytes,bytes) -> 0x948796cf
			// from
			// asset offset
			// dest offset
			// weight offset
			// asset length
			// asset
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				948796cf
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000080
				00000000000000000000000000000000 000000000000000000000000000000c0
				00000000000000000000000000000000 00000000000000000000000000000120
				00000000000000000000000000000000 0000000000000000000000000000000b
				0300000000070010a5d4e8000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000025
				0300010100020202020202020202020202020202020202020202020202020202
				0202020202000000000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken TransferMultiAsset failed: InvalidDest".into(),
					})
				);
				Ok(())
			});
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
			let dest: VersionedLocation = VersionedLocation::V4(
				Junction::AccountId32 {
					network: None,
					id: BOB.into(),
				}
				.into(),
			);
			assert_eq!(
				dest.encode(),
				hex!("04000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// transferWithFee(address,address,uint256,uint256,bytes,bytes) -> 0x0c8d6181
			// from
			// currency
			// amount
			// fee
			// dest offset
			// weight offset
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				0c8d6181
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 0000000000000000000100000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 000000000000000000000000000000c0
				00000000000000000000000000000000 00000000000000000000000000000120
				00000000000000000000000000000000 00000000000000000000000000000025
				0300010100020202020202020202020202020202020202020202020202020202
				0202020202000000000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken TransferWithFee failed: NotCrossChainTransferableCurrency".into(),
					})
				);
				Ok(())
			});
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
			let asset: VersionedAsset = (Here, 1_000_000_000_000u128).into();
			assert_eq!(asset.encode(), hex!("04000000070010a5d4e8"));

			let fee: VersionedAsset = (Here, 1_000_000).into();
			assert_eq!(fee.encode(), hex!("0400000002093d00"));

			let dest: VersionedLocation = VersionedLocation::V4(
				Junction::AccountId32 {
					network: None,
					id: BOB.into(),
				}
				.into(),
			);
			assert_eq!(
				dest.encode(),
				hex!("04000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// transferMultiAssetWithFee(address,bytes,bytes,bytes,bytes) -> 0x3ccae822
			// from
			// asset offset
			// fee offset
			// dest offset
			// weight offset
			// asset length
			// asset
			// fee length
			// fee
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				3ccae822
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 000000000000000000000000000000e0
				00000000000000000000000000000000 00000000000000000000000000000120
				00000000000000000000000000000000 00000000000000000000000000000180
				00000000000000000000000000000000 0000000000000000000000000000000b
				0300000000070010a5d4e8000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				030000000002093d000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000025
				0300010100020202020202020202020202020202020202020202020202020202
				0202020202000000000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken TransferMultiAssetWithFee failed: InvalidDest".into(),
					})
				);
				Ok(())
			});
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
			let dest: VersionedLocation = VersionedLocation::V4(
				Junction::AccountId32 {
					network: None,
					id: BOB.into(),
				}
				.into(),
			);
			assert_eq!(
				dest.encode(),
				hex!("04000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// currencies
			// [[1000000000000000000000000000000000000001,1],[1000000000000000000000000000000000000001,2]]

			// transferMultiCurrencies(address,(address,uint256)[],uint32,bytes,bytes) -> 0xcfea5c46
			// from
			// currencies offset
			// fee item
			// dest offset
			// weight offset
			// currencies length
			// address1
			// amount1
			// address2
			// amount2
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				cfea5c46
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 00000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000140
				00000000000000000000000000000000 000000000000000000000000000001a0
				00000000000000000000000000000000 00000000000000000000000000000002
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000001
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 00000000000000000000000000000002
				00000000000000000000000000000000 00000000000000000000000000000025
				0300010100020202020202020202020202020202020202020202020202020202
				0202020202000000000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken TransferMultiCurrencies failed: NotCrossChainTransferableCurrency".into(),
					})
				);
				Ok(())
			});
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
			let assets: VersionedAssets = VersionedAssets::from(Assets::from((Here, 1_000_000_000_000u128)));
			assert_eq!(assets.encode(), hex!("0404000000070010a5d4e8"));

			let dest: VersionedLocation = VersionedLocation::V4(
				Junction::AccountId32 {
					network: None,
					id: BOB.into(),
				}
				.into(),
			);
			assert_eq!(
				dest.encode(),
				hex!("04000101000202020202020202020202020202020202020202020202020202020202020202")
			);

			let weight = WeightLimit::Limited(Weight::from_parts(100_000, 64 * 1024));
			assert_eq!(weight.encode(), hex!("01821a060002000400"));

			// transferMultiAssets(address,bytes,bytes,bytes,bytes) -> 0x97ed2b15
			// from
			// assets offset
			// fee_item
			// dest offset
			// weight offset
			// assets length
			// assets
			// dest length
			// dest
			// weight length
			// weight
			let input = hex! {"
				97ed2b15
				000000000000000000000000 1000000000000000000000000000000000000001
				00000000000000000000000000000000 000000000000000000000000000000a0
				00000000000000000000000000000000 00000000000000000000000000000000
				00000000000000000000000000000000 000000000000000000000000000000e0
				00000000000000000000000000000000 00000000000000000000000000000140
				00000000000000000000000000000000 0000000000000000000000000000000c
				030400000000070010a5d4e80000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000025
				0300010100020202020202020202020202020202020202020202020202020202
				0202020202000000000000000000000000000000000000000000000000000000
				00000000000000000000000000000000 00000000000000000000000000000009
				01821a0600020004000000000000000000000000000000000000000000000000
			"};

			let _ = with_transaction_result(|| {
				assert_eq!(
					XtokensPrecompile::execute(&mut MockPrecompileHandle::new(&input, Some(10_000), &context, false)),
					Err(PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Xtoken TransferMultiAssets failed: InvalidDest".into(),
					})
				);
				Ok(())
			});
		});
	}
}
