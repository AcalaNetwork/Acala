//! Builtin precompiles.

use evm::{Context, ExitError, ExitSucceed};
use impl_trait_for_tuples::impl_for_tuples;
use primitive_types::H160;
use ripemd160::Digest;
use sp_runtime::SaturatedConversion;
use sp_std::{cmp::min, vec::Vec};

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
