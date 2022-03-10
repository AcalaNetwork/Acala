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

//! # Evm utiltity Module
//!
//! A pallet provides some utility methods.

#![cfg_attr(not(feature = "std"), no_std)]

use sha3::{Digest, Keccak256};

pub use ethereum;
pub use evm::{self, backend::Basic as Account};
pub use evm_gasometer;
pub use evm_runtime;

pub fn sha3_256(s: &str) -> [u8; 32] {
	let mut result = [0u8; 32];

	// create a SHA3-256 object
	let mut hasher = Keccak256::new();
	// write input message
	hasher.update(s);
	// read hash digest
	result.copy_from_slice(&hasher.finalize()[..32]);

	result
}

pub fn get_function_selector(s: &str) -> u32 {
	let result = sha3_256(s);
	u32::from_be_bytes(result[..4].try_into().unwrap())
}
