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
	precompiles::{ECRecover, ECRecoverPublicKey, Identity, Precompile, Ripemd160, Sha256, Sha3FIPS256, Sha3FIPS512},
	runner::state::{PrecompileFailure, PrecompileResult, PrecompileSet},
	Context, ExitRevert,
};
use module_support::PrecompileCallerFilter as PrecompileCallerFilterT;
use primitives::evm::PRECOMPILE_ADDRESS_START;
use sp_core::H160;
use sp_std::marker::PhantomData;

pub mod dex;
pub mod evm;
pub mod input;
pub mod multicurrency;
pub mod nft;
pub mod oracle;
pub mod schedule;

use crate::SystemContractsFilter;
pub use dex::DEXPrecompile;
pub use evm::EVMPrecompile;
pub use multicurrency::MultiCurrencyPrecompile;
pub use nft::NFTPrecompile;
pub use oracle::OraclePrecompile;
pub use schedule::SchedulePrecompile;

#[derive(Default)]
pub struct AllPrecompiles<R>(PhantomData<R>);

impl<R> AllPrecompiles<R>
where
	R: module_evm::Config,
{
	pub fn new() -> Self {
		Self(Default::default())
	}
	pub fn used_addresses() -> sp_std::vec::Vec<H160> {
		sp_std::vec![
			H160::from_low_u64_be(1),
			H160::from_low_u64_be(2),
			H160::from_low_u64_be(3),
			H160::from_low_u64_be(4),
			// Non-standard precompile starts with 128
			H160::from_low_u64_be(128),
			H160::from_low_u64_be(129),
			H160::from_low_u64_be(130),
			// Acala precompile
			PRECOMPILE_ADDRESS_START,
			PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(1),
			PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(2),
			PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(3),
			PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(4),
			PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(5),
		]
	}
}

impl<R> PrecompileSet for AllPrecompiles<R>
where
	R: module_evm::Config,
	MultiCurrencyPrecompile<R>: Precompile,
	NFTPrecompile<R>: Precompile,
	EVMPrecompile<R>: Precompile,
	OraclePrecompile<R>: Precompile,
	DEXPrecompile<R>: Precompile,
	SchedulePrecompile<R>: Precompile,
{
	fn execute(
		&self,
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
		is_static: bool,
	) -> Option<PrecompileResult> {
		if !self.is_precompile(address) {
			return None;
		}
		log::trace!(target: "evm", "Precompile begin, address: {:?}, input: {:?}, target_gas: {:?}, context: {:?}", address, input, target_gas, context);

		// https://github.com/ethereum/go-ethereum/blob/9357280fce5c5d57111d690a336cca5f89e34da6/core/vm/contracts.go#L83
		let result = if address == H160::from_low_u64_be(1) {
			Some(ECRecover::execute(input, target_gas, context, is_static))
		} else if address == H160::from_low_u64_be(2) {
			Some(Sha256::execute(input, target_gas, context, is_static))
		} else if address == H160::from_low_u64_be(3) {
			Some(Ripemd160::execute(input, target_gas, context, is_static))
		} else if address == H160::from_low_u64_be(4) {
			Some(Identity::execute(input, target_gas, context, is_static))
		}
		// Non-standard precompile starts with 128
		else if address == H160::from_low_u64_be(128) {
			Some(ECRecoverPublicKey::execute(input, target_gas, context, is_static))
		} else if address == H160::from_low_u64_be(129) {
			Some(Sha3FIPS256::execute(input, target_gas, context, is_static))
		} else if address == H160::from_low_u64_be(130) {
			Some(Sha3FIPS512::execute(input, target_gas, context, is_static))
		}
		// Acala precompile
		else {
			if !SystemContractsFilter::is_allowed(context.caller) {
				log::debug!(target: "evm", "Precompile no permission: {:?}", context.caller);
				return Some(Err(PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "NoPermission".into(),
					cost: 0,
				}));
			}

			if address == PRECOMPILE_ADDRESS_START {
				Some(MultiCurrencyPrecompile::<R>::execute(
					input, target_gas, context, is_static,
				))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(1) {
				Some(NFTPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(2) {
				Some(EVMPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(3) {
				Some(OraclePrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(4) {
				Some(SchedulePrecompile::<R>::execute(input, target_gas, context, is_static))
			} else if address == PRECOMPILE_ADDRESS_START | H160::from_low_u64_be(5) {
				Some(DEXPrecompile::<R>::execute(input, target_gas, context, is_static))
			} else {
				None
			}
		};

		log::trace!(target: "evm", "Precompile end, address: {:?}, input: {:?}, target_gas: {:?}, context: {:?}, result: {:?}", address, input, target_gas, context, result);
		if let Some(Err(PrecompileFailure::Revert { ref output, .. })) = result {
			log::debug!(target: "evm", "Precompile failed: {:?}", core::str::from_utf8(&output.to_vec()));
		};
		result
	}

	fn is_precompile(&self, address: H160) -> bool {
		Self::used_addresses().contains(&address)
	}
}
