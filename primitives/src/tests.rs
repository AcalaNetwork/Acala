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
use frame_support::assert_ok;
use std::{
	convert::{TryFrom, TryInto},
	str::FromStr,
};

#[test]
fn trading_pair_works() {
	let aca = CurrencyId::Token(TokenSymbol::ACA);
	let ausd = CurrencyId::Token(TokenSymbol::AUSD);
	let erc20 = CurrencyId::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000000").unwrap());
	let aca_ausd_lp = CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Token(TokenSymbol::AUSD));
	let erc20_aca_lp = CurrencyId::DexShare(
		DexShare::Token(TokenSymbol::ACA),
		DexShare::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000000").unwrap()),
	);

	assert_eq!(
		TradingPair::from_currency_ids(ausd, aca).unwrap(),
		TradingPair(aca, ausd)
	);
	assert_eq!(
		TradingPair::from_currency_ids(aca, ausd).unwrap(),
		TradingPair(aca, ausd)
	);
	assert_eq!(
		TradingPair::from_currency_ids(erc20, aca).unwrap(),
		TradingPair(aca, erc20)
	);
	assert_eq!(TradingPair::from_currency_ids(aca, aca), None);

	assert_eq!(
		TradingPair::from_currency_ids(ausd, aca)
			.unwrap()
			.dex_share_currency_id(),
		aca_ausd_lp
	);
	assert_eq!(
		TradingPair::from_currency_ids(aca, erc20)
			.unwrap()
			.dex_share_currency_id(),
		erc20_aca_lp
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
fn currency_id_into_u32_works() {
	let currency_id = DexShare::Token(TokenSymbol::ACA);
	assert_eq!(Into::<u32>::into(currency_id), 0x00);

	let currency_id = DexShare::Token(TokenSymbol::AUSD);
	assert_eq!(Into::<u32>::into(currency_id), 0x01);

	let currency_id = DexShare::Erc20(EvmAddress::from_str("0x2000000000000000000000000000000000000000").unwrap());
	assert_eq!(Into::<u32>::into(currency_id), 0x20000000);

	let currency_id = DexShare::Erc20(EvmAddress::from_str("0x0000000000000001000000000000000000000000").unwrap());
	assert_eq!(Into::<u32>::into(currency_id), 0x01000000);

	let currency_id = DexShare::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000001").unwrap());
	assert_eq!(Into::<u32>::into(currency_id), 0x01);

	let currency_id = DexShare::Erc20(EvmAddress::from_str("0x0000000000000000000000000000000000000000").unwrap());
	assert_eq!(Into::<u32>::into(currency_id), 0x00);
}

#[test]
fn currency_id_try_into_evm_address_works() {
	assert_eq!(
		EvmAddress::try_from(CurrencyId::Token(TokenSymbol::ACA,)),
		Ok(EvmAddress::from_str("0x0000000000000000000000000000000001000000").unwrap())
	);

	assert_eq!(
		EvmAddress::try_from(CurrencyId::DexShare(
			DexShare::Token(TokenSymbol::ACA),
			DexShare::Token(TokenSymbol::AUSD),
		)),
		Ok(EvmAddress::from_str("0x0000000000000000000000010000000000000001").unwrap())
	);

	assert_eq!(
		EvmAddress::try_from(CurrencyId::DexShare(
			DexShare::Erc20(Default::default()),
			DexShare::Erc20(Default::default())
		)),
		Err(())
	);

	let erc20 = EvmAddress::from_str("0x1111111111111111111111111111111111111111").unwrap();
	assert_eq!(EvmAddress::try_from(CurrencyId::Erc20(erc20)), Ok(erc20));
}

#[test]
fn generate_function_selector_works() {
	#[primitives_proc_macro::generate_function_selector]
	#[derive(RuntimeDebug, Eq, PartialEq)]
	#[repr(u32)]
	pub enum Action {
		Name = "name()",
		Symbol = "symbol()",
		Decimals = "decimals()",
		TotalSupply = "totalSupply()",
		BalanceOf = "balanceOf(address)",
		Transfer = "transfer(address,uint256)",
	}

	assert_eq!(Action::Name as u32, 0x06fdde03_u32);
	assert_eq!(Action::Symbol as u32, 0x95d89b41_u32);
	assert_eq!(Action::Decimals as u32, 0x313ce567_u32);
	assert_eq!(Action::TotalSupply as u32, 0x18160ddd_u32);
	assert_eq!(Action::BalanceOf as u32, 0x70a08231_u32);
	assert_eq!(Action::Transfer as u32, 0xa9059cbb_u32);
}
