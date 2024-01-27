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

mod eip_152;

pub struct Blake2F;

impl Blake2F {
	const GAS_COST_PER_ROUND: u64 = 1; // https://eips.ethereum.org/EIPS/eip-152#gas-costs-and-benchmarks
}

impl Precompile for Blake2F {
	/// Format of `input`:
	/// [4 bytes for rounds][64 bytes for h][128 bytes for m][8 bytes for t_0][8 bytes for t_1][1
	/// byte for f]
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		const BLAKE2_F_ARG_LEN: usize = 213;

		let input = handle.input();

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
		handle.record_cost(gas_cost)?;

		let input = handle.input();

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

		let mut output_buf = [0u8; u64::BITS as usize];
		for (i, state_word) in h.iter().enumerate() {
			output_buf[i * 8..(i + 1) * 8].copy_from_slice(&state_word.to_le_bytes());
		}

		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			output: output_buf.to_vec(),
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
	use frame_support::assert_ok;
	use hex_literal::hex;
	use module_evm_utility::evm::Context;
	use sp_core::U256;

	fn get_context() -> Context {
		Context {
			address: Default::default(),
			caller: Default::default(),
			apparent_value: U256::zero(),
		}
	}

	#[test]
	fn blake2f_cost() {
		// 5 rounds
		let input = hex!("0000000548c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		let context = get_context();
		let mut mock_handle = MockPrecompileHandle::new(&input[..], None, &context, false);
		assert_ok!(Blake2F::execute(&mut mock_handle));
		assert_eq!(mock_handle.gas_used, 5);
	}

	#[test]
	fn blake2f_invalid_length() {
		let err = Err(PrecompileFailure::Error {
			exit_status: ExitError::Other("input length for Blake2 F precompile should be exactly 213 bytes".into()),
		});

		// invalid input (too short)
		let input = hex!("00");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false)),
			err
		);

		// Test vector 1 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-1
		let input = hex!("00000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false)),
			err
		);

		// Test vector 2 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-2
		let input = hex!("000000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false)),
			err
		);
	}

	#[test]
	fn blake2f_bad_finalization_flag() {
		let err = Err(PrecompileFailure::Error {
			exit_status: ExitError::Other("incorrect final block indicator flag".into()),
		});

		// Test vector 3 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-3
		let input = hex!("0000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000002");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false)),
			err
		);
	}

	#[test]
	fn blake2f_zero_rounds_is_ok_test_vector_4() {
		// Test vector 4 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-4
		let input = hex!("0000000048c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		let expected = hex!("08c9bcf367e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d282e6ad7f520e511f6c3e2b8c68059b9442be0454267ce079217e1319cde05b");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false))
				.unwrap()
				.output,
			expected
		);
	}

	#[test]
	fn blake2_f_test_vector_5() {
		// Test vector 5 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-5
		let input = hex!("0000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		let expected = hex!("ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false))
				.unwrap()
				.output,
			expected
		);
	}

	#[test]
	fn blake2_f_test_vector_6() {
		// Test vector 6 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-6
		let input = hex!("0000000c48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000");
		let expected = hex!("75ab69d3190a562c51aef8d88f1c2775876944407270c42c9844252c26d2875298743e7f6d5ea2f2d3e8d226039cd31b4e426ac4f2d3d666a610c2116fde4735");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false))
				.unwrap()
				.output,
			expected
		);
	}

	#[test]
	fn blake2_f_test_vector_7() {
		// Test vector 7 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-7
		let input = hex!("0000000148c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		let expected = hex!("b63a380cb2897d521994a85234ee2c181b5f844d2c624c002677e9703449d2fba551b3a8333bcdf5f2f7e08993d53923de3d64fcc68c034e717b9293fed7a421");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false))
				.unwrap()
				.output,
			expected
		);
	}

	#[ignore]
	#[test]
	fn blake2_f_test_vector_8() {
		// Test vector 8 and expected output from https://github.com/ethereum/EIPs/blob/master/EIPS/eip-152.md#test-vector-8
		// Note this test is slow, 4294967295/0xffffffff rounds take a while.
		let input = hex!("ffffffff48c9bdf267e6096a3ba7ca8485ae67bb2bf894fe72f36e3cf1361d5f3af54fa5d182e6ad7f520e511f6c3e2b8c68059b6bbd41fbabd9831f79217e1319cde05b61626300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000001");
		let expected = hex!("fc59093aafa9ab43daae0e914c57635c5402d8e3d2130eb9b3cc181de7f0ecf9b22bf99a7815ce16419e200e01846e6b5df8cc7703041bbceb571de6631d2615");
		assert_eq!(
			Blake2F::execute(&mut MockPrecompileHandle::new(&input[..], None, &get_context(), false))
				.unwrap()
				.output,
			expected
		);
	}
}
