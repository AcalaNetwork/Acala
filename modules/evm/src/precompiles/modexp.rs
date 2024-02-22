// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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
use crate::{PrecompileFailure, PrecompileHandle, PrecompileOutput, PrecompileResult};
use module_evm_utility::evm::{ExitError, ExitSucceed};
use num::{BigUint, One, Zero};
use sp_core::U256;
use sp_runtime::traits::UniqueSaturatedInto;
use sp_std::{
	cmp::{max, min},
	vec::Vec,
};

const MAX_LENGTH: u64 = 1024;
const MIN_GAS_COST: u64 = 200;

struct ModexpPricer;

impl ModexpPricer {
	fn adjusted_exp_len(len: u64, exp_low: U256) -> u64 {
		let bit_index = if exp_low.is_zero() {
			0
		} else {
			(255 - exp_low.leading_zeros()) as u64
		};
		if len <= 32 {
			bit_index
		} else {
			8 * (len - 32) + bit_index
		}
	}

	fn mult_complexity(x: u64) -> u64 {
		match x {
			x if x <= 64 => x * x,
			x if x <= 1024 => (x * x) / 4 + 96 * x - 3072,
			x => (x * x) / 16 + 480 * x - 199_680,
		}
	}

	fn read_lengths(input: &[u8]) -> (U256, U256, U256) {
		let mut input = Vec::from(input);
		if input.len() < 96 {
			input.resize_with(96, Default::default);
		}
		let base_len = U256::from_big_endian(&input[..32]);
		let exp_len = U256::from_big_endian(&input[32..64]);
		let mod_len = U256::from_big_endian(&input[64..96]);
		(base_len, exp_len, mod_len)
	}

	fn read_exp(input: &[u8], base_len: U256, exp_len: U256) -> U256 {
		let input_len = input.len();
		let base_len = if base_len > U256::from(u32::MAX) {
			return U256::zero();
		} else {
			UniqueSaturatedInto::<u64>::unique_saturated_into(base_len)
		};
		if base_len + 96 >= input_len as u64 {
			U256::zero()
		} else {
			let exp_start = 96 + base_len as usize;
			let remaining_len = input_len - exp_start;
			let mut reader = Vec::from(&input[exp_start..exp_start + remaining_len]);
			let len = if exp_len < U256::from(32) {
				UniqueSaturatedInto::<usize>::unique_saturated_into(exp_len)
			} else {
				32
			};

			if reader.len() < len {
				reader.resize_with(len, Default::default);
			}

			let mut buf: Vec<u8> = Vec::new();
			buf.resize_with(32 - len, Default::default);
			buf.extend(&reader[..min(len, remaining_len)]);
			buf.resize_with(32, Default::default);
			U256::from_big_endian(&buf[..])
		}
	}

	fn cost(divisor: u64, input: &[u8]) -> U256 {
		// read lengths as U256 here for accurate gas calculation.
		let (base_len, exp_len, mod_len) = Self::read_lengths(input);

		if mod_len.is_zero() && base_len.is_zero() {
			return U256::zero();
		}

		let max_len = U256::from(MAX_LENGTH - 96);
		if base_len > max_len || mod_len > max_len || exp_len > max_len {
			return U256::max_value();
		}

		// read fist 32-byte word of the exponent.
		let exp_low = Self::read_exp(input, base_len, exp_len);

		let (base_len, exp_len, mod_len) = (
			base_len.unique_saturated_into(),
			exp_len.unique_saturated_into(),
			mod_len.unique_saturated_into(),
		);

		let m = max(mod_len, base_len);

		let adjusted_exp_len = Self::adjusted_exp_len(exp_len, exp_low);

		let (gas, overflow) = Self::mult_complexity(m).overflowing_mul(max(adjusted_exp_len, 1));
		if overflow {
			return U256::max_value();
		}

		(gas / divisor).into()
	}

