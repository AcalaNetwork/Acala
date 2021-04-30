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

use frame_support::{log, sp_runtime::FixedPointNumber};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use num_enum::TryFromPrimitive;
use primitives::CurrencyId;
use sp_core::U256;
use sp_std::{fmt::Debug, marker::PhantomData, prelude::*, result};

use super::input::{Input, InputT};
use module_support::{
	AddressMapping as AddressMappingT, CurrencyIdMapping as CurrencyIdMappingT, Price, PriceProvider as PriceProviderT,
};

/// The `Oracle` impl precompile.
///
///
/// `input` data starts with `action`.
///
/// Actions:
/// - Get price. Rest `input` bytes: `currency_id`.
pub struct OraclePrecompile<AccountId, AddressMapping, CurrencyIdMapping, PriceProvider>(
	PhantomData<(AccountId, AddressMapping, CurrencyIdMapping, PriceProvider)>,
);

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(u8)]
enum Action {
	GetPrice = 0,
}

impl<AccountId, AddressMapping, CurrencyIdMapping, PriceProvider> Precompile
	for OraclePrecompile<AccountId, AddressMapping, CurrencyIdMapping, PriceProvider>
where
	AccountId: Debug + Clone,
	AddressMapping: AddressMappingT<AccountId>,
	CurrencyIdMapping: CurrencyIdMappingT,
	PriceProvider: PriceProviderT<CurrencyId>,
{
	fn execute(
		input: &[u8],
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		//TODO: evaluate cost

		log::debug!(target: "evm", "input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping, CurrencyIdMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::GetPrice => {
				let key = input.currency_id_at(1)?;
				let value = PriceProvider::get_price(key).unwrap_or_else(Default::default);
				log::debug!(target: "evm", "oracle currency_id: {:?}, price: {:?}", key, value);
				Ok((ExitSucceed::Returned, vec_u8_from_price(value), 0))
			}
		}
	}
}

fn vec_u8_from_price(value: Price) -> Vec<u8> {
	let mut be_bytes = [0u8; 32];
	U256::from(value.into_inner()).to_big_endian(&mut be_bytes[..32]);
	be_bytes.to_vec()
}
