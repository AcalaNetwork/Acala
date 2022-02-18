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

//! Tests parachain to parachain xcm communication between Statemine and Karura.
use crate::relaychain::kusama_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use module_asset_registry::AssetMetadata;
use polkadot_parachain::primitives::Sibling;
use xcm::v1::{Junction, MultiLocation};
use xcm_emulator::TestExt;

#[cfg(feature = "with-karura-runtime")]
#[test]
fn can_transfer_custom_asset_into_karura() {
	env_logger::init();

	Karura::execute_with(|| {
		// register foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X3(Parachain(1000), PalletInstance(50), GeneralIndex(0))).into()),
			Box::new(AssetMetadata {
				name: b"Sibling Token".to_vec(),
				symbol: b"ST".to_vec(),
				decimals: 12,
				minimal_balance: Balances::minimum_balance() / 10, // 10%
			})
		));
	});

	Statemine::execute_with(|| {
		use westmint_runtime::*;

		let origin = Origin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), 10 * dollar(KSM));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&Sibling::from(2000).into_account(), 10 * dollar(KSM));

		assert_ok!(Assets::create(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			cent(KSM)
		));
		assert_ok!(Assets::mint(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			1000 * dollar(KSM)
		));

		let para_acc: AccountId = Sibling::from(2000).into_account();

		assert_ok!(PolkadotXcm::reserve_transfer_assets(
			origin.clone(),
			Box::new(MultiLocation::new(1, X1(Parachain(2000))).into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((X2(PalletInstance(50), GeneralIndex(0)), dollar(KSM)).into()),
			0
		));

		assert_eq!(Assets::balance(0, &para_acc), dollar(KSM));
	});

	// Rerun the Statemine::execute to actually send the egress message via XCM
	Statemine::execute_with(|| {});

	Karura::execute_with(|| {
		assert_eq!(
			999_360_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
	});
}
