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

//! Unit tests for example module.

#![cfg(test)]

use crate::mock::*;
use frame_support::assert_ok;
use primitives::nft::{ClassProperty, Properties};

fn setup_nft() {
	assert_ok!(ModuleNFT::create_class(
		Origin::signed(ALICE),
		Default::default(),
		Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
		Default::default(),
		Some(PrtPalletAccount::get()),
	));

	if let Event::ModuleNFT(module_nft::Event::CreatedClass { owner: _, class_id }) =
		System::events().last().unwrap().event.clone()
	{
		assert_ok!(PRT::set_nft_id(Origin::root(), class_id));
	}
}

#[test]
fn place_bid_works() {
	ExtBuilder::default()
		.balances(vec![
			(ALICE, NATIVE_CURRENCY, dollar(1_000)),
			(ALICE, RELAYCHAIN_CURRENCY, dollar(1_000)),
			(BOB, NATIVE_CURRENCY, dollar(1_000)),
			(BOB, RELAYCHAIN_CURRENCY, dollar(1_000)),
		])
		.build()
		.execute_with(|| {
			setup_nft();

			assert_ok!(PRT::place_bid(Origin::signed(ALICE), dollar(100), 1));
		});
}
