use codec::FullCodec;

use frame_support::debug;
use pallet_evm::{AddressMapping, ExitError, ExitSucceed, Precompile};
use sp_core::U256;
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_std::{convert::TryInto, fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::MultiCurrency;

/// The `MultiCurrency` impl precompile.
///
///
/// All `input` data start with `action_byte ++ currency_id_bytes`;
///
/// The 1st byte of `input` indicates action.
/// - `0`: Query total issuance.
/// - `1`: Query balance. Rest: `account_id`.
/// - `2`: Transfer. Rest: `from ++ to ++ amount`.
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
	AccountId: Debug + Clone,
	AccountIdConverter: AddressMapping<AccountId>,
	CurrencyId: FullCodec + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug,
	MC: MultiCurrency<AccountId, CurrencyId = CurrencyId>,
{
	fn execute(input: &[u8], _target_gas: Option<usize>) -> result::Result<(ExitSucceed, Vec<u8>, usize), ExitError> {
		//TODO: evaluate cost

		debug::info!("----------------------------------------------------------------");
		debug::info!(">>> input: {:?}", input);

		if input.len() < 5 {
			return Err(ExitError::Other("invalid input"));
		}

		let action: Action = input[0].into();

		let mut currency_id_bytes = [0u8; 4];
		currency_id_bytes[..].copy_from_slice(&input[1..5]);
		let currency_id: CurrencyId =
			CurrencyId::decode(&mut &currency_id_bytes[..]).map_err(|_| ExitError::Other("invalid currency"))?;

		debug::info!(">>> currency id: {:?}", currency_id);

		match action {
			Action::QueryTotalIssuance => {
				let total_issuance = vec_u8_from_balance(MC::total_issuance(currency_id))?;
				Ok((ExitSucceed::Returned, total_issuance, 0))
			}
			Action::QueryBalance => {
				// 32 * 2
				if input.len() < 64 {
					return Err(ExitError::Other("invalid input"));
				}

				let who = account_id_from_slice::<_, AccountIdConverter>(&input[32..52]);
				let balance = vec_u8_from_balance(MC::total_balance(currency_id, &who))?;

				debug::info!(">>> account id: {:?}", who);
				debug::info!(">>> balance: {:?}", balance);

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::Transfer => {
				// 32 * 4
				if input.len() < 128 {
					return Err(ExitError::Other("invalid input"));
				}

				let from = account_id_from_slice::<_, AccountIdConverter>(&input[32..52]);
				let to = account_id_from_slice::<_, AccountIdConverter>(&input[64..84]);
				let mut amount_bytes = [0u8; 16];
				amount_bytes[..].copy_from_slice(&input[96..112]);
				let amount = u128::from_be_bytes(amount_bytes)
					.try_into()
					.map_err(|_| ExitError::Other("u128 to balance failed"))?;

				debug::info!(">>> from: {:?}", from);
				debug::info!(">>> to: {:?}", to);
				debug::info!(">>> amount: {:?}", amount);

				MC::transfer(currency_id, &from, &to, amount).map_err(|e| ExitError::Other(e.into()))?;

				debug::info!(">>> transfer success!");

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
	let mut be_bytes = [0u8; 32];
	U256::from(balance).to_big_endian(&mut be_bytes[..]);
	Ok(be_bytes.to_vec())
}
