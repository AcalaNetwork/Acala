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

//! Unit tests for the Starport Module

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	Currencies, Event, ExtBuilder, Origin, Runtime, Starport, System, Tokens, ACALA, ADMIN_ACCOUNT, ALICE, BOB, CASH,
	GATEWAY_ACCOUNT, INITIAL_BALANCE, KSM, MAX_GATEWAY_AUTHORITIES, PERCENT_THRESHOLD_FOR_AUTHORITY_SIGNATURE,
};

/// lock/lock_to:
/// lock works
/// lock_to works
/// lock_to Fails with insufficient Balance
/// lock_to Fails with insufficient SupplyCap

/// Invoke
/// can set supply cap via notice invocation
/// can change authorities via notice invocation
/// invocation fails with too many authorities
/// can unlock asset via notice invocation
/// unlock fails with insufficient asset
/// can set future yield via notice invocation
///
/// notices cannot be invoked more than once
/// Only gateway account can invoke notices
/// notices cannot be invoked with insufficient signatures
#[test]
fn initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(KSM, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(CASH, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), INITIAL_BALANCE);
	});
}
