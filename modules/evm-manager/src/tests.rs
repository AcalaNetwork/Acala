// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

//! Unit tests for the evm-manager module.

#![cfg(test)]

use super::*;
use frame_support::assert_ok;
use mock::{ExtBuilder, Runtime, ERC20, ERC20_ADDRESS};
use orml_utilities::with_transaction_result;

#[test]
fn set_erc20_mapping_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));

		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));
	});
}

#[test]
fn get_evm_address_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(with_transaction_result(|| -> DispatchResult {
			EvmCurrencyIdMapping::<Runtime>::set_erc20_mapping(ERC20_ADDRESS)
		}));
		assert_eq!(
			EvmCurrencyIdMapping::<Runtime>::get_evm_address(ERC20.into()),
			Some(ERC20_ADDRESS)
		);

		assert_eq!(EvmCurrencyIdMapping::<Runtime>::get_evm_address(u32::default()), None);
	});
}
