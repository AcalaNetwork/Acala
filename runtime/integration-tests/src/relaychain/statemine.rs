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
use cumulus_primitives_core::ParaId;

use frame_support::assert_ok;
use module_asset_registry::AssetMetadata;
use polkadot_parachain::primitives::Sibling;
use xcm::v1::{Junction, MultiLocation};
use xcm_emulator::TestExt;

#[cfg(feature = "with-karura-runtime")]
#[test]
fn transfer_custom_asset_works() {
	statemine_side();
	let para_acc: AccountId = Sibling::from(2000).into_account();
	let asset_units: u128 = dollar(KSM);

	Karura::execute_with(|| {
		assert_eq!(
			9_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		// ensure sender has enough KSM balance to be charged as fee
		assert_ok!(Tokens::deposit(KSM, &AccountId::from(BOB), 10 * asset_units));

		// Transfer statemine asset back to Statemine
		assert_ok!(XTokens::transfer_multicurrencies(
			Origin::signed(BOB.into()),
			vec![(CurrencyId::ForeignAsset(0), asset_units), (KSM, 4_000_000_000)],
			1,
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
		// Karura send back custom asset to Statemine, ensure recipient got custom asset
		assert_eq!(asset_units, Assets::balance(0, &AccountId::from(BOB)));
		// KSM and custom asset balance of sibling parachain sovereign account also changed
		assert_eq!(9 * asset_units, Assets::balance(0, &para_acc));
		// assert_eq!(9_996_000_000_000, Balances::free_balance(&para_acc));
		// assert_eq!(6000000000, Balances::free_balance(&para_acc));
	});
}

#[cfg(feature = "with-karura-runtime")]
#[test]
fn user_set_too_large_fee_works() {
	env_logger::init();
	statemine_side();
	let para_2000: AccountId = Sibling::from(2000).into_account();
	let para_1000: AccountId = Sibling::from(1000).into_account();
	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();

	let asset_units: u128 = dollar(KSM);
	let xcm_weight: u128 = 4_000_000_000;

	KusamaNet::execute_with(|| {
		let _ = kusama_runtime::Balances::make_free_balance_be(&child_2000, 10 * asset_units);
	});

	Karura::execute_with(|| {
		assert_eq!(
			9_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		// ensure sender has enough KSM balance to be charged as fee
		assert_ok!(Tokens::deposit(KSM, &AccountId::from(BOB), 10 * asset_units));

		// Transfer statemine asset back to Statemine
		assert_ok!(XTokens::transfer_multicurrencies(
			Origin::signed(BOB.into()),
			vec![
				(CurrencyId::ForeignAsset(0), asset_units),
				(KSM, 9 * asset_units + xcm_weight)
			],
			1,
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
			xcm_weight as u64
		));

		assert_eq!(
			8_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		assert_eq!(
			asset_units - xcm_weight,
			Tokens::free_balance(KSM, &AccountId::from(BOB))
		);
	});

	KusamaNet::execute_with(|| {
		assert_eq!(asset_units, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(8_999_893_333_340, kusama_runtime::Balances::free_balance(&child_1000));
	});

	Statemine::execute_with(|| {
		use statemine_runtime::*;
		// Karura send back custom asset to Statemine, ensure recipient got custom asset
		assert_eq!(asset_units, Assets::balance(0, &AccountId::from(BOB)));
		println!("{}", Balances::free_balance(&AccountId::from(BOB))); // 1 000 000 000 000
		println!("{}", Balances::free_balance(&para_2000)); // 17 992 786 666 680  18 987 786 666 680

		// KSM and custom asset balance of sibling parachain sovereign account also changed
		assert_eq!(9 * asset_units, Assets::balance(0, &para_2000));
		// assert_eq!(9_996_000_000_000, Balances::free_balance(&para_acc));
		// assert_eq!(6000000000, Balances::free_balance(&para_acc));
	});
}

fn statemine_side() {
	register_asset();

	let para_acc: AccountId = Sibling::from(2000).into_account();
	let asset_units: u128 = dollar(KSM);

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		let origin = Origin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), 10 * dollar(KSM));
		Balances::make_free_balance_be(&BOB.into(), dollar(KSM));

		// create custom asset cost 1 KSM
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
		let initial_ksm_para_acc = asset_units;
		Balances::make_free_balance_be(&para_acc, initial_ksm_para_acc);

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

		assert_eq!(10 * asset_units, Assets::balance(0, &para_acc));

		// the KSM balance of sibling parachain sovereign account is not changed
		assert_eq!(initial_ksm_para_acc, Balances::free_balance(&para_acc));
	});

	// Rerun the Statemine::execute to actually send the egress message via XCM
	Statemine::execute_with(|| {});
}

fn register_asset() {
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
}
