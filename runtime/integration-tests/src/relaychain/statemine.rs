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
fn transfer_custom_asset_works() {
	Karura::execute_with(|| {
		// register foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X3(Parachain(1000), PalletInstance(50), GeneralIndex(0))).into()),
			Box::new(AssetMetadata {
				name: b"Sibling Token".to_vec(),
				symbol: b"ST".to_vec(),
				decimals: 10,
				minimal_balance: Balances::minimum_balance() / 100, // 10%
			})
		));
	});

	let para_acc: AccountId = Sibling::from(2000).into_account();
	let asset_units: u128 = dollar(KSM);

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		let origin = Origin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), 10 * dollar(KSM));
		Balances::make_free_balance_be(&BOB.into(), dollar(KSM));

		// create asset cost 1 KSM
		assert_ok!(Assets::create(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			asset_units / 100
		));
		assert_eq!(9 * asset_units, Balances::free_balance(&AccountId::from(ALICE)));

		assert_ok!(Assets::mint(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			1000 * asset_units
		));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&para_acc, 10 * asset_units);

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
			Box::new((X2(PalletInstance(50), GeneralIndex(0)), 10 * asset_units).into()),
			0
		));

		assert_eq!(Assets::balance(0, &para_acc), 10 * asset_units);
		assert_eq!(10 * asset_units, Balances::free_balance(&para_acc));
	});

	// Rerun the Statemine::execute to actually send the egress message via XCM
	Statemine::execute_with(|| {});

	Karura::execute_with(|| {
		assert_eq!(
			9_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		assert_ok!(Tokens::deposit(KSM, &AccountId::from(BOB), 10 * asset_units));

		// Transfer statemine asset back to Statemine
		assert_ok!(XTokens::transfer_using_relaychain_as_fee(
			Origin::signed(BOB.into()),
			CurrencyId::ForeignAsset(0),
			asset_units,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(1000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			4_000_000_000
		));

		assert_eq!(
			8_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		assert_eq!(9_996_000_000_000, Tokens::free_balance(KSM, &AccountId::from(BOB)));
	});

	Statemine::execute_with(|| {
		use statemine_runtime::*;
		assert_eq!(Assets::balance(0, &para_acc), 9 * asset_units);
		assert_eq!(asset_units, Assets::balance(0, &AccountId::from(BOB)));
	});
}
