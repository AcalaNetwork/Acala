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
use sp_std::{marker::PhantomData, mem, result::Result, vec, vec::Vec};

use ethabi::Token;
use module_evm::{runner::state::PrecompileFailure, ExitError};
use module_support::{AddressMapping as AddressMappingT, Erc20InfoMapping as Erc20InfoMappingT};
use primitives::{Amount, Balance, CurrencyId};
use sp_core::{H160, U256};

pub const FUNCTION_SELECTOR_LENGTH: usize = 4;
pub const PER_PARAM_BYTES: usize = 32;
pub const ACTION_INDEX: usize = 0;

pub const BALANCE_BYTES: usize = mem::size_of::<Balance>();
pub const AMOUNT_BYTES: usize = mem::size_of::<Amount>();
pub const U64_BYTES: usize = mem::size_of::<u64>();
pub const U32_BYTES: usize = mem::size_of::<u32>();

pub trait InputT {
	type Error;
	type Action;
	type AccountId;

	fn nth_param(&self, n: usize, len: Option<usize>) -> Result<&[u8], Self::Error>;
	fn action(&self) -> Result<Self::Action, Self::Error>;

	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error>;
	fn evm_address_at(&self, index: usize) -> Result<H160, Self::Error>;
	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error>;

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error>;
	fn amount_at(&self, index: usize) -> Result<Amount, Self::Error>;

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error>;
	fn u32_at(&self, index: usize) -> Result<u32, Self::Error>;

	fn bytes_at(&self, start: usize, len: usize) -> Result<Vec<u8>, Self::Error>;
}

pub struct Input<'a, Action, AccountId, AddressMapping, Erc20InfoMapping> {
	content: &'a [u8],
	_marker: PhantomData<(Action, AccountId, AddressMapping, Erc20InfoMapping)>,
}
impl<'a, Action, AccountId, AddressMapping, Erc20InfoMapping>
	Input<'a, Action, AccountId, AddressMapping, Erc20InfoMapping>
{
	pub fn new(content: &'a [u8]) -> Self {
		Self {
			content,
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
			PrecompileFailure::Error {
				exit_status: ExitError::Other("invalid input".into())
			}
		);

		Ok(&self.content[start..end])
	}

	fn action(&self) -> Result<Self::Action, Self::Error> {
		let param = self.nth_param(ACTION_INDEX, None)?;
		let action = u32::from_be_bytes(param.try_into().map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("invalid action".into()),
		})?);

		action.try_into().map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("invalid action".into()),
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

		Erc20InfoMapping::decode_evm_address(address).ok_or_else(|| PrecompileFailure::Error {
			exit_status: ExitError::Other("invalid currency id".into()),
		})
	}

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut balance = [0u8; BALANCE_BYTES];
		let start = PER_PARAM_BYTES - BALANCE_BYTES;
		balance[..].copy_from_slice(&param[start..]);

		Ok(Balance::from_be_bytes(balance))
	}

	fn amount_at(&self, index: usize) -> Result<Amount, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut amount = [0u8; AMOUNT_BYTES];
		let start = PER_PARAM_BYTES - AMOUNT_BYTES;
		amount[..].copy_from_slice(&param[start..]);

		Ok(Amount::from_be_bytes(amount))
	}

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut num = [0u8; U64_BYTES];
		let start = PER_PARAM_BYTES - U64_BYTES;
		num[..].copy_from_slice(&param[start..]);

		Ok(u64::from_be_bytes(num))
	}

	fn u32_at(&self, index: usize) -> Result<u32, Self::Error> {
		let param = self.nth_param(index, None)?;

		let mut num = [0u8; U32_BYTES];
		let start = PER_PARAM_BYTES - U32_BYTES;
		num[..].copy_from_slice(&param[start..]);

		Ok(u32::from_be_bytes(num))
	}

	fn bytes_at(&self, index: usize, len: usize) -> Result<Vec<u8>, Self::Error> {
		let bytes = self.nth_param(index, Some(len))?;

		Ok(bytes.to_vec())
	}
}

#[derive(Default, Clone, PartialEq, Debug)]
pub struct Output;

impl Output {
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

#[cfg(test)]
mod tests {
	use super::*;

	use frame_support::{assert_err, assert_ok};
	use num_enum::TryFromPrimitive;
	use sp_core::H160;
	use sp_runtime::RuntimeDebug;

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
		let input = TestInput::new(&[1u8; 36][..]);
		assert_ok!(input.nth_param(1, None), &[1u8; 32][..]);
		assert_err!(
			input.nth_param(2, None),
			PrecompileFailure::Error {
				exit_status: ExitError::Other("invalid input".into())
			}
		);
	}

	#[test]
	fn action_works() {
		let input = TestInput::new(&[0u8; 36][..]);
		assert_ok!(input.action(), Action::QueryBalance);

		let mut raw_input = [0u8; 36];
		raw_input[3] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.action(), Action::Transfer);

		let mut raw_input = [0u8; 36];
		raw_input[3] = 2;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.action(), Action::Unknown);

		let mut raw_input = [0u8; 36];
		raw_input[3] = 3;
		let input = TestInput::new(&raw_input[..]);
		assert_eq!(
			input.action(),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::Other("invalid action".into())
			})
		);
	}

	#[test]
	fn account_id_works() {
		let mut address = [0u8; 20];
		address[19] = 1;
		let account_id = MockAddressMapping::get_account_id(&address.into());

		let mut raw_input = [0u8; 36];
		raw_input[35] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.account_id_at(1), account_id);
	}

	#[test]
	fn evm_address_works() {
		let mut address = [0u8; 20];
		address[19] = 1;
		let evm_address = H160::from_slice(&address);

		let mut raw_input = [0u8; 36];
		raw_input[35] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.evm_address_at(1), evm_address);
	}

	#[test]
	fn currency_id_works() {
		let input = TestInput::new(&[0u8; 100][..]);
		assert_err!(
			input.currency_id_at(1),
			PrecompileFailure::Error {
				exit_status: ExitError::Other("invalid currency id".into())
			}
		);

		let mut raw_input = [0u8; 36];
		raw_input[25] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.currency_id_at(1), CurrencyId::Token(TokenSymbol::ACA));

		raw_input[35] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.currency_id_at(1), CurrencyId::Token(TokenSymbol::AUSD));
	}

	#[test]
	fn balance_works() {
		let balance = 127u128;
		let balance_bytes = balance.to_be_bytes();

		let mut raw_input = [0u8; 36];
		raw_input[20..].copy_from_slice(&balance_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.balance_at(1), balance);
	}

	#[test]
	fn amount_works() {
		let amount = 127i128;
		let amount_bytes = amount.to_be_bytes();

		let mut raw_input = [0u8; 36];
		raw_input[20..].copy_from_slice(&amount_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.amount_at(1), amount);
	}

	#[test]
	fn u64_works() {
		let u64_num = 127u64;
		let u64_bytes = u64_num.to_be_bytes();

		let mut raw_input = [0u8; 36];
		raw_input[28..].copy_from_slice(&u64_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.u64_at(1), u64_num);
	}
}