	fn eip_2565_mul_complexity(base_length: U256, modulus_length: U256) -> U256 {
		let max_length = max(base_length, modulus_length);
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

	fn eip_2565_iter_count(exponent_length: U256, exponent: U256) -> U256 {
		let thirty_two = U256::from(32);
		let it = if exponent_length <= thirty_two && exponent.is_zero() {
			U256::zero()
		} else if exponent_length <= thirty_two {
			U256::from(exponent.bits()) - U256::from(1)
		} else {
			// else > 32
			U256::from(8)
				.saturating_mul(exponent_length - thirty_two)
				.saturating_add(U256::from(exponent.bits()).saturating_sub(U256::from(1)))
		};
		max(it, U256::one())
	}

	fn eip_2565_cost(
		divisor: U256,
		base_length: U256,
		modulus_length: U256,
		exponent_length: U256,
		exponent: U256,
	) -> U256 {
		let multiplication_complexity = Self::eip_2565_mul_complexity(base_length, modulus_length);
		let iteration_count = Self::eip_2565_iter_count(exponent_length, exponent);
		max(
			U256::from(MIN_GAS_COST),
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

	fn execute_modexp(input: &[u8]) -> Vec<u8> {
		let mut reader = Vec::from(input);
		if reader.len() < 96 {
			reader.resize_with(96, Default::default);
		}
		// read lengths as u64.
		// ignoring the first 24 bytes might technically lead us to fall out of consensus,
		// but so would running out of addressable memory!
		let mut buf = [0u8; 8];
		buf.copy_from_slice(&reader[24..32]);
		let base_len = u64::from_be_bytes(buf);
		buf.copy_from_slice(&reader[32 + 24..64]);
		let exp_len = u64::from_be_bytes(buf);
		buf.copy_from_slice(&reader[64 + 24..96]);
		let mod_len = u64::from_be_bytes(buf);

		// Gas formula allows arbitrary large exp_len when base and modulus are empty, so we need to handle
		// empty base first.
		let r = if base_len == 0 && mod_len == 0 {
			BigUint::zero()
		} else {
			let total_len = 96 + base_len + exp_len + mod_len;
			if total_len > MAX_LENGTH {
				return [0u8; 1].to_vec();
			}
			let mut reader = Vec::from(input);
			if reader.len() < total_len as usize {
				reader.resize_with(total_len as usize, Default::default);
			}
			// read the numbers themselves.
			let base_end = 96 + base_len as usize;
			let base = BigUint::from_bytes_be(&reader[96..base_end]);
			let exp_end = base_end + exp_len as usize;
			let exponent = BigUint::from_bytes_be(&reader[base_end..exp_end]);
			let mod_end = exp_end + mod_len as usize;
			let modulus = BigUint::from_bytes_be(&reader[exp_end..mod_end]);

			if modulus.is_zero() || modulus.is_one() {
				BigUint::zero()
			} else {
				base.modpow(&exponent, &modulus)
			}
		};

		// write output to given memory, left padded and same length as the modulus.
		let bytes = r.to_bytes_be();

		// always true except in the case of zero-length modulus, which leads to
		// output of length and value 1.
		if bytes.len() as u64 <= mod_len {
			let mut ret = Vec::with_capacity(mod_len as usize);
			ret.extend(core::iter::repeat(0).take(mod_len as usize - bytes.len()));
			ret.extend_from_slice(&bytes[..]);
			ret.to_vec()
		} else {
			[0u8; 0].to_vec()
		}
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
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let input = handle.input();
		let target_gas = handle.gas_limit();

		if input.len() as u64 > MAX_LENGTH {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			});
		}
		let cost = ModexpPricer::cost(Self::DIVISOR, input);
		if let Some(target_gas) = target_gas {
			if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let output = Self::execute_modexp(input);
		handle.record_cost(cost.as_u64())?;

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output,
		})
	}

	#[cfg(feature = "evm-tests")]
	fn execute_ext(
		input: &[u8],
		target_gas: Option<u64>,
		context: &crate::Context,
		is_static: bool,
	) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
		let mut handle = crate::precompiles::tests::MockPrecompileHandle::new(&input, target_gas, context, is_static);
		let output = Self::execute(&mut handle)?;

		Ok((output, handle.gas_used))
	}
}

impl Precompile for Modexp {
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let input = handle.input();
		let target_gas = handle.gas_limit();

