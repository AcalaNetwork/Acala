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
use frame_support::{assert_noop, assert_ok};
use primitives::nft::{ClassProperty, Properties};
use sp_runtime::traits::BadOrigin;

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
fn set_prt_class_id_works() {
	ExtBuilder::default()
		.balances(vec![(ALICE, NATIVE_CURRENCY, dollar(1_000))])
		.build()
		.execute_with(|| {
			assert_noop!(PRT::set_nft_id(Origin::signed(ALICE), 1), BadOrigin);

			assert_ok!(PRT::set_nft_id(Origin::root(), 1));
			assert_eq!(PRT::prt_class_id(), Some(1));
			System::assert_last_event(Event::PRT(crate::Event::PrtClassIdSet { class_id: 1 }));

			assert_ok!(PRT::set_nft_id(Origin::root(), 324));
			assert_eq!(PRT::prt_class_id(), Some(324));
			System::assert_last_event(Event::PRT(crate::Event::PrtClassIdSet { class_id: 324 }));
		});
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

// can place bid
// cannot bid below minimum
// Require prt class id set
// cannot bid with insufficient balance

// can retract bid
// Require prt class id set
// balance unchanged when retract is requested
// cannot retract if bid is not found

// can confirm retraction
// require Oracle origin
// cannot confirm if bid doesn't exist
// cannot confirm if user does not have enough reserved balance

// can confirm issue
// Require prt class id set
// correct NFT is issued
// require oracle origin
// cannot confirm if bid doesn't exist
// cannot confirm if bidder does not have enough reserved balance
// cannot double issue.

// can handle racing conditions
// Issue can cancel retraction

// can request_thaw
// Require prt class id set
// PRT must be found
// require owner of NFT
// Require PRT expired

// can confirm thaw
// NFT is burned, and found unreserved.
// require Oracle origin
// require PRT is found
