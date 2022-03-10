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

//! Builtin precompiles.

use crate::runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult};
use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
use sp_std::vec::Vec;

mod blake2;
mod bn128;
mod ecrecover;
mod ecrecover_publickey;
mod identity;
mod modexp;
mod ripemd;
mod sha256;
mod sha3fips;

pub use blake2::Blake2F;
pub use bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
pub use ecrecover::ECRecover;
pub use ecrecover_publickey::ECRecoverPublicKey;
pub use identity::Identity;
pub use modexp::{IstanbulModexp, Modexp};
pub use ripemd::Ripemd160;
pub use sha256::Sha256;
pub use sha3fips::{Sha3FIPS256, Sha3FIPS512};

/// One single precompile used by EVM engine.
pub trait Precompile {
	/// Try to execute the precompile. Calculate the amount of gas needed with given `input` and
	/// `target_gas`. Return `Ok(status, output, gas_used)` if the execution is
	/// successful. Otherwise return `Err(_)`.
	fn execute(input: &[u8], target_gas: Option<u64>, context: &Context, is_static: bool) -> PrecompileResult;
}

pub trait LinearCostPrecompile {
	const BASE: u64;
	const WORD: u64;

	fn execute(input: &[u8], cost: u64) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure>;
}

impl<T: LinearCostPrecompile> Precompile for T {
	fn execute(input: &[u8], target_gas: Option<u64>, _: &Context, _: bool) -> PrecompileResult {
		let cost = ensure_linear_cost(target_gas, input.len() as u64, T::BASE, T::WORD)?;

		let (exit_status, output) = T::execute(input, cost)?;
		Ok(PrecompileOutput {
			exit_status,
			cost,
			output,
			logs: Default::default(),
		})
	}
}

/// Linear gas cost
fn ensure_linear_cost(target_gas: Option<u64>, len: u64, base: u64, word: u64) -> Result<u64, PrecompileFailure> {
	let cost = base
		.checked_add(
			word.checked_mul(len.saturating_add(31) / 32)
				.ok_or(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				})?,
		)
		.ok_or(PrecompileFailure::Error {
			exit_status: ExitError::OutOfGas,
		})?;

	if let Some(target_gas) = target_gas {
		if cost > target_gas {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			});
		}
	}

	Ok(cost)
}
