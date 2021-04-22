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
					EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address_not_exists())
				}),
				Error::<Runtime>::CurrencyIdExisted,
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
				EvmCurrencyIdMapping::<Runtime>::get_evm_address(
					CurrencyId::Erc20(erc20_address()).try_into().unwrap()
				),
				Some(erc20_address())
			);

			assert_eq!(EvmCurrencyIdMapping::<Runtime>::get_evm_address(u32::default()), None);
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
		});
}

#[test]
fn encode_currency_id_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));

			// CurrencyId::Token
			let mut bytes = [0u8; 32];
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::Token(TokenSymbol::ACA)),
				Some(bytes)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::Token(TokenSymbol::ACA))
			);

			bytes[31] = 1;
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::Token(TokenSymbol::AUSD)),
				Some(bytes)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::Token(TokenSymbol::AUSD))
			);

			// CurrencyId::Erc20
			let mut bytes = [0u8; 32];
			bytes[11] = 2;
			bytes[12..32].copy_from_slice(&erc20_address().as_bytes()[..]);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::Erc20(erc20_address())),
				Some(bytes)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::Erc20(erc20_address()))
			);

			// CurrencyId::DexShare(Token, Token)
			let mut bytes = [0u8; 32];
			bytes[11] = 1;
			let id1: u32 = CurrencyId::Token(TokenSymbol::ACA).try_into().unwrap();
			let id2: u32 = CurrencyId::Token(TokenSymbol::AUSD).try_into().unwrap();
			bytes[12..16].copy_from_slice(&id1.to_be_bytes()[..]);
			bytes[16..20].copy_from_slice(&id2.to_be_bytes()[..]);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(bytes)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				))
			);

			// CurrencyId::DexShare(Erc20, Erc20)
			let mut bytes = [0u8; 32];
			bytes[11] = 1;
			let id1: u32 = CurrencyId::Erc20(erc20_address()).try_into().unwrap();
			let id2: u32 = CurrencyId::Erc20(erc20_address()).try_into().unwrap();
			bytes[12..16].copy_from_slice(&id1.to_be_bytes()[..]);
			bytes[16..20].copy_from_slice(&id2.to_be_bytes()[..]);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				Some(bytes)
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				))
			);

			// Invalid CurrencyId::DexShare(_, _)
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::encode_currency_id(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address_not_exists()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);
		});
}

#[test]
fn decode_currency_id_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(with_transaction_result(|| -> DispatchResult {
				EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(erc20_address())
			}));

			// CurrencyId::Token
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&[0u8; 32]),
				Some(CurrencyId::Token(TokenSymbol::ACA))
			);
			assert_eq!(EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&[255u8; 32]), None);

			// CurrencyId::DexShare(Token, Token)
			let mut bytes = [0u8; 32];
			bytes[11] = 1;
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::ACA)
				))
			);

			// CurrencyId::DexShare(Erc20, Token)
			let mut bytes = [0u8; 32];
			bytes[11] = 1;
			let id: u32 = CurrencyId::Erc20(erc20_address()).try_into().unwrap();
			bytes[12..16].copy_from_slice(&id.to_be_bytes()[..]);
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::ACA)
				))
			);

			// CurrencyId::Erc20
			bytes[11] = 2;
			assert_eq!(
				EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes),
				Some(CurrencyId::Erc20(
					H160::from_str("0x0200000000000000000000000000000000000000").unwrap()
				))
			);

			// Invalid
			bytes[11] = 3;
			assert_eq!(EvmCurrencyIdMapping::<Runtime>::decode_currency_id(&bytes), None);
		});
}
