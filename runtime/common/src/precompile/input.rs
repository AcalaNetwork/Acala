use codec::Decode;

use frame_support::ensure;
use sp_std::{marker::PhantomData, prelude::*, result::Result};

use module_evm::{AddressMapping as AddressMappingT, ExitError};
use primitives::{Amount, CurrencyId};

const PER_PARAM_BYTES: usize = 32;
const ACTION_INDEX: usize = 0;

/// Based on `primitives::CurrencyId` impl.
const CURRENCY_ID_BYTES: usize = 4;

/// Based on `i128` as `primitives::Amount`.
const AMOUNT_BYTES: usize = 16;

macro_rules! ensure_valid_input {
	($e:expr) => {
		ensure!($e, ExitError::Other("invalid input".into()));
	};
}

pub trait InputT {
	type Error;
	type Action;
	type AccountId;

	fn nth_param(&self, n: usize) -> Result<&[u8], Self::Error>;
	fn action(&self) -> Result<Self::Action, Self::Error>;
	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error>;
	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error>;
	fn amount_at(&self, index: usize) -> Result<Amount, Self::Error>;
}

pub struct Input<Action, AccountId, AddressMapping> {
	content: Box<[u8]>,
	_marker: PhantomData<(Action, AccountId, AddressMapping)>,
}
impl<Action, AccountId, AddressMapping> Input<Action, AccountId, AddressMapping> {
	fn new(content: Box<[u8]>) -> Self {
		Self {
			content,
			_marker: PhantomData,
		}
	}
}

impl<Action, AccountId, AddressMapping> InputT for Input<Action, AccountId, AddressMapping>
where
	Action: From<u8>,
	AddressMapping: AddressMappingT<AccountId>,
{
	type Error = ExitError;
	type Action = Action;
	type AccountId = AccountId;

	fn nth_param(&self, n: usize) -> Result<&[u8], Self::Error> {
		let start = PER_PARAM_BYTES * n;
		let end = start + PER_PARAM_BYTES;

		ensure_valid_input!(end <= self.content.len());

		Ok(&self.content[start..end])
	}

	fn action(&self) -> Result<Self::Action, Self::Error> {
		let action_param = self.nth_param(ACTION_INDEX)?;
		let action_u8: &u8 = action_param.last().expect("Action bytes is 32 bytes");

		Ok((*action_u8).into())
	}

	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error> {
		let address_param = self.nth_param(index)?;

		let mut address = [0u8; 20];
		address.copy_from_slice(&address_param[12..]);

		Ok(AddressMapping::into_account_id(address.into()))
	}

	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error> {
		let currency_id_param = self.nth_param(index)?;

		let mut currency_id = [0u8; CURRENCY_ID_BYTES];
		let start = PER_PARAM_BYTES - CURRENCY_ID_BYTES;
		currency_id[..].copy_from_slice(&currency_id_param[start..]);

		CurrencyId::decode(&mut &currency_id[..]).map_err(|_| ExitError::Other("invalid currency".into()))
	}

	fn amount_at(&self, index: usize) -> Result<Amount, Self::Error> {
		let amount_param = self.nth_param(index)?;

		let mut amount = [0u8; AMOUNT_BYTES];
		let start = PER_PARAM_BYTES - AMOUNT_BYTES;
		amount[..].copy_from_slice(&amount_param[start..]);

		Ok(Amount::from_be_bytes(amount))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use frame_support::{assert_err, assert_ok};
	use sp_core::{crypto::AccountId32, H160};

	use primitives::{AccountId, CurrencyId, TokenSymbol};

	#[derive(Debug, PartialEq, Eq)]
	pub enum Action {
		QueryBalance,
		Transfer,
		Unknown,
	}
	impl From<u8> for Action {
		fn from(a: u8) -> Self {
			match a {
				0 => Action::QueryBalance,
				1 => Action::Transfer,
				_ => Action::Unknown,
			}
		}
	}

	pub struct EvmAddressMapping;
	impl AddressMappingT<AccountId> for EvmAddressMapping {
		fn into_account_id(address: H160) -> AccountId {
			let mut data: [u8; 32] = [0u8; 32];
			data[0..4].copy_from_slice(b"evm:");
			data[4..24].copy_from_slice(&address[..]);
			AccountId32::from(data).into()
		}
	}

	pub type TestInput = Input<Action, AccountId, EvmAddressMapping>;

	#[test]
	fn nth_param_works() {
		let input = TestInput::new(Box::new([1u8; 64]));
		assert_ok!(input.nth_param(1), &[1u8; 32][..]);
		assert_err!(input.nth_param(2), ExitError::Other("invalid input".into()));
	}

	#[test]
	fn action_works() {
		let input = TestInput::new(Box::new([0u8; 32]));
		assert_ok!(input.action(), Action::QueryBalance);

		let mut raw_input = [0u8; 32];
		raw_input[31] = 1;
		let input = TestInput::new(Box::new(raw_input));
		assert_ok!(input.action(), Action::Transfer);

		let mut raw_input = [0u8; 32];
		raw_input[31] = 2;
		let input = TestInput::new(Box::new(raw_input));
		assert_ok!(input.action(), Action::Unknown);
	}

	#[test]
	fn account_id_works() {
		let mut address = [0u8; 20];
		address[19] = 1;
		let account_id = EvmAddressMapping::into_account_id(address.into());

		let mut raw_input = [0u8; 32];
		raw_input[31] = 1;
		let input = TestInput::new(Box::new(raw_input));
		assert_ok!(input.account_id_at(0), account_id);
	}

	#[test]
	fn currency_id_works() {
		let input = TestInput::new(Box::new([0u8; 32]));
		assert_ok!(input.currency_id_at(0), CurrencyId::Token(TokenSymbol::ACA));

		let mut raw_input = [0u8; 32];
		raw_input[29] = 1;
		let input = TestInput::new(Box::new(raw_input));
		assert_ok!(input.currency_id_at(0), CurrencyId::Token(TokenSymbol::AUSD));
	}

	#[test]
	fn amount_works() {
		let amount = 127i128;
		let amount_bytes = amount.to_be_bytes();

		let mut raw_input = [0u8; 32];
		raw_input[16..].copy_from_slice(&amount_bytes);
		let input = TestInput::new(Box::new(raw_input));
		assert_ok!(input.amount_at(0), amount);
	}
}
