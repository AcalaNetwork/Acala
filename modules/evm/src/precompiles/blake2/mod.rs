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
use sp_std::mem::size_of;

mod eip_152;

pub struct Blake2F;

impl Blake2F {
	const GAS_COST_PER_ROUND: u64 = 1; // https://eips.ethereum.org/EIPS/eip-152#gas-costs-and-benchmarks
}

impl Precompile for Blake2F {
	/// Format of `input`:
	/// [4 bytes for rounds][64 bytes for h][128 bytes for m][8 bytes for t_0][8 bytes for t_1][1
	/// byte for f]
	fn execute(input: &[u8], target_gas: Option<u64>, _context: &Context, _is_static: bool) -> PrecompileResult {
		const BLAKE2_F_ARG_LEN: usize = 213;

		if input.len() != BLAKE2_F_ARG_LEN {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::Other(
					"input length for Blake2 F precompile should be exactly 213 bytes".into(),
				),
			});
		}

		let mut rounds_buf: [u8; 4] = [0; 4];
		rounds_buf.copy_from_slice(&input[0..4]);
		let rounds: u32 = u32::from_be_bytes(rounds_buf);

		let gas_cost: u64 = (rounds as u64) * Blake2F::GAS_COST_PER_ROUND;
		if let Some(gas_left) = target_gas {
			if gas_left < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		// we use from_le_bytes below to effectively swap byte order to LE if architecture is BE

		let mut h_buf: [u8; 64] = [0; 64];
		h_buf.copy_from_slice(&input[4..68]);
		let mut h = [0u64; 8];
		let mut ctr = 0;
		for state_word in &mut h {
			let mut temp: [u8; 8] = Default::default();
			temp.copy_from_slice(&h_buf[(ctr * 8)..(ctr + 1) * 8]);
			*state_word = u64::from_le_bytes(temp);
			ctr += 1;
		}

		let mut m_buf: [u8; 128] = [0; 128];
		m_buf.copy_from_slice(&input[68..196]);
		let mut m = [0u64; 16];
		ctr = 0;
		for msg_word in &mut m {
			let mut temp: [u8; 8] = Default::default();
			temp.copy_from_slice(&m_buf[(ctr * 8)..(ctr + 1) * 8]);
			*msg_word = u64::from_le_bytes(temp);
			ctr += 1;
		}

		let mut t_0_buf: [u8; 8] = [0; 8];
		t_0_buf.copy_from_slice(&input[196..204]);
		let t_0 = u64::from_le_bytes(t_0_buf);

		let mut t_1_buf: [u8; 8] = [0; 8];
		t_1_buf.copy_from_slice(&input[204..212]);
		let t_1 = u64::from_le_bytes(t_1_buf);

		let f = if input[212] == 1 {
			true
		} else if input[212] == 0 {
			false
		} else {
			return Err(PrecompileFailure::Error {
				exit_status: ExitError::Other("incorrect final block indicator flag".into()),
			});
		};

		eip_152::compress(&mut h, m, [t_0, t_1], f, rounds as usize);

		let mut output_buf = [0u8; 8 * size_of::<u64>()];
		for (i, state_word) in h.iter().enumerate() {
			output_buf[i * 8..(i + 1) * 8].copy_from_slice(&state_word.to_le_bytes());
		}

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: gas_cost,
			output: output_buf.to_vec(),
			logs: Default::default(),
		})
	}
}
