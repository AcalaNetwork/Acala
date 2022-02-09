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

use crate::precompile::PrecompileOutput;
use frame_support::{
	log,
	traits::{Currency, Get},
};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use module_support::Erc20InfoMapping as Erc20InfoMappingT;
use sp_runtime::RuntimeDebug;
use sp_std::{marker::PhantomData, prelude::*, result};

use orml_traits::MultiCurrency as MultiCurrencyT;

use super::input::{Input, InputT, Output};
use num_enum::{IntoPrimitive, TryFromPrimitive};

/// The `MultiCurrency` impl precompile.
///
///
/// `input` data starts with `action` and `currency_id`.
///
/// Actions:
/// - Query total issuance.
/// - Query balance. Rest `input` bytes: `account_id`.
/// - Transfer. Rest `input` bytes: `from`, `to`, `amount`.
pub struct MultiCurrencyPrecompile<R>(PhantomData<R>);

#[module_evm_utiltity_macro::generate_function_selector]
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

impl<Runtime> Precompile for MultiCurrencyPrecompile<Runtime>
where
	Runtime: module_evm::Config + module_prices::Config + module_transaction_payment::Config,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, Runtime::AccountId, Runtime::AddressMapping, Runtime::Erc20InfoMapping>::new(input);

		let action = input.action()?;
		let currency_id = Runtime::Erc20InfoMapping::decode_evm_address(context.caller)
			.ok_or_else(|| ExitError::Other("invalid currency id".into()))?;

		log::debug!(target: "evm", "multicurrency: currency id: {:?}", currency_id);

		match action {
			Action::QueryName => {
				let name = Runtime::Erc20InfoMapping::name(currency_id)
					.ok_or_else(|| ExitError::Other("Get name failed".into()))?;
				log::debug!(target: "evm", "multicurrency: name: {:?}", name);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_bytes(&name),
					logs: Default::default(),
				})
			}
			Action::QuerySymbol => {
				let symbol = Runtime::Erc20InfoMapping::symbol(currency_id)
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
				let decimals = Runtime::Erc20InfoMapping::decimals(currency_id)
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
				let total_issuance =
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_issuance(currency_id);
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
				let balance = if currency_id == <Runtime as module_transaction_payment::Config>::NativeCurrencyId::get()
				{
					<Runtime as module_evm::Config>::Currency::free_balance(&who)
				} else {
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_balance(currency_id, &who)
				};
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

				Runtime::MultiCurrency::transfer(currency_id, &from, &to, amount).map_err(|e| {
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
