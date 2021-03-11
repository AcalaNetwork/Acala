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
use std::convert::TryInto;

use frame_support::{assert_err, assert_ok};

#[test]
fn currency_id_to_bytes_works() {
	assert_eq!(Into::<[u8; 32]>::into(CurrencyId::Token(TokenSymbol::ACA)), [0u8; 32]);

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[0, 1, 0][..]);
	assert_eq!(Into::<[u8; 32]>::into(CurrencyId::Token(TokenSymbol::AUSD)), bytes);

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[0, 5, 0][..]);
	assert_eq!(Into::<[u8; 32]>::into(CurrencyId::Token(TokenSymbol::RENBTC)), bytes);

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[1, 0, 1][..]);
	assert_eq!(
		Into::<[u8; 32]>::into(CurrencyId::DEXShare(TokenSymbol::ACA, TokenSymbol::AUSD)),
		bytes
	);
}

#[test]
fn currency_id_try_from_bytes_works() {
	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[0, 1, 0][..]);
	assert_ok!(bytes.try_into(), CurrencyId::Token(TokenSymbol::AUSD));

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[0, u8::MAX, 0][..]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[1, 0, 1][..]);
	assert_ok!(
		bytes.try_into(),
		CurrencyId::DEXShare(TokenSymbol::ACA, TokenSymbol::AUSD)
	);

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[1, u8::MAX, 0][..]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());

	let mut bytes = [0u8; 32];
	bytes[29..].copy_from_slice(&[1, 0, u8::MAX][..]);
	assert_err!(TryInto::<CurrencyId>::try_into(bytes), ());
}

#[test]
fn currency_id_encode_decode_bytes_works() {
	let currency_id = CurrencyId::Token(TokenSymbol::AUSD);
	let bytes: [u8; 32] = currency_id.into();
	assert_ok!(bytes.try_into(), currency_id)
}

#[test]
fn currency_id_try_from_vec_u8_works() {
	assert_ok!(
		"ACA".as_bytes().to_vec().try_into(),
		CurrencyId::Token(TokenSymbol::ACA)
	);
}
