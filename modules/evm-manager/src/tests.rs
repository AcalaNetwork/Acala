// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

//! Unit tests for the evm-manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{alice, deploy_contracts, erc20_address, erc20_address_not_exists, ExtBuilder, Runtime};
use orml_utilities::with_transaction_result;
use primitives::TokenSymbol;
use sp_core::H160;
use std::str::FromStr;

#[test]
fn set_erc20_mapping_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));

			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));

			assert_noop!(
				with_transaction_result(|| -> DispatchResult {
					EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(
						EvmAddress::from_str("0000000000000000000000000000000200000000").unwrap(),
					)
				}),
				Error::<Runtime>::CurrencyIdExisted,
			);

			assert_noop!(
				with_transaction_result(|| -> DispatchResult {
					EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(
						EvmAddress::from_str("0000000000000000000000000000000200000001").unwrap(),
					)
				}),
				Error::<Runtime>::CurrencyIdExisted,
			);

			assert_noop!(
				with_transaction_result(|| -> DispatchResult {
					EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address_not_exists())
				}),
				module_evm_bridge::Error::<Runtime>::InvalidReturnValue,
			);
		});
}

#[test]
fn get_evm_address_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::get_evm_address(DexShare::Erc20(erc20_address()).into()),
				Some(erc20_address())
			);

			assert_eq!(EvmCurrencyIdMapping::<Runtime>::get_evm_address(u32::default()), None);
		});
}

#[test]
fn name_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::Token(TokenSymbol::ACA)),
				Some(b"Acala".to_vec())
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::Erc20(erc20_address())),
				Some(b"long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Token(TokenSymbol::AUSD))),
				Some(b"LP Acala - Acala Dollar".to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Token(TokenSymbol::AUSD))),
				Some(b"LP long string name, long string name, long string name, long string name, long string name - Acala Dollar"[..32].to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Erc20(erc20_address()))),
				Some(b"LP long string name, long string name, long string name, long string name, long string name - long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Erc20(erc20_address_not_exists()))),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Erc20(erc20_address_not_exists()))),
				None
			);
		});
}

#[test]
fn symbol_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::Token(TokenSymbol::ACA)),
				Some(b"ACA".to_vec())
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::Erc20(erc20_address())),
				Some(b"TestToken".to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(b"LP_ACA_AUSD".to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(b"LP_TestToken_AUSD".to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				Some(b"LP_TestToken_TestToken".to_vec())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);
		});
}

#[test]
fn decimals_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::Token(TokenSymbol::ACA)),
				Some(12)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::Erc20(erc20_address())),
				Some(17)
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(12)
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(17)
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				Some(17)
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);
		});
}

#[test]
fn encode_evm_address_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Token(TokenSymbol::ACA)),
				H160::from_str("0x0000000000000000000000000000000001000000").ok()
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address())),
				Some(erc20_address())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address_not_exists())),
				Some(erc20_address_not_exists())
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				H160::from_str("0x0000000000000000000000010000000000000001").ok()
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				H160::from_str("0x0000000000000000000000010200000000000001").ok()
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::AUSD),
					DexShare::Erc20(erc20_address())
				)),
				H160::from_str("0x0000000000000000000000010000000102000000").ok()
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				H160::from_str("0x0000000000000000000000010200000002000000").ok()
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);
		});
}

#[test]
fn decode_evm_address_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Token(TokenSymbol::ACA)).unwrap()
				),
				Some(CurrencyId::Token(TokenSymbol::ACA))
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address())).unwrap()
				),
				Some(CurrencyId::Erc20(erc20_address()))
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address_not_exists()))
						.unwrap()
				),
				None,
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Token(TokenSymbol::ACA),
						DexShare::Token(TokenSymbol::AUSD)
					))
					.unwrap(),
				),
				Some(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				))
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Erc20(erc20_address()),
						DexShare::Token(TokenSymbol::AUSD)
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				))
			);

			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					EvmCurrencyIdMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Erc20(erc20_address()),
						DexShare::Erc20(erc20_address())
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				))
			);

			// decode invalid evm address
			// CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA),
			// DexShare::Erc20(erc20_address_not_exists()))
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					H160::from_str("0x0000000000000000000000010000000002000001").unwrap()
				),
				None
			);

			// decode invalid evm address
			// CurrencyId::DexShare(DexShare::Erc20(erc20_address()),
			// DexShare::Erc20(erc20_address_not_exists()))
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(
					H160::from_str("0x0000000000000000000000010200000002000001").unwrap()
				),
				None
			);

			// decode invalid evm address
			// Allow non-system contracts
			let non_system_contracts = H160::from_str("0x1000000000000000000000000000000000000000").unwrap();
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(non_system_contracts),
				None
			);

			let id = Into::<u32>::into(DexShare::Erc20(non_system_contracts));
			CurrencyIdMap::<Runtime>::mutate(id, |maybe_erc20_info| {
				let info = Erc20Info {
					address: non_system_contracts,
					name: b"Test".to_vec(),
					symbol: b"T".to_vec(),
					decimals: 17,
				};

				*maybe_erc20_info = Some(info);
			});
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_evm_address(non_system_contracts),
				Some(CurrencyId::Erc20(non_system_contracts))
			);
		});
}
