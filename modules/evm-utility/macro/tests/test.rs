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

#[cfg(test)]
mod tests {
	#[test]
	fn generate_function_selector_works() {
		#[module_evm_utility_macro::generate_function_selector]
		#[derive(Debug, Eq, PartialEq)]
		#[repr(u32)]
		pub enum Action {
			Name = "name()",
			Symbol = "symbol()",
			Decimals = "decimals()",
			TotalSupply = "totalSupply()",
			BalanceOf = "balanceOf(address)",
			Transfer = "transfer(address,uint256)",
		}

		assert_eq!(Action::Name as u32, 0x06fdde03_u32);
		assert_eq!(Action::Symbol as u32, 0x95d89b41_u32);
		assert_eq!(Action::Decimals as u32, 0x313ce567_u32);
		assert_eq!(Action::TotalSupply as u32, 0x18160ddd_u32);
		assert_eq!(Action::BalanceOf as u32, 0x70a08231_u32);
		assert_eq!(Action::Transfer as u32, 0xa9059cbb_u32);
	}

	#[test]
	fn keccak256_works() {
		assert_eq!(
			module_evm_utility_macro::keccak256!(""),
			&module_evm_utility::sha3_256("")
		);
		assert_eq!(
			module_evm_utility_macro::keccak256!("keccak256"),
			&module_evm_utility::sha3_256("keccak256")
		);
	}
}
