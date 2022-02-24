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

use super::Precompile;
use crate::runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult};
use module_evm_utiltity::evm::{Context, ExitError, ExitSucceed};
use num::{BigUint, One, Zero};
use sp_core::U256;
use sp_std::{
	cmp::{max, min},
	vec::Vec,
};

const MIN_GAS_COST: u64 = 200;

struct ModexpPricer;

impl ModexpPricer {
	fn adjusted_exp_len(len: usize, exp_low: &BigUint) -> u64 {
		let bit_index = if exp_low.is_zero() {
			0
		} else {
			let bytes = exp_low.to_bytes_be();
			let length = min(32, bytes.len());
			let zeros = U256::from_big_endian(&bytes[..length]).leading_zeros() as u64;
			255 - zeros
		};
		if len <= 32 {
			bit_index
		} else {
			8 * (len as u64 - 32) + bit_index
		}
	}

	fn mult_complexity(x: u64) -> u64 {
		match x {
			x if x <= 64 => x * x,
			x if x <= 1024 => (x * x) / 4 + 96 * x - 3072,
			x => (x * x) / 16 + 480 * x - 199_680,
		}
	}

	fn cost(
		is_eip_2565: bool,
		divisor: u64,
		base_len: usize,
		exp_len: usize,
		mod_len: usize,
		exponent: &BigUint,
		target_gas: Option<u64>,
	) -> u64 {
		if is_eip_2565 {
			return Self::eip_2565_cost(divisor, base_len, mod_len, exp_len, exponent);
		}

		if mod_len.is_zero() && base_len.is_zero() {
			return 0;
		}

		let max_len = (u32::max_value() / 2) as usize;
		if base_len > max_len || mod_len > max_len || exp_len > max_len {
			return target_gas.unwrap_or(u64::MAX);
		}

		let m = max(mod_len, base_len);

		let adjusted_exp_len = Self::adjusted_exp_len(exp_len, exponent);

		let (gas, overflow) = Self::mult_complexity(m as u64).overflowing_mul(max(adjusted_exp_len, 1));
		if overflow {
			return target_gas.unwrap_or(u64::MAX);
		}

		gas / divisor
	}

	fn eip_2565_mul_complexity(base_length: usize, modulus_length: usize) -> u64 {
		let max_length = max(base_length, modulus_length) as u64;
		let words = {
			// div_ceil(max_length, 8);
			let tmp = max_length / 8;
			if (max_length % 8).is_zero() {
				tmp
			} else {
				tmp + 1
			}
		};
		words.saturating_mul(words)
	}

	fn eip_2565_iter_count(exponent_length: usize, exponent: &BigUint) -> u64 {
		let bytes = exponent.to_bytes_be();
		let length = min(32, bytes.len());
		let exponent = U256::from_big_endian(&bytes[..length]);

		let it = if exponent_length <= 32 && exponent.is_zero() {
			0
		} else if exponent_length <= 32 {
			(exponent.bits() - 1) as u64
		} else {
			// else > 32
			8u64.saturating_mul(exponent_length as u64 - 32)
				.saturating_add(exponent.bits().saturating_sub(1) as u64)
		};
		max(it, 1)
	}

	fn eip_2565_cost(
		divisor: u64,
		base_length: usize,
		modulus_length: usize,
		exponent_length: usize,
		exponent: &BigUint,
	) -> u64 {
		let multiplication_complexity = Self::eip_2565_mul_complexity(base_length, modulus_length);
		let iteration_count = Self::eip_2565_iter_count(exponent_length, exponent);
		max(
			MIN_GAS_COST,
			multiplication_complexity.saturating_mul(iteration_count) / divisor,
		)
	}
}

// ModExp expects the following as inputs:
// 1) 32 bytes expressing the length of base
// 2) 32 bytes expressing the length of exponent
// 3) 32 bytes expressing the length of modulus
// 4) base, size as described above
// 5) exponent, size as described above
// 6) modulus, size as described above
//
//
// NOTE: input sizes are bound to 1024 bytes, with the expectation
//       that gas limits would be applied before actual computation.
//
//       maximum stack size will also prevent abuse.
//
//       see: https://eips.ethereum.org/EIPS/eip-198

pub trait ModexpImpl {
	const DIVISOR: u64;
	const EIP_2565: bool;

