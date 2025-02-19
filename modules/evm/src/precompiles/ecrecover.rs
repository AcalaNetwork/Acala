// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

use super::LinearCostPrecompile;
use crate::PrecompileFailure;
use module_evm_utility::evm::ExitSucceed;
use sp_std::{cmp::min, vec::Vec};

/// The ecrecover precompile.
pub struct ECRecover;

impl LinearCostPrecompile for ECRecover {
	const BASE: u64 = 3000;
	const WORD: u64 = 0;

	fn execute(i: &[u8], _: u64) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
		let mut input = [0u8; 128];
		input[..min(i.len(), 128)].copy_from_slice(&i[..min(i.len(), 128)]);

		let mut msg = [0u8; 32];
		let mut sig = [0u8; 65];

		msg[0..32].copy_from_slice(&input[0..32]);
		sig[0..32].copy_from_slice(&input[64..96]); // r
		sig[32..64].copy_from_slice(&input[96..128]); // s
		sig[64] = input[63]; // v

		// v can only be 27 or 28 on the full 32 bytes value.
		// https://github.com/ethereum/go-ethereum/blob/a907d7e81aaeea15d80b2d3209ad8e08e3bf49e0/core/vm/contracts.go#L177
		if input[32..63] != [0u8; 31] || ![27, 28].contains(&input[63]) {
			return Ok((ExitSucceed::Returned, [0u8; 0].to_vec()));
		}

		let result = match sp_io::crypto::secp256k1_ecdsa_recover(&sig, &msg) {
			Ok(pubkey) => {
				let mut address = sp_io::hashing::keccak_256(&pubkey);
				address[0..12].copy_from_slice(&[0u8; 12]);
				address.to_vec()
			}
			Err(_) => [0u8; 0].to_vec(),
		};

		Ok((ExitSucceed::Returned, result))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;

	#[test]
	fn handle_invalid_v() {
		// V = 1
		let input = hex! {"
			18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c
			0000000000000000000000000000000000000000000000000000000000000001
			73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f
			eeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549
		"};
		let (exit, output) = ECRecover::execute(&input, 0).unwrap();
		assert_eq!(exit, ExitSucceed::Returned);
		assert_eq!(output, [0u8; 0].to_vec());
	}

	#[test]
	fn validate_v() {
		// V = 28
		let mut input = hex! {"
			18c547e4f7b0f325ad1e56f57e26c745b09a3e503d86e00e5255ff7f715d3d1c
			000000000000000000000000000000000000000000000000000000000000001c
			73b1693892219d736caba55bdb67216e485557ea6b6af75f37096c9aa6a5a75f
			eeb940b1d03b21e36b0e47e79769f095fe2ab855bd91e3a38756b7d75a9c4549
		"};

		let expected = hex!("000000000000000000000000a94f5374fce5edbc8e2a8697c15331677e6ebf0b");

		let (exit, output) = ECRecover::execute(&input, 0).unwrap();
		assert_eq!(exit, ExitSucceed::Returned);
		assert_eq!(output, expected);

		// V = 27
		input[63] = 27;
		let (exit, output) = ECRecover::execute(&input, 0).unwrap();
		assert_eq!(exit, ExitSucceed::Returned);
		assert_ne!(output, expected);
	}
}
