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

//! Cross-chain transfer tests within Kusama network.

use crate::relaychain::kusama_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use polkadot_parachain::primitives::Sibling;
use xcm::VersionedMultiLocation;
use xcm_builder::AccountId32Aliases;
use xcm_emulator::TestExt;
use xcm_executor::traits::Convert;

#[test]
fn statemine() {
	env_logger::init();
	Statemine::execute_with(|| {
		use westmint_runtime::*;

		let origin = Origin::signed(ALICE.into());

		Balances::make_free_balance_be(&ALICE.into(), 10 * dollar(KSM));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&Sibling::from(2000).into_account(), 10 * dollar(KSM));

		assert_ok!(Assets::create(origin.clone(), 0, MultiAddress::Id(ALICE.into()), 10,));

		assert_ok!(Assets::mint(origin.clone(), 0, MultiAddress::Id(ALICE.into()), 1000));

		System::reset_events();

		let para_acc: AccountId = Sibling::from(2000).into_account();
		println!("{:?}", para_acc);

		assert_ok!(PolkadotXcm::reserve_transfer_assets(
			origin.clone(),
			Box::new(MultiLocation::new(1, X1(Parachain(2000),)).into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((GeneralIndex(0), 100).into()),
			0
		));
		println!("{:?}", System::events());
	});
}
