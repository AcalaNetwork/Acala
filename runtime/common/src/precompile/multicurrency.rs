use frame_support::debug;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use primitives::evm::AddressMapping as AddressMappingT;
use sp_core::U256;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::MultiCurrency as MultiCurrencyT;

use super::input::{Input, InputT};
use primitives::{Balance, CurrencyId};

/// The `MultiCurrency` impl precompile.
///
///
/// `input` data starts with `action` and `currency_id`.
///
/// Actions:
/// - Query total issuance.
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Transfer. Rest `input` bytes: `from`, `to`, `amount`.
pub struct MultiCurrencyPrecompile<AccountId, AddressMapping, MultiCurrency>(
	PhantomData<(AccountId, AddressMapping, MultiCurrency)>,
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

impl<AccountId, AddressMapping, MultiCurrency> Precompile
	for MultiCurrencyPrecompile<AccountId, AddressMapping, MultiCurrency>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	MultiCurrency: MultiCurrencyT<AccountId, Balance = Balance, CurrencyId = CurrencyId>,
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
		let currency_id = input.currency_id_at(1)?;

		debug::debug!(target: "evm", "currency id: {:?}", currency_id);

		match action {
			Action::QueryTotalIssuance => {
				let total_issuance = vec_u8_from_balance(MultiCurrency::total_issuance(currency_id));
				debug::debug!(target: "evm", "total issuance: {:?}", total_issuance);

				Ok((ExitSucceed::Returned, total_issuance, 0))
			}
			Action::QueryBalance => {
				let who = input.account_id_at(2)?;
				debug::debug!(target: "evm", "who: {:?}", who);

				let balance = vec_u8_from_balance(MultiCurrency::total_balance(currency_id, &who));
				debug::debug!(target: "evm", "balance: {:?}", balance);

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::Transfer => {
				let from = input.account_id_at(2)?;
				let to = input.account_id_at(3)?;
				let amount = input.balance_at(4)?;

				debug::debug!(target: "evm", "from: {:?}", from);
				debug::debug!(target: "evm", "to: {:?}", to);
				debug::debug!(target: "evm", "amount: {:?}", amount);

				MultiCurrency::transfer(currency_id, &from, &to, amount).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				debug::debug!(target: "evm", "transfer success!");

				Ok((ExitSucceed::Returned, vec![], 0))
			}
			Action::Unknown => Err(ExitError::Other("unknown action".into())),
		}
	}
}

fn vec_u8_from_balance(balance: Balance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(balance).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}
