// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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
use module_evm_utility::evm::{Context, ExitError, ExitSucceed};
use sp_core::U256;
use sp_std::vec::Vec;

fn read_fr(input: &[u8], start_inx: usize) -> Result<bn::Fr, PrecompileFailure> {
	let mut padded_input = Vec::from(input);
	if padded_input.len() < start_inx + 32 {
		padded_input.resize_with(start_inx + 32, Default::default);
	}

	bn::Fr::from_slice(&padded_input[start_inx..(start_inx + 32)]).map_err(|_| PrecompileFailure::Error {
		exit_status: ExitError::Other("Invalid field element".into()),
	})
}

fn read_point(input: &[u8], start_inx: usize) -> Result<bn::G1, PrecompileFailure> {
	use bn::{AffineG1, Fq, Group, G1};

	let mut padded_input = Vec::from(input);
	if padded_input.len() < start_inx + 64 {
		padded_input.resize_with(start_inx + 64, Default::default);
	}

	let px = Fq::from_slice(&padded_input[start_inx..(start_inx + 32)]).map_err(|_| PrecompileFailure::Error {
		exit_status: ExitError::Other("Invalid point x coordinate".into()),
	})?;
	let py =
		Fq::from_slice(&padded_input[(start_inx + 32)..(start_inx + 64)]).map_err(|_| PrecompileFailure::Error {
			exit_status: ExitError::Other("Invalid point y coordinate".into()),
		})?;
	Ok(if px == Fq::zero() && py == Fq::zero() {
		G1::zero()
	} else {
		AffineG1::new(px, py)
			.map_err(|_| PrecompileFailure::Error {
				exit_status: ExitError::Other("Invalid curve point".into()),
			})?
			.into()
	})
}

/// The Bn128Add builtin
pub struct Bn128Add;

impl Bn128Add {
	const GAS_COST: u64 = 150; // https://eips.ethereum.org/EIPS/eip-1108
}

impl Precompile for Bn128Add {
	fn execute(input: &[u8], _target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		use bn::AffineG1;

		let p1 = read_point(input, 0)?;
		let p2 = read_point(input, 64)?;

		let mut buf = [0u8; 64];
		if let Some(sum) = AffineG1::from_jacobian(p1 + p2) {
			// point not at infinity
			sum.x()
				.to_big_endian(&mut buf[0..32])
				.map_err(|_| PrecompileFailure::Error {
					exit_status: ExitError::Other("Cannot fail since 0..32 is 32-byte length".into()),
				})?;
			sum.y()
				.to_big_endian(&mut buf[32..64])
				.map_err(|_| PrecompileFailure::Error {
					exit_status: ExitError::Other("Cannot fail since 32..64 is 32-byte length".into()),
				})?;
		}

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: Bn128Add::GAS_COST,
			output: buf.to_vec(),
			logs: Default::default(),
		})
	}
}

/// The Bn128Mul builtin
pub struct Bn128Mul;

impl Bn128Mul {
	const GAS_COST: u64 = 6_000; // https://eips.ethereum.org/EIPS/eip-1108
}

impl Precompile for Bn128Mul {
	fn execute(input: &[u8], _target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		use bn::AffineG1;

		let p = read_point(input, 0)?;
		let fr = read_fr(input, 64)?;

		let mut buf = [0u8; 64];
		if let Some(sum) = AffineG1::from_jacobian(p * fr) {
			// point not at infinity
			sum.x()
				.to_big_endian(&mut buf[0..32])
				.map_err(|_| PrecompileFailure::Error {
					exit_status: ExitError::Other("Cannot fail since 0..32 is 32-byte length".into()),
				})?;
			sum.y()
				.to_big_endian(&mut buf[32..64])
				.map_err(|_| PrecompileFailure::Error {
					exit_status: ExitError::Other("Cannot fail since 32..64 is 32-byte length".into()),
				})?;
		}

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: Bn128Mul::GAS_COST,
			output: buf.to_vec(),
			logs: Default::default(),
		})
	}
}

/// The Bn128Pairing builtin
pub struct Bn128Pairing;

impl Bn128Pairing {
	// https://eips.ethereum.org/EIPS/eip-1108
	const BASE_GAS_COST: u64 = 45_000;
	const GAS_COST_PER_PAIRING: u64 = 34_000;
}

impl Precompile for Bn128Pairing {
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		use bn::{pairing_batch, AffineG1, AffineG2, Fq, Fq2, Group, Gt, G1, G2};

		if let Some(gas_left) = target_gas {
			if gas_left < Bn128Pairing::BASE_GAS_COST {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		if input.len() % 192 != 0 {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::Other("Invalid input length, must be multiple of 192 (3 * (32*2))".into()),
			});
		}

		let (ret_val, gas_cost) = if input.is_empty() {
			(U256::one(), Bn128Pairing::BASE_GAS_COST)
		} else {
			// (a, b_a, b_b - each 64-byte affine coordinates)
			let elements = input.len() / 192;

			let gas_cost: u64 = Bn128Pairing::BASE_GAS_COST + (elements as u64 * Bn128Pairing::GAS_COST_PER_PAIRING);
			if let Some(gas_left) = target_gas {
				if gas_left < gas_cost {
					return Err(PrecompileFailure::Error {
						exit_status: ExitError::OutOfGas,
					});
				}
			}

			let mut vals = Vec::new();
			for idx in 0..elements {
				let a_x = Fq::from_slice(&input[idx * 192..idx * 192 + 32]).map_err(|_| PrecompileFailure::Error {
					exit_status: ExitError::Other("Invalid a argument x coordinate".into()),
				})?;

				let a_y =
					Fq::from_slice(&input[idx * 192 + 32..idx * 192 + 64]).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid a argument y coordinate".into()),
					})?;

