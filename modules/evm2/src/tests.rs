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

#![cfg(test)]

use super::*;
use mock::{Event, *};

use frame_support::{assert_err, assert_noop, assert_ok};
use module_support::AddressMapping;
use sp_core::{
	bytes::{from_hex, to_hex},
	H160,
};
use sp_runtime::{traits::BadOrigin, AccountId32};
use std::str::FromStr;

#[test]
fn fail_call_return_ok() {
	new_test_ext().execute_with(|| {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"evm:");
		let signer: AccountId32 = AccountId32::from(data).into();

		let origin = Origin::signed(signer);
		assert_noop!(
			EVM::call(origin.clone(), contract_a(), Vec::new(), 0, 1000000, 0),
			Error::<Test>::BalanceLow
		);
		assert_noop!(
			EVM::call(origin, contract_b(), Vec::new(), 0, 1000000, 0),
			Error::<Test>::BalanceLow
		);

		let alice_account_id = <Test as Config>::AddressMapping::get_account_id(&alice());
		assert_eq!(Balances::free_balance(alice_account_id.clone()), INITIAL_BALANCE);
		let origin = Origin::signed(alice_account_id);
		assert_ok!(EVM::call(origin.clone(), contract_a(), Vec::new(), 0, 1000000, 0));
		assert_ok!(EVM::call(origin, contract_b(), Vec::new(), 0, 1000000, 0));
	});
}