		if input.len() as u64 > MAX_LENGTH {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			});
		}

		if let Some(target_gas) = target_gas {
			if target_gas < MIN_GAS_COST {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let (base_len, exp_len, mod_len) = ModexpPricer::read_lengths(input);
		let exp = ModexpPricer::read_exp(input, base_len, exp_len);
		let cost = ModexpPricer::eip_2565_cost(U256::from(Self::DIVISOR), base_len, mod_len, exp_len, exp);
		if let Some(target_gas) = target_gas {
			if cost > U256::from(u64::MAX) || target_gas < cost.as_u64() {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let output = Self::execute_modexp(input);
		handle.record_cost(cost.as_u64())?;

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output,
		})
	}

	#[cfg(feature = "evm-tests")]
	fn execute_ext(
		input: &[u8],
		target_gas: Option<u64>,
		context: &crate::Context,
		is_static: bool,
	) -> Result<(PrecompileOutput, u64), PrecompileFailure> {
		let mut handle = crate::precompiles::tests::MockPrecompileHandle::new(&input, target_gas, context, is_static);
		let output = Self::execute(&mut handle)?;

		Ok((output, handle.gas_used))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::precompiles::tests::MockPrecompileHandle;
	use hex_literal::hex;
	use module_evm_utility::evm::Context;

	fn get_context() -> Context {
		Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		}
	}

	#[test]
	fn handle_min_gas() {
		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(&[], Some(199), &get_context(), false)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(&[], Some(200), &get_context(), false)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 0].to_vec(),
			})
		);
	}

	#[test]
	fn test_empty_input() {
		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(&[], None, &get_context(), false)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 0].to_vec(),
			})
		);
	}

	#[test]
	fn test_insufficient_input() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
		"};

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(&input, None, &get_context(), false)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 1].to_vec(),
			})
		);
	}

	#[test]
	fn test_excessive_input() {
		let input = hex! {"
			1000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
		"};

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);
	}

	#[test]
	fn exp_len_overflow() {
		let input = hex! {"
			00000000000000000000000000000000000000000000000000000000000000ff
            2a1e530000000000000000000000000000000000000000000000000000000000
            0000000000000000000000000000000000000000000000000000000000000000
		"};

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);
	}

	#[test]
	fn gas_cost_multiplication_overflow() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
			000000000000000000000000000000000000000000000000000000003b27bafd
			00000000000000000000000000000000000000000000000000000000503c8ac3
		"};
		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);
	}

	#[test]
	fn test_simple_inputs() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000001
            03
            05
            07
		"};

		// 3 ^ 5 % 7 == 5

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: vec![5],
			})
		);
	}

	#[test]
	fn test_large_inputs() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000020
            000000000000000000000000000000000000000000000000000000000000EA5F
            0000000000000000000000000000000000000000000000000000000000000015
            0000000000000000000000000000000000000000000000000000000000003874
		"};

		// 59999 ^ 21 % 14452 = 10055

		let mut output = [0u8; 32];
		U256::from(10055u64).to_big_endian(&mut output);

		assert_eq!(
			IstanbulModexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: output.to_vec(),
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: output.to_vec(),
			})
		);
	}

	#[test]
	fn test_large_computation() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000020
            03
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e
            fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f
		"};

		let mut output = [0u8; 32];
		U256::from(1u64).to_big_endian(&mut output);

		assert_eq!(
			IstanbulModexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: output.to_vec(),
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: output.to_vec(),
			})
		);
	}

	#[test]
	fn zero_padding() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000002
            0000000000000000000000000000000000000000000000000000000000000020
            03
            ffff
            80
		"};

		let expected = hex!("3b01b01ac41f2d6e917c6d6a221ce793802469026d9ab7578fa2e79e4da6aaab");

		assert_eq!(
			IstanbulModexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: expected.to_vec(),
			})
		);
	}

	#[test]
	fn zero_length_modulus() {
		let input = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
            0000000000000000000000000000000000000000000000000000000000000020
            0000000000000000000000000000000000000000000000000000000000000000
            03
            ffff
		"};

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 0].to_vec(),
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

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 1].to_vec(),
			})
		);
	}

	#[test]
	fn large_input() {
		let input = vec![0u8; 1025];

		assert_eq!(
			IstanbulModexp::execute(&mut MockPrecompileHandle::new(
				&input[..1024],
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 0].to_vec(),
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input[..1024],
				Some(100_000),
				&get_context(),
				false
			)),
			Ok(PrecompileOutput {
				exit_status: ExitSucceed::Returned,
				output: [0u8; 0].to_vec(),
			})
		);

		assert_eq!(
			IstanbulModexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);

		assert_eq!(
			Modexp::execute(&mut MockPrecompileHandle::new(
				&input,
				Some(100_000),
				&get_context(),
				false
			)),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::OutOfGas,
			})
		);
	}
}
