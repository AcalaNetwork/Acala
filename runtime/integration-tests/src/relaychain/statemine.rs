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
pub use orml_traits::GetByKey;
use polkadot_parachain::primitives::Sibling;
use xcm::v1::{Junction, MultiLocation};
use xcm_emulator::TestExt;

pub const UNIT: Balance = 1_000_000_000_000;
pub const TEN: Balance = 10_000_000_000_000;
pub const FEE_STATEMINE: Balance = 4_000_000_000;
pub const FEE_KUSAMA: Balance = 106_666_660;

// Statemine ED
pub const EXISTENTIAL_DEPOSIT: Balance = CENTS / 10;
pub const UNITS: Balance = 1_000_000_000_000;
pub const CENTS: Balance = UNITS / 30_000;

fn init_statemine_xcm_interface() {
	let xcm_operation =
		module_xcm_interface::XcmInterfaceOperation::ParachainFee(Box::new((1, Parachain(1000)).into()));
	assert_ok!(<module_xcm_interface::Pallet<Runtime>>::update_xcm_dest_weight_and_fee(
		Origin::root(),
		vec![(xcm_operation.clone(), Some(4_000_000_000), Some(4_000_000_000),)],
	));
	System::assert_has_event(Event::XcmInterface(module_xcm_interface::Event::XcmDestWeightUpdated {
		xcm_operation: xcm_operation.clone(),
		new_xcm_dest_weight: 4_000_000_000,
	}));
	System::assert_has_event(Event::XcmInterface(module_xcm_interface::Event::XcmFeeUpdated {
		xcm_operation,
		new_xcm_dest_weight: 4_000_000_000,
	}));
}

#[test]
fn statemine_min_xcm_fee_matched() {
	Statemine::execute_with(|| {
		use frame_support::weights::{IdentityFee, WeightToFeePolynomial};

		init_statemine_xcm_interface();
		let weight = FEE_STATEMINE as u64;

		let fee: Balance = IdentityFee::calc(&weight);
		let statemine: MultiLocation = (1, Parachain(parachains::statemine::ID)).into();
		let bifrost: MultiLocation = (1, Parachain(parachains::bifrost::ID)).into();

		let statemine_fee: u128 = ParachainMinFee::get(&statemine);
		assert_eq!(fee, statemine_fee);

		let bifrost_fee: u128 = ParachainMinFee::get(&bifrost);
		assert_eq!(u128::MAX, bifrost_fee);
	});
}

#[test]
fn user_different_ksm_fee() {
	let para_2000: AccountId = Sibling::from(2000).into_account();
	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();
	let user1_fees = vec![
		2 * FEE_STATEMINE - 1,
		2 * FEE_STATEMINE,
		2 * FEE_STATEMINE + 1,
		2 * FEE_STATEMINE + FEE_KUSAMA,
	];
	let min_user_fee1 = 2 * FEE_STATEMINE;
	let para_init_amount1 = UNIT;

	let user2_fees = vec![2 * FEE_STATEMINE + FEE_KUSAMA];
	let min_user_fee2 = 2 * FEE_STATEMINE + FEE_KUSAMA;
	let para_init_amount2 = FEE_STATEMINE + EXISTENTIAL_DEPOSIT;

	let data1 = vec![
		(user1_fees, min_user_fee1, para_init_amount1),
		(user2_fees, min_user_fee2, para_init_amount2),
	];

	for (user_fees, min_user_fee, init_amount) in data1 {
		for user_fee in user_fees {
			TestNet::reset();

			statemine_side(init_amount);

			KusamaNet::execute_with(|| {
				let _ = kusama_runtime::Balances::make_free_balance_be(&child_2000, UNIT);
			});

			// User fee amount split into two parts:
			// First part is `FEE_STATEMINE` sent to statemine.
			// Second part `user_fee` sent to kusama then route to statemine.
			karura_side(user_fee + FEE_STATEMINE);

			KusamaNet::execute_with(|| {
				assert_eq!(UNIT - user_fee, kusama_runtime::Balances::free_balance(&child_2000));
				assert_eq!(
					user_fee - FEE_KUSAMA,
					kusama_runtime::Balances::free_balance(&child_1000)
				);
			});

			Statemine::execute_with(|| {
				use statemine_runtime::*;
				// Karura send back custom asset to Statemine, ensure recipient got custom asset
				assert_eq!(UNIT, Assets::balance(0, &AccountId::from(BOB)));
				// the recipient's ksm not changed
				assert_eq!(UNIT, Balances::free_balance(&AccountId::from(BOB)));
				// and withdraw sibling parachain sovereign account
				assert_eq!(TEN - UNIT, Assets::balance(0, &para_2000));

				if user_fee < min_user_fee {
					assert_eq!(UNIT - FEE_STATEMINE, Balances::free_balance(&para_2000));
				} else {
					assert_eq!(
						init_amount - FEE_STATEMINE + user_fee - (FEE_STATEMINE + FEE_KUSAMA),
						Balances::free_balance(&para_2000)
					);
				}
			});
		}
	}
}

