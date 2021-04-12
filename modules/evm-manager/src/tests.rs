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
use frame_support::assert_ok;
use mock::{ExtBuilder, Runtime, ERC20, ERC20_ADDRESS};
use orml_utilities::with_transaction_result;
use primitives::TokenSymbol;
use sp_core::H160;
use std::str::FromStr;

#[test]
fn set_erc20_mapping_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));

		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));
	});
}

#[test]
fn get_evm_address_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));
		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::get_evm_address(ERC20.into()),
			Some(ERC20_ADDRESS)
		);

		assert_eq!(EvmCurrencyIdMapping::<Runtime>::get_evm_address(u32::default()), None);
	});
}

#[test]
fn decimals_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));
		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::decimals(CurrencyId::Token(TokenSymbol::ACA)),
			Some(12)
		);
		assert_eq!(EvmCurrencyIdMapping::<Runtime>::decimals(ERC20), Some(17));
	});
}

#[test]
fn u256_to_currency_id_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));

		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::u256_to_currency_id(&[0u8; 32]),
			Some(CurrencyId::Token(TokenSymbol::ACA))
		);
		assert_eq!(EvmCurrencyIdMapping::<Runtime>::u256_to_currency_id(&[255u8; 32]), None);

		let mut id = [0u8; 32];
		id[23] = 1;
		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::u256_to_currency_id(&id),
			Some(CurrencyId::DexShare(
				DexShare::Token(TokenSymbol::ACA),
				DexShare::Token(TokenSymbol::ACA)
			))
		);

		// CurrencyId::DexShare(Erc20, token)
		let mut id = [0u8; 32];
		id[23] = 1;
		id[24..28].copy_from_slice(&Into::<u32>::into(ERC20).to_be_bytes()[..]);
		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::u256_to_currency_id(&id),
			Some(CurrencyId::DexShare(
				DexShare::Erc20(H160::from_str("0x2000000000000000000000000000000000000001").unwrap()),
				DexShare::Token(TokenSymbol::ACA)
			))
		);

		// CurrencyId::Erc20
		id[23] = 0;
		assert_eq!(EvmCurrencyIdMapping::<Runtime>::u256_to_currency_id(&id), None);
	});
}
