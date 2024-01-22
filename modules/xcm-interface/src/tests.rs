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

//! Unit tests for xcm interface module.

#![cfg(test)]

use crate::mocks::{kusama, polkadot, ExtBuilder, ALICE, BOB};
use insta::assert_debug_snapshot;

#[test]
fn build_transfer_to_liquid_crowdloan_module_account_polkadot() {
	ExtBuilder::default().build::<polkadot::Runtime>().execute_with(|| {
		let xcm = polkadot::XcmInterface::build_transfer_to_liquid_crowdloan_module_account(ALICE, BOB, 1000000000000)
			.unwrap();
		assert_debug_snapshot!(xcm);
	});
}

#[test]
fn build_transfer_to_liquid_crowdloan_module_account_kusama() {
	ExtBuilder::default().build::<kusama::Runtime>().execute_with(|| {
		let xcm =
			kusama::XcmInterface::build_transfer_to_liquid_crowdloan_module_account(ALICE, BOB, 1000000000000).unwrap();
		assert_debug_snapshot!(xcm);
	});
}
