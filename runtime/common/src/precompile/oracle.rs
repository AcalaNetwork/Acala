use frame_support::{debug, sp_runtime::FixedPointNumber};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use primitives::{evm::AddressMapping as AddressMappingT, CurrencyId, Moment};
use sp_core::U256;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::DataProviderExtended as OracleT;

use super::input::{Input, InputT};
use module_support::Price;
use orml_oracle::TimestampedValue;

/// The `Oracle` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get price. Rest `input` bytes: `currency_id`.
pub struct OraclePrecompile<AccountId, AddressMapping, Oracle>(PhantomData<(AccountId, AddressMapping, Oracle)>);

enum Action {
	GetPrice,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::GetPrice,
			_ => Action::Unknown,
		}
	}
}

impl<AccountId, AddressMapping, Oracle> Precompile for OraclePrecompile<AccountId, AddressMapping, Oracle>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	Oracle: OracleT<CurrencyId, TimestampedValue<Price, Moment>>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<usize>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		//TODO: evaluate cost

		debug::debug!(target: "evm", "input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::GetPrice => {
				let key = input.currency_id_at(1)?;
				let value = Oracle::get_no_op(&key).ok_or_else(|| ExitError::Other("no data".into()))?;
				Ok((ExitSucceed::Returned, vec_u8_from_timestamped(value), 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}

fn vec_u8_from_timestamped(value: TimestampedValue<Price, Moment>) -> Vec<u8> {
	let mut be_bytes = [0u8; 64];
	U256::from(value.value.into_inner()).to_big_endian(&mut be_bytes[..32]);
	U256::from(value.timestamp).to_big_endian(&mut be_bytes[32..64]);
	be_bytes.to_vec()
}
