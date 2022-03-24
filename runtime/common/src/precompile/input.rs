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
use sp_std::{marker::PhantomData, result::Result, vec, vec::Vec};

use crate::WeightToGas;
use ethabi::Token;
use frame_support::traits::Get;
use module_evm::{runner::state::PrecompileFailure, ExitRevert};
use module_support::{AddressMapping as AddressMappingT, Erc20InfoMapping as Erc20InfoMappingT};
use primitives::{Balance, CurrencyId, DexShare};
use sp_core::{H160, U256};
use sp_runtime::traits::Convert;

pub const FUNCTION_SELECTOR_LENGTH: usize = 4;
pub const PER_PARAM_BYTES: usize = 32;
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

	fn u256_at(&self, index: usize) -> Result<U256, Self::Error>;

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error>;

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error>;
	fn u32_at(&self, index: usize) -> Result<u32, Self::Error>;

	fn bytes_at(&self, start: usize, len: usize) -> Result<Vec<u8>, Self::Error>;
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
}

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Output;

impl Output {
	pub fn encode_bool(&self, b: bool) -> Vec<u8> {
		let out = Token::Bool(b);
		ethabi::encode(&[out])
	}

	pub fn encode_u8(&self, b: u8) -> Vec<u8> {
		let out = Token::Uint(U256::from(b));
		ethabi::encode(&[out])
	}

	pub fn encode_u32(&self, b: u32) -> Vec<u8> {
		let out = Token::Uint(U256::from(b));
		ethabi::encode(&[out])
	}

	pub fn encode_u128(&self, b: u128) -> Vec<u8> {
		let out = Token::Uint(U256::from(b));
		ethabi::encode(&[out])
	}

	pub fn encode_u128_tuple(&self, b: u128, c: u128) -> Vec<u8> {
		let out = Token::Tuple(vec![Token::Uint(U256::from(b)), Token::Uint(U256::from(c))]);
		ethabi::encode(&[out])
	}

	pub fn encode_bytes(&self, b: &[u8]) -> Vec<u8> {
		let out = Token::Bytes(b.to_vec());
		ethabi::encode(&[out])
	}

	pub fn encode_address(&self, b: &H160) -> Vec<u8> {
		let out = Token::Address(H160::from_slice(b.as_bytes()));
		ethabi::encode(&[out])
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
}
