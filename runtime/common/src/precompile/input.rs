use frame_support::ensure;
use sp_std::{
	convert::{TryFrom, TryInto},
	marker::PhantomData,
	mem,
	result::Result,
	vec::Vec,
};

use module_evm::ExitError;
use primitives::{evm::AddressMapping as AddressMappingT, Amount, Balance, CurrencyId};
use sp_core::H160;

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

	fn nth_param(&self, n: usize) -> Result<&[u8], Self::Error>;
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

pub struct Input<'a, Action, AccountId, AddressMapping> {
	content: &'a [u8],
	_marker: PhantomData<(Action, AccountId, AddressMapping)>,
}
impl<'a, Action, AccountId, AddressMapping> Input<'a, Action, AccountId, AddressMapping> {
	pub fn new(content: &'a [u8]) -> Self {
		Self {
			content,
			_marker: PhantomData,
		}
	}
}

impl<Action, AccountId, AddressMapping> InputT for Input<'_, Action, AccountId, AddressMapping>
where
	Action: TryFrom<u8>,
	AddressMapping: AddressMappingT<AccountId>,
{
	type Error = ExitError;
	type Action = Action;
	type AccountId = AccountId;

	fn nth_param(&self, n: usize) -> Result<&[u8], Self::Error> {
		let start = PER_PARAM_BYTES * n;
		let end = start + PER_PARAM_BYTES;

		ensure!(end <= self.content.len(), ExitError::Other("invalid input".into()));

		Ok(&self.content[start..end])
	}

	fn action(&self) -> Result<Self::Action, Self::Error> {
		let param = self.nth_param(ACTION_INDEX)?;
		let action_u8: &u8 = param.last().expect("Action bytes is 32 bytes");

		(*action_u8)
			.try_into()
			.map_err(|_| ExitError::Other("invalid action".into()))
	}

	fn account_id_at(&self, index: usize) -> Result<Self::AccountId, Self::Error> {
		let param = self.nth_param(index)?;

		let mut address = [0u8; 20];
		address.copy_from_slice(&param[12..]);

		Ok(AddressMapping::get_account_id(&address.into()))
	}

	fn evm_address_at(&self, index: usize) -> Result<H160, Self::Error> {
		let param = self.nth_param(index)?;

		let mut address = [0u8; 20];
		address.copy_from_slice(&param[12..]);

		Ok(H160::from_slice(&address))
	}

	fn currency_id_at(&self, index: usize) -> Result<CurrencyId, Self::Error> {
		let param = self.nth_param(index)?;

		let bytes: &[u8; 32] = param
			.try_into()
			.map_err(|_| ExitError::Other("currency id param bytes too short".into()))?;

		(*bytes)
			.try_into()
			.map_err(|_| ExitError::Other("invalid currency id".into()))
	}

	fn balance_at(&self, index: usize) -> Result<Balance, Self::Error> {
		let param = self.nth_param(index)?;

		let mut balance = [0u8; BALANCE_BYTES];
		let start = PER_PARAM_BYTES - BALANCE_BYTES;
		balance[..].copy_from_slice(&param[start..]);

		Ok(Balance::from_be_bytes(balance))
	}

	fn amount_at(&self, index: usize) -> Result<Amount, Self::Error> {
		let param = self.nth_param(index)?;

		let mut amount = [0u8; AMOUNT_BYTES];
		let start = PER_PARAM_BYTES - AMOUNT_BYTES;
		amount[..].copy_from_slice(&param[start..]);

		Ok(Amount::from_be_bytes(amount))
	}

	fn u64_at(&self, index: usize) -> Result<u64, Self::Error> {
		let param = self.nth_param(index)?;

		let mut num = [0u8; U64_BYTES];
		let start = PER_PARAM_BYTES - U64_BYTES;
		num[..].copy_from_slice(&param[start..]);

		Ok(u64::from_be_bytes(num))
	}

	fn u32_at(&self, index: usize) -> Result<u32, Self::Error> {
		let param = self.nth_param(index)?;

		let mut num = [0u8; U32_BYTES];
		let start = PER_PARAM_BYTES - U32_BYTES;
		num[..].copy_from_slice(&param[start..]);

		Ok(u32::from_be_bytes(num))
	}

	fn bytes_at(&self, start: usize, len: usize) -> Result<Vec<u8>, Self::Error> {
		let end = start + len;

		ensure!(
			end <= self.content.len(),
			ExitError::Other("invalid bytes input".into())
		);

		let bytes = &self.content[start..end];

		Ok(bytes.to_vec())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use frame_support::{assert_err, assert_ok};
	use sp_core::H160;

	use primitives::{mocks::MockAddressMapping, AccountId, CurrencyId, TokenSymbol};

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

	pub type TestInput<'a> = Input<'a, Action, AccountId, MockAddressMapping>;

	#[test]
	fn nth_param_works() {
		let input = TestInput::new(&[1u8; 64][..]);
		assert_ok!(input.nth_param(1), &[1u8; 32][..]);
		assert_err!(input.nth_param(2), ExitError::Other("invalid input".into()));
	}

	#[test]
	fn action_works() {
		let input = TestInput::new(&[0u8; 32][..]);
		assert_ok!(input.action(), Action::QueryBalance);

		let mut raw_input = [0u8; 32];
		raw_input[31] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.action(), Action::Transfer);

		let mut raw_input = [0u8; 32];
		raw_input[31] = 2;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.action(), Action::Unknown);
	}

	#[test]
	fn account_id_works() {
		let mut address = [0u8; 20];
		address[19] = 1;
		let account_id = MockAddressMapping::get_account_id(&address.into());

		let mut raw_input = [0u8; 32];
		raw_input[31] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.account_id_at(0), account_id);
	}

	#[test]
	fn evm_address_works() {
		let mut address = [0u8; 20];
		address[19] = 1;
		let evm_address = H160::from_slice(&address);

		let mut raw_input = [0u8; 32];
		raw_input[31] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.evm_address_at(0), evm_address);
	}

	#[test]
	fn currency_id_works() {
		let input = TestInput::new(&[0u8; 32][..]);
		assert_ok!(input.currency_id_at(0), CurrencyId::Token(TokenSymbol::ACA));

		let mut raw_input = [0u8; 32];
		raw_input[30] = 1;
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.currency_id_at(0), CurrencyId::Token(TokenSymbol::AUSD));
	}

	#[test]
	fn balance_works() {
		let balance = 127u128;
		let balance_bytes = balance.to_be_bytes();

		let mut raw_input = [0u8; 32];
		raw_input[16..].copy_from_slice(&balance_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.balance_at(0), balance);
	}

	#[test]
	fn amount_works() {
		let amount = 127i128;
		let amount_bytes = amount.to_be_bytes();

		let mut raw_input = [0u8; 32];
		raw_input[16..].copy_from_slice(&amount_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.amount_at(0), amount);
	}

	#[test]
	fn u64_works() {
		let u64_num = 127u64;
		let u64_bytes = u64_num.to_be_bytes();

		let mut raw_input = [0u8; 32];
		raw_input[24..].copy_from_slice(&u64_bytes);
		let input = TestInput::new(&raw_input[..]);
		assert_ok!(input.u64_at(0), u64_num);
	}
}
