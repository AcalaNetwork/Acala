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

//! The precompiles for EVM, includes standard Ethereum precompiles, and more:
//! - MultiCurrency at address `H160::from_low_u64_be(1024)`.

#![allow(clippy::upper_case_acronyms)]

mod mock;
mod tests;

use frame_support::log;
use module_evm::{
	precompiles::{
		ECRecover, ECRecoverPublicKey, EvmPrecompiles, Identity, Precompile, PrecompileSet, Ripemd160, Sha256,
		Sha3FIPS256, Sha3FIPS512,
	},
	runner::state::PrecompileOutput,
	Context, ExitError,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
use primitives::evm::{is_acala_precompile, PRECOMPILE_ADDRESS_START};
use sp_core::H160;
use sp_std::marker::PhantomData;

pub mod dex;
pub mod input;
pub mod multicurrency;
pub mod nft;
pub mod oracle;
pub mod schedule_call;
pub mod state_rent;

use crate::SystemContractsFilter;
pub use dex::DexPrecompile;
pub use multicurrency::MultiCurrencyPrecompile;
pub use nft::NFTPrecompile;
pub use oracle::OraclePrecompile;
pub use schedule_call::ScheduleCallPrecompile;
pub use state_rent::StateRentPrecompile;

pub struct AllPrecompiles<R>(PhantomData<R>);

impl<R> PrecompileSet for AllPrecompiles<R>
where
	R: module_evm::Config,
	MultiCurrencyPrecompile<R>: Precompile,
	NFTPrecompile<R>: Precompile,
	StateRentPrecompile<R>: Precompile,
	OraclePrecompile<R>: Precompile,
	DexPrecompile<R>: Precompile,
	ScheduleCallPrecompile<R>: Precompile,
{
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<PrecompileOutput, ExitError>> {
		EvmPrecompiles::<ECRecover, Sha256, Ripemd160, Identity, ECRecoverPublicKey, Sha3FIPS256, Sha3FIPS512>::execute(
			address, input, target_gas, context,
		)
		.or_else(|| {
			if !is_acala_precompile(address) {
				return None;
			}

			if !SystemContractsFilter::is_allowed(context.caller) {
				log::debug!(target: "evm", "Precompile no permission");
				return Some(Err(ExitError::Other("no permission".into())));
			}

			log::debug!(target: "evm", "Precompile begin, address: {:?}, input: {:?}, target_gas: {:?}, context: {:?}", address, input, target_gas, context);

			let result = if address == PRECOMPILE_ADDRESS_START {
				Some(MultiCurrencyPrecompile::<R>::execute(input, target_gas, context))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(1) {
				Some(NFTPrecompile::<R>::execute(input, target_gas, context))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(2) {
				Some(StateRentPrecompile::<R>::execute(input, target_gas, context))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(3) {
				Some(OraclePrecompile::<R>::execute(input, target_gas, context))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(4) {
				Some(ScheduleCallPrecompile::<R>::execute(input, target_gas, context))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(5) {
				Some(DexPrecompile::<R>::execute(input, target_gas, context))
			} else {
				None
			};

			log::debug!(target: "evm", "Precompile end, result: {:?}", result);
			result
		})
	}
}
