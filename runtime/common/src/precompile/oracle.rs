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

use frame_support::{debug, sp_runtime::FixedPointNumber};
use module_evm::{Context, ExitError, ExitSucceed, Precompile};
use primitives::{evm::AddressMapping as AddressMappingT, CurrencyId, Moment};
use sp_core::U256;
use sp_std::{convert::TryFrom, fmt::Debug, marker::PhantomData, prelude::*, result};

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
}

impl TryFrom<u8> for Action {
	type Error = ();

	fn try_from(value: u8) -> Result<Self, Self::Error> {
		match value {
			0 => Ok(Action::GetPrice),
			_ => Err(()),
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
		_target_gas: Option<u64>,
		_context: &Context,
	) -> result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		//TODO: evaluate cost

		debug::debug!(target: "evm", "input: {:?}", input);

		let input = Input::<Action, AccountId, AddressMapping>::new(input);

		let action = input.action()?;

		match action {
			Action::GetPrice => {
				let key = input.currency_id_at(1)?;
				let value = Oracle::get_no_op(&key).unwrap_or_else(|| TimestampedValue {
					value: Default::default(),
					timestamp: Default::default(),
				});
				Ok((ExitSucceed::Returned, vec_u8_from_timestamped(value), 0))
			}
		}
	}
}

fn vec_u8_from_timestamped(value: TimestampedValue<Price, Moment>) -> Vec<u8> {
	let mut be_bytes = [0u8; 64];
	U256::from(value.value.into_inner()).to_big_endian(&mut be_bytes[..32]);
	U256::from(value.timestamp).to_big_endian(&mut be_bytes[32..64]);
	be_bytes.to_vec()
}