#[test]
fn user_large_fee_fund_to_sovereign_account_works() {
	let para_2000: AccountId = Sibling::from(2000).into_account();
	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();

	let assets: Vec<(u128, u128, u128, u128)> = vec![
		(9 * UNIT + FEE_STATEMINE, UNIT, 8_999_893_333_340, 9_991_893_333_340),
		(UNIT, 9_004_000_000_000, 995_893_333_340, 1_987_893_333_340),
	];

	for (asset, c_2000, c_1000, p_2000) in assets {
		TestNet::reset();

		statemine_side(UNIT);

		KusamaNet::execute_with(|| {
			let _ = kusama_runtime::Balances::make_free_balance_be(&child_2000, TEN);
		});

		karura_side(asset);

		KusamaNet::execute_with(|| {
			// first xcm send to relaychain with 9 KSM. 10 KSM - 9 KSM = 1 KSM
			assert_eq!(c_2000, kusama_runtime::Balances::free_balance(&child_2000));
			// 9 KSM - fee on relaychain = 9 KSM - 106_666_660
			assert_eq!(c_1000, kusama_runtime::Balances::free_balance(&child_1000));
		});

		Statemine::execute_with(|| {
			use statemine_runtime::*;
			// Karura send back custom asset to Statemine, ensure recipient got custom asset
			assert_eq!(UNIT, Assets::balance(0, &AccountId::from(BOB)));
			// the recipient's ksm not changed
			assert_eq!(UNIT, Balances::free_balance(&AccountId::from(BOB)));
			// and withdraw sibling parachain sovereign account
			assert_eq!(9 * UNIT, Assets::balance(0, &para_2000));

			assert_eq!(p_2000, Balances::free_balance(&para_2000));
		});
	}
}

// transfer custom asset from Karura to Statemine
fn karura_side(fee_amount: u128) {
	Karura::execute_with(|| {
		init_statemine_xcm_interface();

		assert_eq!(
			9_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		// ensure sender has enough KSM balance to be charged as fee
		assert_ok!(Tokens::deposit(KSM, &AccountId::from(BOB), TEN));

		assert_ok!(XTokens::transfer_multicurrencies(
			Origin::signed(BOB.into()),
			vec![(CurrencyId::ForeignAsset(0), UNIT), (KSM, fee_amount)],
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
			FEE_STATEMINE as u64
		));

		assert_eq!(
			8_999_936_000_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		assert_eq!(TEN - fee_amount, Tokens::free_balance(KSM, &AccountId::from(BOB)));
	});
}

// transfer custom asset from Statemine to Karura
fn statemine_side(para_2000_init_amount: u128) {
	register_asset();

	let para_acc: AccountId = Sibling::from(2000).into_account();

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		let origin = Origin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), TEN);
		Balances::make_free_balance_be(&BOB.into(), UNIT);

		// create custom asset cost 1 KSM
		assert_ok!(Assets::create(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			UNIT / 100
		));
		assert_eq!(9 * UNIT, Balances::free_balance(&AccountId::from(ALICE)));

		assert_ok!(Assets::mint(
			origin.clone(),
			0,
			MultiAddress::Id(ALICE.into()),
			1000 * UNIT
		));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&para_acc, para_2000_init_amount);

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
			Box::new((X2(PalletInstance(50), GeneralIndex(0)), TEN).into()),
			0
		));

		assert_eq!(0, Assets::balance(0, &AccountId::from(BOB)));

		assert_eq!(TEN, Assets::balance(0, &para_acc));
		// the KSM balance of sibling parachain sovereign account is not changed
		assert_eq!(para_2000_init_amount, Balances::free_balance(&para_acc));
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
