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

use frame_support::log;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use primitives::evm::AddressMapping as AddressMappingT;
use sp_core::U256;
use sp_std::{convert::TryFrom, fmt::Debug, marker::PhantomData, prelude::*, result};

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
}

impl TryFrom<u8> for Action {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Action::QueryTotalIssuance),
			1 => Ok(Action::QueryBalance),
			2 => Ok(Action::Transfer),
			_ => Err(()),
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
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		//TODO: evaluate cost

		log::debug!(target: "evm", "input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;
		let currency_id = input.currency_id_at(1)?;

		log::debug!(target: "evm", "currency id: {:?}", currency_id);

		match action {
			Action::QueryTotalIssuance => {
				let total_issuance = vec_u8_from_balance(MultiCurrency::total_issuance(currency_id));
				log::debug!(target: "evm", "total issuance: {:?}", total_issuance);

				Ok((ExitSucceed::Returned, total_issuance, 0))
			}
			Action::QueryBalance => {
				let who = input.account_id_at(2)?;
				log::debug!(target: "evm", "who: {:?}", who);

				let balance = vec_u8_from_balance(MultiCurrency::total_balance(currency_id, &who));
				log::debug!(target: "evm", "balance: {:?}", balance);

				Ok((ExitSucceed::Returned, balance, 0))
			}
			Action::Transfer => {
				let from = input.account_id_at(2)?;
				let to = input.account_id_at(3)?;
				let amount = input.balance_at(4)?;

				log::debug!(target: "evm", "from: {:?}", from);
				log::debug!(target: "evm", "to: {:?}", to);
				log::debug!(target: "evm", "amount: {:?}", amount);

				MultiCurrency::transfer(currency_id, &from, &to, amount).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				log::debug!(target: "evm", "transfer success!");

				Ok((ExitSucceed::Returned, vec![], 0))
			}
		}
	}
}

fn vec_u8_from_balance(balance: Balance) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(balance).to_big_endian(&mut be_bytes[..]);
	be_bytes.to_vec()
}
