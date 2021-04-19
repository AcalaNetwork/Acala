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

use super::*;
use crate::evm::EvmAddress;
use std::{convert::TryInto, str::FromStr};

use frame_support::{assert_err, assert_ok};

#[test]
fn currency_id_try_from_bytes_works() {
	let mut bytes = [0u8; 32];
	bytes[31] = 1;
	assert_ok!(bytes.try_into(), CurrencyId::Token(TokenSymbol::AUSD));

	let mut bytes = [0u8; 32];
	bytes[11..16].copy_from_slice(&[0, 0, 0, 0, u8::MAX][..]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());

	let mut bytes = [0u8; 32];
	bytes[11..20].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, 1][..]);
	// No support CurrencyId::DexShare. EVM-manager deals with it.
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());

	let mut bytes = [0u8; 32];
	bytes[11..20].copy_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0, u8::MAX]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());

	let mut bytes = [0u8; 32];
	bytes[11..20].copy_from_slice(&[1, 0, 0, 0, u8::MAX, 0, 0, 0, 0]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());
}

#[test]
fn currency_id_decode_bytes_works() {
	let mut bytes = [0u8; 32];
	assert_ok!(bytes.try_into(), CurrencyId::Token(TokenSymbol::ACA));

	bytes[11] = 2;
	bytes[12] = 32;
	assert_ok!(
		bytes.try_into(),
		CurrencyId::Erc20(EvmAddress::from_str("0x2000000000000000000000000000000000000000").unwrap())
	);
}

#[test]
fn currency_id_try_from_vec_u8_works() {
	assert_ok!(
		"ACA".as_bytes().to_vec().try_into(),
		CurrencyId::Token(TokenSymbol::ACA)
	);
}

#[test]
fn currency_id_try_into_u32_works() {
	let currency_id = CurrencyId::Erc20(EvmAddress::from_str("0x2000000000000000000000000000000000000000").unwrap());
	assert_eq!(currency_id.try_into(), Ok(0x20000000));

	let currency_id = CurrencyId::Erc20(EvmAddress::from_str("0x0000000000000001000000000000000000000000").unwrap());
	assert_eq!(currency_id.try_into(), Ok(0x01000000));

	let currency_id = CurrencyId::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000001").unwrap());
	assert_eq!(currency_id.try_into(), Ok(0x01));

	let currency_id = CurrencyId::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000000").unwrap());
	assert_eq!(currency_id.try_into(), Ok(0x00));
}
