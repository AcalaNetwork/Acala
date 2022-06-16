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

use frame_support::ensure;
use sp_std::{marker::PhantomData, result::Result, vec::Vec};

use crate::WeightToGas;
use ethabi::Token;
use frame_support::traits::Get;
use module_evm::{runner::state::PrecompileFailure, ExitRevert};
use module_support::{AddressMapping as AddressMappingT, Erc20InfoMapping as Erc20InfoMappingT};
use primitives::{Balance, CurrencyId, DexShare};
use sp_core::{H160, U256};
use sp_runtime::traits::Convert;
use sp_std::prelude::*;

pub const FUNCTION_SELECTOR_LENGTH: usize = 4;
pub const PER_PARAM_BYTES: usize = 32;
pub const HALF_PARAM_BYTES: usize = PER_PARAM_BYTES / 2;
pub const ACTION_INDEX: usize = 0;

pub trait InputT {
	type Error;
	type Action;
	type AccountId;

	fn nth_param(&self, n: usize, len: Option<usize>) -> Result<&[u8], Self::Error>;
	fn action(&self) -> Result<Self::Action, Self::Error>;

	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error>;
	fn evm_address_at(&self, index: usize) -> Result<H160, Self::Error>;
	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error>;

	fn i128_at(&self, index: usize) -> Result<i128, Self::Error>;
	fn u256_at(&self, index: usize) -> Result<U256, Self::Error>;

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error>;

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error>;
	fn u32_at(&self, index: usize) -> Result<u32, Self::Error>;

	fn bytes_at(&self, start: usize, len: usize) -> Result<Vec<u8>, Self::Error>;
	fn bool_at(&self, index: usize) -> Result<bool, Self::Error>;
}

pub struct Input<'a, Action, AccountId, AddressMapping, Erc20InfoMapping> {
	content: &'a [u8],
	target_gas: Option<u64>,
	_marker: PhantomData<(Action, AccountId, AddressMapping, Erc20InfoMapping)>,
}
impl<'a, Action, AccountId, AddressMapping, Erc20InfoMapping>
	Input<'a, Action, AccountId, AddressMapping, Erc20InfoMapping>
{
	pub fn new(content: &'a [u8], target_gas: Option<u64>) -> Self {
		Self {
			content,
			target_gas,
			_marker: PhantomData,
		}
	}
}

impl<Action, AccountId, AddressMapping, Erc20InfoMapping> InputT
	for Input<'_, Action, AccountId, AddressMapping, Erc20InfoMapping>
