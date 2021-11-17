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
use frame_support::{log, sp_runtime::FixedPointNumber};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use primitives::CurrencyId;
use sp_runtime::RuntimeDebug;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT, Output};
use module_support::{
	AddressMapping as AddressMappingT, Erc20InfoMapping as Erc20InfoMappingT, PriceProvider as PriceProviderT,
};

/// The `Oracle` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get price. Rest `input` bytes: `currency_id`.
pub struct OraclePrecompile<AccountId, AddressMapping, Erc20InfoMapping, PriceProvider>(
	PhantomData<(AccountId, AddressMapping, Erc20InfoMapping, PriceProvider)>,
);

#[module_evm_utiltity_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	GetPrice = "getPrice(address)",
}

impl<AccountId, AddressMapping, Erc20InfoMapping, PriceProvider> Precompile
	for OraclePrecompile<AccountId, AddressMapping, Erc20InfoMapping, PriceProvider>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	Erc20InfoMapping: Erc20InfoMappingT,
	PriceProvider: PriceProviderT<CurrencyId>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<PrecompileOutput, ExitError> {
		let input = Input::<Action, AccountId, AddressMapping, Erc20InfoMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::GetPrice => {
				let currency_id = input.currency_id_at(1)?;
				let mut price = PriceProvider::get_price(currency_id).unwrap_or_default();

				let maybe_decimals = Erc20InfoMapping::decimals(currency_id);
				let decimals = match maybe_decimals {
					Some(decimals) => decimals,
					None => {
						// If the option is none, let price = 0 to return 0.
						// Solidity should handle the situation of price 0.
						price = Default::default();
						Default::default()
					}
				};

				let maybe_adjustment_multiplier = 10u128.checked_pow((18 - decimals).into());
				let adjustment_multiplier = match maybe_adjustment_multiplier {
					Some(adjustment_multiplier) => adjustment_multiplier,
					None => {
						// If the option is none, let price = 0 to return 0.
						// Solidity should handle the situation of price 0.
						price = Default::default();
						Default::default()
					}
				};

				let output = price.into_inner().wrapping_div(adjustment_multiplier);

				log::debug!(target: "evm", "oracle: getPrice currency_id: {:?}, price: {:?}, adjustment_multiplier: {:?}, output: {:?}", currency_id, price, adjustment_multiplier, output);
				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: 0,
					output: Output::default().encode_u128(output),
					logs: Default::default(),
				})
			}
		}
	}
}
