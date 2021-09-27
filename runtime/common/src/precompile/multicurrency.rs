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

use crate::precompile::PrecompileOutput;
use frame_support::log;
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::{AddressMapping as AddressMappingT, CurrencyIdMapping as CurrencyIdMappingT};
use sp_runtime::RuntimeDebug;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use orml_traits::MultiCurrency as MultiCurrencyT;

use super::input::{Input, InputT, Output};
use num_enum::{IntoPrimitive, TryFromPrimitive};
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
pub struct MultiCurrencyPrecompile<AccountId, AddressMapping, CurrencyIdMapping, MultiCurrency>(
	PhantomData<(AccountId, AddressMapping, CurrencyIdMapping, MultiCurrency)>,
);

#[primitives_proc_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	QueryName = "name()",
	QuerySymbol = "symbol()",
	QueryDecimals = "decimals()",
	QueryTotalIssuance = "totalSupply()",
	QueryBalance = "balanceOf(address)",
	Transfer = "transfer(address,address,uint256)",
}

impl<AccountId, AddressMapping, CurrencyIdMapping, MultiCurrency> Precompile
	for MultiCurrencyPrecompile<AccountId, AddressMapping, CurrencyIdMapping, MultiCurrency>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	CurrencyIdMapping: CurrencyIdMappingT,
	MultiCurrency: MultiCurrencyT<AccountId, Balance = Balance, CurrencyId = CurrencyId>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, AccountId, AddressMapping, CurrencyIdMapping>::new(input);

		let action = input.action()?;
		let currency_id = CurrencyIdMapping::decode_evm_address(context.caller)
			.ok_or_else(|| ExitError::Other("invalid currency id".into()))?;

		log::debug!(target: "evm", "multicurrency: currency id: {:?}", currency_id);

		match action {
			Action::QueryName => {
				let name =
					CurrencyIdMapping::name(currency_id).ok_or_else(|| ExitError::Other("Get name failed".into()))?;
				log::debug!(target: "evm", "multicurrency: name: {:?}", name);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_bytes(&name),
					logs: Default::default(),
				})
			}
			Action::QuerySymbol => {
				let symbol = CurrencyIdMapping::symbol(currency_id)
					.ok_or_else(|| ExitError::Other("Get symbol failed".into()))?;
				log::debug!(target: "evm", "multicurrency: symbol: {:?}", symbol);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_bytes(&symbol),
					logs: Default::default(),
				})
			}
			Action::QueryDecimals => {
				let decimals = CurrencyIdMapping::decimals(currency_id)
					.ok_or_else(|| ExitError::Other("Get decimals failed".into()))?;
				log::debug!(target: "evm", "multicurrency: decimals: {:?}", decimals);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u8(decimals),
					logs: Default::default(),
				})
			}
			Action::QueryTotalIssuance => {
				let total_issuance = MultiCurrency::total_issuance(currency_id);
				log::debug!(target: "evm", "multicurrency: total issuance: {:?}", total_issuance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(total_issuance),
					logs: Default::default(),
				})
			}
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;
				let balance = MultiCurrency::total_balance(currency_id, &who);
				log::debug!(target: "evm", "multicurrency: who: {:?}, balance: {:?}", who, balance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(balance),
					logs: Default::default(),
				})
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;
				let amount = input.balance_at(3)?;
				log::debug!(target: "evm", "multicurrency: transfer from: {:?}, to: {:?}, amount: {:?}", from, to, amount);

				MultiCurrency::transfer(currency_id, &from, &to, amount).map_err(|e| {
					let err_msg: &str = e.into();
					ExitError::Other(err_msg.into())
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: vec![],
					logs: Default::default(),
				})
			}
		}
	}
}