where
	Action: TryFrom<u32>,
	AddressMapping: AddressMappingT<AccountId>,
	Erc20InfoMapping: Erc20InfoMappingT,
{
	type Error = PrecompileFailure;
	type Action = Action;
	type AccountId = AccountId;

	fn nth_param(&self, n: usize, len: Option<usize>) -> Result<&[u8], Self::Error> {
		let (start, end) = if n == 0 {
			// ACTION_INDEX
			let start = 0;
			let end = start + FUNCTION_SELECTOR_LENGTH;
			(start, end)
		} else {
			let start = FUNCTION_SELECTOR_LENGTH + PER_PARAM_BYTES * (n - 1);
			let end = start + len.unwrap_or(PER_PARAM_BYTES);
			(start, end)
		};

		ensure!(
			end <= self.content.len(),
			PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid input".into(),
				cost: self.target_gas.unwrap_or_default(),
			}
		);

		Ok(&self.content[start..end])
	}

	fn action(&self) -> Result<Self::Action, Self::Error> {
		let param = self.nth_param(ACTION_INDEX, None)?;
		let action = u32::from_be_bytes(param.try_into().map_err(|_| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid action".into(),
			cost: self.target_gas.unwrap_or_default(),
		})?);

		action.try_into().map_err(|_| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid action".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut address = [0u8; 20];
		address.copy_from_slice(&param[12..]);

		Ok(AddressMapping::get_account_id(&address.into()))
	}

	fn evm_address_at(&self, index: usize) -> Result<H160, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut address = [0u8; 20];
		address.copy_from_slice(&param[12..]);

		Ok(H160::from_slice(&address))
	}

	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error> {
		let address = self.evm_address_at(index)?;

		Erc20InfoMapping::decode_evm_address(address).ok_or_else(|| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid currency id".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn i128_at(&self, index: usize) -> Result<i128, Self::Error> {
		let param = self.nth_param(index, None)?;
		decode_i128(param).ok_or(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "failed to decode i128".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn u256_at(&self, index: usize) -> Result<U256, Self::Error> {
		let param = self.nth_param(index, None)?;
		Ok(U256::from_big_endian(param))
	}

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error> {
		let param = self.u256_at(index)?;
		param.try_into().map_err(|_| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "failed to convert uint256 into Balance".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error> {
		let param = self.u256_at(index)?;
		param.try_into().map_err(|_| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "failed to convert uint256 into u64".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn u32_at(&self, index: usize) -> Result<u32, Self::Error> {
		let param = self.u256_at(index)?;
		param.try_into().map_err(|_| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "failed to convert uint256 into u32".into(),
			cost: self.target_gas.unwrap_or_default(),
		})
	}

	fn bytes_at(&self, index: usize, len: usize) -> Result<Vec<u8>, Self::Error> {
		let bytes = self.nth_param(index, Some(len))?;

		Ok(bytes.to_vec())
	}

	fn bool_at(&self, index: usize) -> Result<bool, Self::Error> {
		const ONE: U256 = U256([1u64, 0, 0, 0]);
		let param = self.u256_at(index)?;
		if param == ONE {
			Ok(true)
		} else if param.is_zero() {
			Ok(false)
		} else {
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to decode bool".into(),
				cost: self.target_gas.unwrap_or_default(),
			})
		}
	}
}

pub struct Output;

impl Output {
	pub fn encode_bool(b: bool) -> Vec<u8> {
		ethabi::encode(&[Token::Bool(b)])
	}

	pub fn encode_uint<T>(b: T) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Uint(U256::from(b))])
	}

	pub fn encode_uint_tuple<T>(b: Vec<T>) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Tuple(b.into_iter().map(U256::from).map(Token::Uint).collect())])
	}

	pub fn encode_uint_array<T>(b: Vec<T>) -> Vec<u8>
	where
		U256: From<T>,
	{
		ethabi::encode(&[Token::Array(b.into_iter().map(U256::from).map(Token::Uint).collect())])
	}

	pub fn encode_bytes(b: &[u8]) -> Vec<u8> {
		ethabi::encode(&[Token::Bytes(b.to_vec())])
	}

	pub fn encode_fixed_bytes(b: &[u8]) -> Vec<u8> {
		ethabi::encode(&[Token::FixedBytes(b.to_vec())])
	}

	pub fn encode_address(b: H160) -> Vec<u8> {
		ethabi::encode(&[Token::Address(b)])
	}

	pub fn encode_address_tuple(b: Vec<H160>) -> Vec<u8> {
		ethabi::encode(&[Token::Tuple(b.into_iter().map(Token::Address).collect())])
	}

	pub fn encode_address_array(b: Vec<H160>) -> Vec<u8> {
		ethabi::encode(&[Token::Array(b.into_iter().map(Token::Address).collect())])
	}
}

pub struct InputPricer<T>(PhantomData<T>);

impl<T> InputPricer<T>
where
	T: frame_system::Config,
{
	const BASE_COST: u64 = 200;

	pub(crate) fn read_currency(currency_id: CurrencyId) -> u64 {
		match currency_id {
			CurrencyId::DexShare(a, b) => {
				let cost_a = if matches!(a, DexShare::Erc20(_)) {
					// AssetRegistry::Erc20IdToAddress (r: 1)
					WeightToGas::convert(T::DbWeight::get().reads(1))
				} else {
					Self::BASE_COST
				};
				let cost_b = if matches!(b, DexShare::Erc20(_)) {
					// AssetRegistry::Erc20IdToAddress (r: 1)
					WeightToGas::convert(T::DbWeight::get().reads(1))
				} else {
					Self::BASE_COST
				};
				cost_a.saturating_add(cost_b)
			}
			_ => Self::BASE_COST,
		}
	}

	pub(crate) fn read_accounts(count: u64) -> u64 {
		// EvmAccounts::Accounts
		WeightToGas::convert(T::DbWeight::get().reads(count))
	}
}

