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

use super::LinearCostPrecompile;
use crate::PrecompileFailure;
use module_evm_utility::evm::ExitSucceed;
use sp_std::vec::Vec;

/// The identity precompile.
pub struct Identity;

impl LinearCostPrecompile for Identity {
	const BASE: u64 = 15;
	const WORD: u64 = 3;

	fn execute(input: &[u8], _: u64) -> core::result::Result<(ExitSucceed, Vec<u8>), PrecompileFailure> {
		Ok((ExitSucceed::Returned, input.to_vec()))
	}
}
