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

//! Builtin precompiles.

use evm::{Context, ExitError, ExitSucceed};
use impl_trait_for_tuples::impl_for_tuples;
use primitive_types::H160;
use ripemd160::Digest;
use sp_runtime::SaturatedConversion;
use sp_std::{cmp::min, marker::PhantomData, vec::Vec};
use tiny_keccak::Hasher;

/// Custom precompiles to be used by EVM engine.
pub trait Precompiles {
	#![allow(clippy::type_complexity)]
	/// Try to execute the code address as precompile. If the code address is
	/// not a precompile or the precompile is not yet available, return `None`.
	/// Otherwise, calculate the amount of gas needed with given `input` and
	/// `target_gas`. Return `Some(Ok(status, output, gas_used))` if the
	/// execution is successful. Otherwise return `Some(Err(_))`.
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError>>;
}

/// One single precompile used by EVM engine.
pub trait Precompile {
	/// Try to execute the precompile. Calculate the amount of gas needed with
	/// given `input` and `target_gas`. Return `Ok(status, output, gas_used)` if
	/// the execution is successful. Otherwise return `Err(_)`.
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError>;
}

#[impl_for_tuples(16)]
#[tuple_types_no_default_trait_bound]
impl Precompiles for Tuple {
	for_tuples!( where #( Tuple: Precompile )* );
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
		let mut index = 0;

		for_tuples!( #(
			index += 1;
			if address == H160::from_low_u64_be(index) {
				return Some(Tuple::execute(input, target_gas, context))
			}
		)* );

		None
	}
}

pub struct EvmPrecompiles<ECRecover, Sha256, Ripemd160, Identity, ECRecoverPublicKey, Sha3FIPS256, Sha3FIPS512>(
	PhantomData<(
		ECRecover,
		Sha256,
		Ripemd160,
		Identity,
		ECRecoverPublicKey,
		Sha3FIPS256,
		Sha3FIPS512,
	)>,
);

impl<ECRecover, Sha256, Ripemd160, Identity, ECRecoverPublicKey, Sha3FIPS256, Sha3FIPS512> Precompiles
	for EvmPrecompiles<ECRecover, Sha256, Ripemd160, Identity, ECRecoverPublicKey, Sha3FIPS256, Sha3FIPS512>
where
	ECRecover: Precompile,
	Sha256: Precompile,
	Ripemd160: Precompile,
	Identity: Precompile,
	ECRecoverPublicKey: Precompile,
	Sha3FIPS256: Precompile,
	Sha3FIPS512: Precompile,
{
	#[allow(clippy::type_complexity)]
	fn execute(
		address: H160,
		input: &[u8],
		target_gas: Option<u64>,
		context: &Context,
	) -> Option<core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError>> {
		// https://github.com/ethereum/go-ethereum/blob/9357280fce5c5d57111d690a336cca5f89e34da6/core/vm/contracts.go#L83
		if address == H160::from_low_u64_be(1) {
			Some(ECRecover::execute(input, target_gas, context))
		} else if address == H160::from_low_u64_be(2) {
			Some(Sha256::execute(input, target_gas, context))
		} else if address == H160::from_low_u64_be(3) {
			Some(Ripemd160::execute(input, target_gas, context))
		} else if address == H160::from_low_u64_be(4) {
			Some(Identity::execute(input, target_gas, context))
		}
		// Non-standard precompile starts with 128
		else if address == H160::from_low_u64_be(128) {
			Some(ECRecoverPublicKey::execute(input, target_gas, context))
		} else if address == H160::from_low_u64_be(129) {
			Some(Sha3FIPS256::execute(input, target_gas, context))
		} else if address == H160::from_low_u64_be(130) {
			Some(Sha3FIPS512::execute(input, target_gas, context))
		} else {
			None
		}
	}
}

/// Linear gas cost
fn ensure_linear_cost(target_gas: Option<u64>, len: usize, base: usize, word: usize) -> Result<u64, ExitError> {
	let cost: u64 = base
		.checked_add(
			word.checked_mul(len.saturating_add(31) / 32)
				.ok_or(ExitError::OutOfGas)?,
		)
		.ok_or(ExitError::OutOfGas)?
		.saturated_into();

	if let Some(target_gas) = target_gas {
		if cost > target_gas {
			return Err(ExitError::OutOfGas);
		}
	}

	Ok(cost.saturated_into())
}

/// The identity precompile.
pub struct Identity;

impl Precompile for Identity {
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, input.len(), 15, 3)?;

		Ok((ExitSucceed::Returned, input.to_vec(), cost))
	}
}

/// The ecrecover precompile.
pub struct ECRecover;

impl Precompile for ECRecover {
	fn execute(
		i: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, i.len(), 3000, 0)?;

		let mut input = [0u8; 128];
		input[..min(i.len(), 128)].copy_from_slice(&i[..min(i.len(), 128)]);