fn decode_i128(bytes: &[u8]) -> Option<i128> {
	if bytes[0..HALF_PARAM_BYTES] == [0xff; HALF_PARAM_BYTES] {
		if let Ok(v) = i128::try_from(!U256::from(bytes)) {
			if let Some(v) = v.checked_neg() {
				return v.checked_sub(1);
			}
		}
		return None;
	} else if bytes[0..HALF_PARAM_BYTES] == [0x00; HALF_PARAM_BYTES] {
		return i128::try_from(U256::from_big_endian(bytes)).ok();
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;

	use frame_support::{assert_err, assert_ok};
	use hex_literal::hex;
	use num_enum::TryFromPrimitive;
	use sp_core::H160;
	use sp_runtime::RuntimeDebug;
	use std::str::FromStr;

	use module_support::mocks::{MockAddressMapping, MockErc20InfoMapping};
	use primitives::{AccountId, CurrencyId, TokenSymbol};

	#[derive(RuntimeDebug, PartialEq, Eq, TryFromPrimitive)]
	#[repr(u32)]
	pub enum Action {
		QueryBalance = 0,
		Transfer = 1,
		Unknown = 2,
	}

	pub type TestInput<'a> = Input<'a, Action, AccountId, MockAddressMapping, MockErc20InfoMapping>;

	#[test]
	fn nth_param_works() {
		let data = hex_literal::hex! {"
			00000000
			ffffffffffffffffffffffffffffffff00000000000000000000000000000001
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(
			input.nth_param(1, None),
			&hex!("ffffffffffffffffffffffffffffffff00000000000000000000000000000001")[..]
		);
		assert_err!(
			input.nth_param(2, None),
			PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid input".into(),
				cost: 10,
			}
		);
	}

	#[test]
	fn action_works() {
		let input = TestInput::new(&hex!("00000000")[..], None);
		assert_ok!(input.action(), Action::QueryBalance);

		let input = TestInput::new(&hex!("00000001")[..], None);
		assert_ok!(input.action(), Action::Transfer);

		let input = TestInput::new(&hex!("00000002")[..], None);
		assert_ok!(input.action(), Action::Unknown);

		let input = TestInput::new(&hex!("00000003")[..], Some(10));
		assert_eq!(
			input.action(),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid action".into(),
				cost: 10,
			})
		);
	}

	#[test]
	fn account_id_works() {
		// extra bytes should be ignored
		let data = hex_literal::hex! {"
			00000000
			000000000000000000000000 ff00000000000000000000000000000000000001
			ffffffffffffffffffffffff 0000000000000000000000000000000000000002
			ffffffffffffffffffffffff ff00000000000000000000000000000000000003
		"};

		let input = TestInput::new(&data[..], None);
		assert_ok!(
			input.account_id_at(1),
			MockAddressMapping::get_account_id(&H160::from_str("ff00000000000000000000000000000000000001").unwrap())
		);
		assert_ok!(
			input.account_id_at(2),
			MockAddressMapping::get_account_id(&H160::from_str("0000000000000000000000000000000000000002").unwrap())
		);
		assert_ok!(
			input.account_id_at(3),
			MockAddressMapping::get_account_id(&H160::from_str("ff00000000000000000000000000000000000003").unwrap())
		);
	}

	#[test]
	fn evm_address_works() {
		// extra bytes should be ignored
		let data = hex_literal::hex! {"
			00000000
			000000000000000000000000 ff00000000000000000000000000000000000001
			ffffffffffffffffffffffff 0000000000000000000000000000000000000002
			ffffffffffffffffffffffff ff00000000000000000000000000000000000003
		"};
		let input = TestInput::new(&data[..], None);
		assert_ok!(
			input.evm_address_at(1),
			H160::from_str("ff00000000000000000000000000000000000001").unwrap()
		);
		assert_ok!(
			input.evm_address_at(2),
			H160::from_str("0000000000000000000000000000000000000002").unwrap()
		);
		assert_ok!(
			input.evm_address_at(3),
			H160::from_str("ff00000000000000000000000000000000000003").unwrap()
		);
	}

	#[test]
	fn currency_id_works() {
		let input = TestInput::new(&[0u8; 100][..], Some(10));
		assert_err!(
			input.currency_id_at(1),
			PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid currency id".into(),
				cost: 10,
			}
		);

		// extra bytes should be ignored
		let data = hex_literal::hex! {"
			00000000
			000000000000000000000000 0000000000000000000100000000000000000000
			000000000000000000000000 0000000000000000000100000000000000000001
			ffffffffffffffffffffffff 0000000000000000000100000000000000000002
		"};

		let input = TestInput::new(&data[..], None);
		assert_ok!(input.currency_id_at(1), CurrencyId::Token(TokenSymbol::ACA));
		assert_ok!(input.currency_id_at(2), CurrencyId::Token(TokenSymbol::AUSD));
		assert_ok!(input.currency_id_at(3), CurrencyId::Token(TokenSymbol::DOT));
	}

	#[test]
	fn u256_works() {
		let data = hex_literal::hex! {"
			00000000
			000000000000000000000000000000000000000000000000000000000000007f
			00000000000000000000000000000000ffffffffffffffffffffffffffffffff
			ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
		"};
		let input = TestInput::new(&data[..], None);
		assert_ok!(input.u256_at(1), U256::from(127u128));
		assert_ok!(input.u256_at(2), U256::from(u128::MAX));
		assert_ok!(input.u256_at(3), U256::MAX);
	}

	#[test]
	fn balance_works() {
		let data = hex_literal::hex! {"
			00000000
			00000000000000000000000000000000 0000000000000000000000000000007f
			00000000000000000000000000000000 ffffffffffffffffffffffffffffffff
			ffffffffffffffffffffffffffffffff ffffffffffffffffffffffffffffffff
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(input.balance_at(1), 127u128);
		assert_ok!(input.balance_at(2), u128::MAX);
		assert_eq!(
			input.balance_at(3),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to convert uint256 into Balance".into(),
				cost: 10,
			})
		);
	}

	#[test]
	fn u64_works() {
		let data = hex_literal::hex! {"
			00000000
			000000000000000000000000000000000000000000000000 000000000000007f
			000000000000000000000000000000000000000000000000 ffffffffffffffff
			000000000000000000000000000000000000000000000001 ffffffffffffffff
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(input.u64_at(1), 127u64);
		assert_ok!(input.u64_at(2), u64::MAX);
		assert_eq!(
			input.u64_at(3),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to convert uint256 into u64".into(),
				cost: 10,
			})
		);
	}

	#[test]
	fn u32_works() {
		let data = hex_literal::hex! {"
			00000000
			00000000000000000000000000000000000000000000000000000000 0000007f
			00000000000000000000000000000000000000000000000000000000 ffffffff
			00000000000000000000000000000000000000000000000000000001 ffffffff
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(input.u32_at(1), 127u32);
		assert_ok!(input.u32_at(2), u32::MAX);
		assert_eq!(
			input.u32_at(3),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to convert uint256 into u32".into(),
				cost: 10,
			})
		);
	}

	#[test]
	fn i128_works() {
		let data = hex_literal::hex! {"
			00000000
			00000000000000000000000000000000 0000000000000000000000000000007f
			0fffffffffffffffffffffffffffffff 00000000000000000000000000000001
			ffffffffffffffffffffffffffffffff ffffffffffffffffffffffffffffffff
			ffffffffffffffffffffffffffffffff fffffffffffffffffffffffffffffff0
			ff000000000000000000000000000000 ffffffffffffffffffffffffffffffff
			00000000000000000000000000000000 00000000000000000000000000000000
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(input.i128_at(1), 127_i128);
		assert_eq!(
			input.i128_at(2),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to decode i128".into(),
				cost: 10,
			})
		);
		assert_ok!(input.i128_at(3), -1_i128);
		assert_ok!(input.i128_at(4), -16_i128);
		assert_eq!(
			input.i128_at(5),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to decode i128".into(),
				cost: 10,
			})
		);
		assert_ok!(input.i128_at(6), 0);
	}

	#[test]
	fn bool_works() {
		let data = hex_literal::hex! {"
			00000000
			0000000000000000000000000000000000000000000000000000000000000000
			0000000000000000000000000000000000000000000000000000000000000001
			0000000000000000000000000000000000000000000000000000000000000002
		"};
		let input = TestInput::new(&data[..], Some(10));
		assert_ok!(input.bool_at(1), false);
		assert_ok!(input.bool_at(2), true);
		assert_eq!(
			input.bool_at(3),
			Err(PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "failed to decode bool".into(),
				cost: 10,
			})
		);
	}

	#[test]
	fn decode_int128() {
		let items = [
			("ff00000000000000000000000000000000000000000000000000000000000000", None),
			("0000000000000000000000000000000100000000000000000000000000000000", None),
			("000000000000000000000000000000ff00000000000000000000000000000000", None),
			(
				"0000000000000000000000000000000010000000000000000000000000000000",
				Some(21267647932558653966460912964485513216i128),
			),
			(
				"fffffffffffffffffffffffffffffffff0000000000000000000000000000000",
				Some(-21267647932558653966460912964485513216i128),
			),
			(
				"0000000000000000000000000000000000000000000000000000000000000000",
				Some(0),
			),
			(
				"ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
				Some(-1),
			),
			(
				"0000000000000000000000000000000000000000000000000000000000000001",
				Some(1),
			),
			(
				"fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff0",
				Some(-16),
			),
			(
				"00000000000000000000000000000000000000000000000000000000000000ff",
				Some(255),
			),
			(
				"ffffffffffffffffffffffffffffffffffffffffffff000000000000000000ff",
				Some(-1208925819614629174705921),
			),
			(
				"00000000000000000000000000000000000000000000ffffffffffffffffff00",
				Some(1208925819614629174705920),
			),
			(
				"ffffffffffffffffffffffffffffffff80000000000000000000000000000000",
				Some(i128::MIN),
			),
			(
				"000000000000000000000000000000007fffffffffffffffffffffffffffffff",
				Some(i128::MAX),
			),
			("00000000000000000000000000000000ffffffffffffffffffffffffffffffff", None),
			("ffffffffffffffffffffffffffffffff00000000000000000000000000000000", None),
		];

		items.into_iter().for_each(|(input, value)| {
			assert_eq!(decode_i128(&crate::from_hex(input).unwrap()), value);
		});
	}
}
