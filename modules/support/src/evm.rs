use codec::FullCodec;

use pallet_evm::{AddressMapping, ExitError, ExitSucceed, Precompile};
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_std::{convert::TryInto, fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::MultiCurrency;

/// The `MultiCurrency` impl precompile.
///
/// The 1st byte of `input` indicates action.
/// - `0`: Query total issuance. In this case, the 2nd byte is `currency_id`.
/// - `1`: Query balance. In this case, the 2nd byte is `currency_id`, and the
///   rest 20 bytes is `account_id`. In total the `input` should be 22 bytes.
/// - `2`: Transfer. In this case, the 2nd byte is `currency_id`, and then 20
///   bytes as `from`, which follows 20 bytes as `to`, and the rest 16 bytes as
///   `amount`. In total the `input` should be 58 bytes.
pub struct MultiCurrencyPrecompile<AccountId, AccountIdConverter, CurrencyId, MC>(
	PhantomData<(AccountId, AccountIdConverter, CurrencyId, MC)>,
);

enum Action {
	QueryTotalIssuance,
	QueryBalance,
	Transfer,
	Unknown,
}

impl From<u8> for Action {
	fn from(a: u8) -> Self {
		match a {
			0 => Action::QueryTotalIssuance,
			1 => Action::QueryBalance,
			2 => Action::Transfer,
			_ => Action::Unknown,
		}
	}
}

impl<AccountId, AccountIdConverter, CurrencyId, MC> Precompile
	for MultiCurrencyPrecompile<AccountId, AccountIdConverter, CurrencyId, MC>
where
	AccountIdConverter: AddressMapping<AccountId>,
	CurrencyId: FullCodec + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug + From<u8>,
	MC: MultiCurrency<AccountId, CurrencyId = CurrencyId>,
{
	fn execute(input: &[u8], _target_gas: Option<usize>) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		//TODO: evaluate cost

		if input.len() <= 1 {
			return Err(ExitError::Other("invalid input"));
		}

		let action: Action = input[0].into();
		let currency_id: CurrencyId = input[1].into();

		match action {
			Action::QueryTotalIssuance => {
				let total_issuance = vec_u8_from_balance(MC::total_issuance(currency_id))?;
				Ok((ExitSucceed::Returned, total_issuance, 0))
			}
			Action::QueryBalance => {
				if input.len() != 22 {
					return Err(ExitError::Other("invalid input"));
				}

				let who = account_id_from_slice::<_, AccountIdConverter>(&input[2..23]);
				let balance = vec_u8_from_balance(MC::total_balance(currency_id, &who))?;

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::Transfer => {
				if input.len() != 58 {
					return Err(ExitError::Other("invalid input"));
				}

				let from = account_id_from_slice::<_, AccountIdConverter>(&input[2..22]);
				let to = account_id_from_slice::<_, AccountIdConverter>(&input[22..42]);
				let mut amount_bytes = [0u8; 16];
				amount_bytes[..].copy_from_slice(&input[42..]);
				let amount = u128::from_be_bytes(amount_bytes)
					.try_into()
					.map_err(|_| ExitError::Other("u128 to balance failed"))?;

				MC::transfer(currency_id, &from, &to, amount).map_err(|e| ExitError::Other(e.into()))?;

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action")),
		}
	}
}

fn account_id_from_slice<AccountId, AccountIdConverter: AddressMapping<AccountId>>(src: &[u8]) -> AccountId {
	let mut address = [0u8; 20];
	address[..].copy_from_slice(src);
	AccountIdConverter::into_account_id(address.into())
}

fn vec_u8_from_balance<Balance: TryInto<u128>>(b: Balance) -> result::Result<Vec<u8>, ExitError> {
	let balance = b.try_into().map_err(|_| ExitError::Other("balance to u128 failed"))?;
	Ok(balance.to_be_bytes().to_vec())
}