		let mut msg = [0u8; 32];
		let mut sig = [0u8; 65];

		msg[0..32].copy_from_slice(&input[0..32]);
		sig[0..32].copy_from_slice(&input[64..96]);
		sig[32..64].copy_from_slice(&input[96..128]);
		sig[64] = input[63];

		let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg)
			.map_err(|_| ExitError::Other("Public key recover failed".into()))?;
		let mut address = sp_io::hashing::keccak_256(&pubkey);
		address[0..12].copy_from_slice(&[0u8; 12]);

		Ok((ExitSucceed::Returned, address.to_vec(), cost))
	}
}

/// The ripemd precompile.
pub struct Ripemd160;

impl Precompile for Ripemd160 {
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, input.len(), 600, 120)?;

		let mut ret = [0u8; 32];
		ret[12..32].copy_from_slice(&ripemd160::Ripemd160::digest(input));
		Ok((ExitSucceed::Returned, ret.to_vec(), cost))
	}
}

/// The sha256 precompile.
pub struct Sha256;

impl Precompile for Sha256 {
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, input.len(), 60, 12)?;

		let ret = sp_io::hashing::sha2_256(input);
		Ok((ExitSucceed::Returned, ret.to_vec(), cost))
	}
}

/// The ecrecover precompile.
pub struct ECRecoverPublicKey;

impl Precompile for ECRecoverPublicKey {
	fn execute(
		i: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, i.len(), 3000, 0)?;

		let mut input = [0u8; 128];
		input[..min(i.len(), 128)].copy_from_slice(&i[..min(i.len(), 128)]);

		let mut msg = [0u8; 32];
		let mut sig = [0u8; 65];

		msg[0..32].copy_from_slice(&input[0..32]);
		sig[0..32].copy_from_slice(&input[64..96]);
		sig[32..64].copy_from_slice(&input[96..128]);
		sig[64] = input[63];

		let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg)
			.map_err(|_| ExitError::Other("Public key recover failed".into()))?;

		Ok((ExitSucceed::Returned, pubkey.to_vec(), cost))
	}
}

/// The Sha3FIPS256 precompile.
pub struct Sha3FIPS256;

impl Precompile for Sha3FIPS256 {
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, input.len(), 60, 12)?;

		let mut output = [0; 32];
		let mut sha3 = tiny_keccak::Sha3::v256();
		sha3.update(input);
		sha3.finalize(&mut output);
		Ok((ExitSucceed::Returned, output.to_vec(), cost))
	}
}

/// The Sha3FIPS512 precompile.
pub struct Sha3FIPS512;

impl Precompile for Sha3FIPS512 {
	fn execute(
		input: &[u8],
		target_gas: Option<u64>,
		_context: &Context,
	) -> core::result::Result<(ExitSucceed, Vec<u8>, u64), ExitError> {
		let cost = ensure_linear_cost(target_gas, input.len(), 60, 12)?;

		let mut output = [0; 64];
		let mut sha3 = tiny_keccak::Sha3::v512();
		sha3.update(input);
		sha3.finalize(&mut output);
		Ok((ExitSucceed::Returned, output.to_vec(), cost))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn sha3_ipfs_256_should_works() -> std::result::Result<(), ExitError> {
		let input = b"hello";
		let expected = b"\
			\x33\x38\xbe\x69\x4f\x50\xc5\xf3\x38\x81\x49\x86\xcd\xf0\x68\x64\
			\x53\xa8\x88\xb8\x4f\x42\x4d\x79\x2a\xf4\xb9\x20\x23\x98\xf3\x92\
		";

		match Sha3FIPS256::execute(
			input,
			None,
			&Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			},
		) {
			Ok((_, out, _)) => {
				assert_eq!(out, expected);
				Ok(())
			}
			Err(e) => {
				panic!("Test not expected to fail: {:?}", e);
			}
		}
	}

	#[test]
	fn sha3_ipfs_512_should_works() -> std::result::Result<(), ExitError> {
		let input = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.";
		let expected = b"\
			\xf3\x2a\x94\x23\x55\x13\x51\xdf\x0a\x07\xc0\xb8\xc2\x0e\xb9\x72\
			\x36\x7c\x39\x8d\x61\x06\x60\x38\xe1\x69\x86\x44\x8e\xbf\xbc\x3d\
			\x15\xed\xe0\xed\x36\x93\xe3\x90\x5e\x9a\x8c\x60\x1d\x9d\x00\x2a\
			\x06\x85\x3b\x97\x97\xef\x9a\xb1\x0c\xbd\xe1\x00\x9c\x7d\x0f\x09\
		";

		match Sha3FIPS512::execute(
			input,
			None,
			&Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			},
		) {
			Ok((_, out, _)) => {
				assert_eq!(out, expected);
				Ok(())
			}
			Err(e) => {
				panic!("Test not expected to fail: {:?}", e);
			}
		}
	}
}