	fn execute_modexp(input: &[u8], target_gas: Option<u64>) -> PrecompileResult {
		if let Some(gas_left) = target_gas {
			if Self::EIP_2565 && gas_left < MIN_GAS_COST {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		};

		let mut input = Vec::from(input);
		if input.len() < 96 {
			// fill with zeros
			input.resize_with(96, Default::default);
		}

		let max_len = U256::from(u32::max_value() / 2);

		let mut buf = [0u8; 32];

		let base_len = {
			buf.copy_from_slice(&input[0..32]);
			let base_len = U256::from(&buf);
			if base_len > max_len {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
			base_len.as_usize()
		};

		let mod_len = {
			buf.copy_from_slice(&input[64..96]);
			let mod_len = U256::from(&buf);
			if mod_len > max_len {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
			mod_len.as_usize()
		};

		// Gas formula allows arbitrary large exp_len when base and modulus are empty, so we need to handle
		// empty base first.
		if mod_len.is_zero() && base_len.is_zero() {
			return Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: if Self::EIP_2565 { MIN_GAS_COST } else { 0 },
				output: [0u8; 1].to_vec(),
				logs: Default::default(),
			});
		}

		let exp_len = {
			buf.copy_from_slice(&input[32..64]);
			let exp_len = U256::from(&buf);
			if exp_len > max_len {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
			exp_len.as_usize()
		};

		// input length should be at least 96 + user-specified length of base + exp + mod
		let total_len = base_len + exp_len + mod_len + 96;
		if input.len() < total_len {
			// fill with zeros
			input.resize_with(total_len, Default::default);
		}

		// read the numbers themselves.
		let base_start = 96; // previous 3 32-byte fields
		let base = BigUint::from_bytes_be(&input[base_start..base_start + base_len]);

		let exp_start = base_start + base_len;
		let exponent = BigUint::from_bytes_be(&input[exp_start..exp_start + exp_len]);

		// do our gas accounting
		let gas_cost = ModexpPricer::cost(
			Self::EIP_2565,
			Self::DIVISOR,
			base_len,
			exp_len,
			mod_len,
			&exponent,
			target_gas,
		);
		if let Some(gas_left) = target_gas {
			if gas_left < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		};

		let mod_start = exp_start + exp_len;
		let modulus = BigUint::from_bytes_be(&input[mod_start..mod_start + mod_len]);

		let bytes = if modulus.is_zero() || modulus.is_one() {
			[0u8; 1].to_vec()
		} else {
			base.modpow(&exponent, &modulus).to_bytes_be()
		};

		// always true except in the case of zero-length modulus, which leads to
		// output of length and value 1.
		let output = match bytes.len() {
			len if len < mod_len => {
				let mut output = Vec::with_capacity(mod_len);
				output.extend(core::iter::repeat(0).take(mod_len - len));
				output.extend_from_slice(&bytes[..]);
				output
			}
			len if len == mod_len => bytes,
			_ => [0u8; 0].to_vec(),
		};

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gas_cost,
			output,
			logs: Default::default(),
		})
	}
}

pub struct IstanbulModexp;
pub struct Modexp;

impl ModexpImpl for IstanbulModexp {
	const DIVISOR: u64 = 20;
	const EIP_2565: bool = false;
}

impl ModexpImpl for Modexp {
	const DIVISOR: u64 = 3;
	const EIP_2565: bool = true;
}

impl Precompile for IstanbulModexp {
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		Self::execute_modexp(input, target_gas)
	}
}

impl Precompile for Modexp {
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		Self::execute_modexp(input, target_gas)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::H256;

	#[test]
	fn handle_min_gas() {
		let input: [u8; 0] = [];

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			Modexp::execute(&input, Some(199), &context, false),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas
			})
		);

		assert_eq!(
			Modexp::execute(&input, Some(200), &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 200,
				output: [0u8; 1].to_vec(),
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_empty_input() {
		let input: [u8; 0] = [];

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			Modexp::execute(&input, None, &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 200,
				output: [0u8; 1].to_vec(),
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_insufficient_input() {
		let input = hex::decode(
			"0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001",
		)
		.expect("Decode failed");

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			Modexp::execute(&input, None, &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 200,
				output: [0u8; 1].to_vec(),
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_excessive_input() {
		let input = hex::decode(
			"1000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001",
		)
		.expect("Decode failed");

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: From::from(0),
		};

		assert_eq!(
			IstanbulModexp::execute(&input, None, &context, false),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);
	}

	#[test]
	fn test_simple_inputs() {
		let input = hex::decode(
			"0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000001\
            03\
            05\
            07",
		)
		.expect("Decode failed");

		// 3 ^ 5 % 7 == 5

		let cost: u64 = 100000;

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			Modexp::execute(&input, Some(cost), &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 200,
				output: vec![5],
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_large_inputs() {
		let input = hex::decode(
			"0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            000000000000000000000000000000000000000000000000000000000000EA5F\
            0000000000000000000000000000000000000000000000000000000000000015\
            0000000000000000000000000000000000000000000000000000000000003874",
		)
		.expect("Decode failed");

		// 59999 ^ 21 % 14452 = 10055

		let cost: u64 = 100000;

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			IstanbulModexp::execute(&input, Some(cost), &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 204,
				output: H256::from_low_u64_be(10055).as_bytes().to_vec(),
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_large_computation() {
		let input = hex::decode(
			"0000000000000000000000000000000000000000000000000000000000000001\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000020\
            03\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e\
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f",
		)
		.expect("Decode failed");

		let cost: u64 = 100000;

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			IstanbulModexp::execute(&input, Some(cost), &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 13056,
				output: H256::from_low_u64_be(1).as_bytes().to_vec(),
				logs: Default::default(),
			})
		);
	}

	#[test]
	fn test_zero_exp_with_33_length() {
		// This is a regression test which ensures that the 'iteration_count' calculation
		// in 'calculate_iteration_count' cannot underflow.
		//
		// In debug mode, this underflow could cause a panic. Otherwise, it causes N**0 to
		// be calculated at more-than-normal expense.
		//
		// TODO: cite security advisory

		let input = vec![
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 33, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
			0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
		];

		let cost: u64 = 100000;

		let context: Context = Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		};

		assert_eq!(
			Modexp::execute(&input, Some(cost), &context, false),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				cost: 200,
				output: [0u8; 1].to_vec(),
				logs: Default::default(),
			})
		);
	}
}