				let b_a_y =
					Fq::from_slice(&input[idx * 192 + 64..idx * 192 + 96]).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid b argument imaginary coeff x coordinate".into()),
					})?;

				let b_a_x =
					Fq::from_slice(&input[idx * 192 + 96..idx * 192 + 128]).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid b argument imaginary coeff y coordinate".into()),
					})?;

				let b_b_y =
					Fq::from_slice(&input[idx * 192 + 128..idx * 192 + 160]).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid b argument real coeff x coordinate".into()),
					})?;

				let b_b_x =
					Fq::from_slice(&input[idx * 192 + 160..idx * 192 + 192]).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid b argument real coeff y coordinate".into()),
					})?;

				let b_a = Fq2::new(b_a_x, b_a_y);
				let b_b = Fq2::new(b_b_x, b_b_y);
				let b = if b_a.is_zero() && b_b.is_zero() {
					G2::zero()
				} else {
					G2::from(AffineG2::new(b_a, b_b).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid b argument - not on curve".into()),
					})?)
				};
				let a = if a_x.is_zero() && a_y.is_zero() {
					G1::zero()
				} else {
					G1::from(AffineG1::new(a_x, a_y).map_err(|_| PrecompileFailure::Error {
						exit_status: ExitError::Other("Invalid a argument - not on curve".into()),
					})?)
				};
				vals.push((a, b));
			}

			let mul = pairing_batch(&vals);

			if mul == Gt::one() {
				(U256::one(), gas_cost)
			} else {
				(U256::zero(), gas_cost)
			}
		};

		let mut buf = [0u8; 32];
		ret_val.to_big_endian(&mut buf);

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gas_cost,
			output: buf.to_vec(),
			logs: Default::default(),
		})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;

	fn get_context() -> Context {
		Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		}
	}

	#[test]
	fn bn128_add() {
		// zero-points additions
		{
			let input = hex! {"
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
			"};

			let expected = hex! {"
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				Bn128Add::execute(&input[..], None, &get_context(), false)
					.unwrap()
					.output,
				expected
			);
		}

		// no input, should not fail
		{
			let input = [0u8; 0];

			let expected = hex! {"
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				Bn128Add::execute(&input[..], None, &get_context(), false)
					.unwrap()
					.output,
				expected
			);
		}

		// should fail - point not on curve
		{
			let input = hex! {"
				1111111111111111111111111111111111111111111111111111111111111111
				1111111111111111111111111111111111111111111111111111111111111111
				1111111111111111111111111111111111111111111111111111111111111111
				1111111111111111111111111111111111111111111111111111111111111111
			"};

			assert_eq!(
				Bn128Add::execute(&input[..], None, &get_context(), false),
				Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Invalid curve point".into())
				})
			);
		}
	}

	#[test]
	fn bn128_mul() {
		// zero-point multiplication
		{
			let input = hex! {"
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
				0200000000000000000000000000000000000000000000000000000000000000
			"};

			let expected = hex! {"
				0000000000000000000000000000000000000000000000000000000000000000
				0000000000000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				Bn128Mul::execute(&input[..], None, &get_context(), false)
					.unwrap()
					.output,
				expected
			);
		}

		// should fail - point not on curve
		{
			let input = hex! {"
				1111111111111111111111111111111111111111111111111111111111111111
				1111111111111111111111111111111111111111111111111111111111111111
				0f00000000000000000000000000000000000000000000000000000000000000
			"};

			assert_eq!(
				Bn128Mul::execute(&input[..], None, &get_context(), false),
				Err(PrecompileFailure::Error {
					exit_status: ExitError::Other("Invalid curve point".into())
				})
			);
		}
	}

	#[test]
	fn bn128_pairing_empty() {
		// should not fail, because empty input is a valid input of 0 elements
		let input = [0u8; 0];

		let expected = hex! {"
			0000000000000000000000000000000000000000000000000000000000000001
		"};

		assert_eq!(
			Bn128Pairing::execute(&input[..], None, &get_context(), false)
				.unwrap()
				.output,
			expected
		);
	}

	#[test]
	fn bn128_pairing_notcurve() {
		// should fail - point not on curve
		let input = hex! {"
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
		"};

		assert_eq!(
			Bn128Pairing::execute(&input[..], None, &get_context(), false),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::Other("Invalid b argument - not on curve".into())
			})
		);
	}

	#[test]
	fn bn128_pairing_fragmented() {
		// should fail - input length is invalid
		let input = hex! {"
			1111111111111111111111111111111111111111111111111111111111111111
			1111111111111111111111111111111111111111111111111111111111111111
			111111111111111111111111111111
		"};

		assert_eq!(
			Bn128Pairing::execute(&input[..], None, &get_context(), false),
			Err(PrecompileFailure::Error {
				exit_status: ExitError::Other("Invalid input length, must be multiple of 192 (3 * (32*2))".into())
			})
		);
	}
}
