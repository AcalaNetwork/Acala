// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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
pub use orml_traits::GetByKey;
use polkadot_parachain::primitives::Sibling;
use primitives::currency::AssetMetadata;
use sp_runtime::traits::AccountIdConversion;
use xcm::v3::{Junction, MultiLocation};
use xcm_emulator::TestExt;

pub const UNIT: Balance = 1_000_000_000_000;
pub const TEN: Balance = 10_000_000_000_000;
pub const FEE_WEIGHT: Balance = 40_000_000_000;
pub const FEE: Balance = 200_000_000;
pub const FEE_STATEMINE: Balance = 43_543_101;
pub const FEE_KUSAMA: Balance = 11_492_737;
const ASSET_ID: u32 = 100;

fn init_statemine_xcm_interface() {
	let xcm_operation =
		module_xcm_interface::XcmInterfaceOperation::ParachainFee(Box::new((Parent, Parachain(1000)).into()));
	assert_ok!(<module_xcm_interface::Pallet<Runtime>>::update_xcm_dest_weight_and_fee(
		RuntimeOrigin::root(),
		vec![(
			xcm_operation.clone(),
			Some(XcmWeight::from_parts(4_000_000_000, 0)),
			Some(200_000_000),
		)],
	));
	System::assert_has_event(RuntimeEvent::XcmInterface(
		module_xcm_interface::Event::XcmDestWeightUpdated {
			xcm_operation: xcm_operation.clone(),
			new_xcm_dest_weight: XcmWeight::from_parts(4_000_000_000, 0),
		},
	));
	System::assert_has_event(RuntimeEvent::XcmInterface(module_xcm_interface::Event::XcmFeeUpdated {
		xcm_operation,
		new_xcm_dest_weight: 200_000_000,
	}));
}

#[test]
fn statemine_min_xcm_fee_matched() {
	Statemine::execute_with(|| {
		use frame_support::weights::{IdentityFee, WeightToFee};

		init_statemine_xcm_interface();
		let weight = FEE_WEIGHT as u64;

		let fee: Balance = IdentityFee::weight_to_fee(&Weight::from_parts(weight, 0));
		let statemine: MultiLocation = (Parent, Parachain(parachains::statemine::ID)).into();
		let bifrost: MultiLocation = (Parent, Parachain(parachains::bifrost::ID)).into();

		let statemine_fee: u128 = ParachainMinFee::get(&statemine).unwrap();
		assert_eq!(statemine_fee, FEE);
		assert_eq!(fee, FEE_WEIGHT);

		let bifrost_fee: Option<u128> = ParachainMinFee::get(&bifrost);
		assert_eq!(None, bifrost_fee);
	});
}

#[test]
fn statemine_reserve_transfer_ksm_to_karura_should_not_allowed() {
	TestNet::reset();
	let sibling_2000: AccountId = Sibling::from(2000).into_account_truncating();
	let child_2000: AccountId = ParaId::from(2000).into_account_truncating();
	let child_1000: AccountId = ParaId::from(1000).into_account_truncating();

	KusamaNet::execute_with(|| {
		assert_eq!(2 * UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	Statemine::execute_with(|| {
		Balances::make_free_balance_be(&ALICE.into(), 2 * UNIT);
		// Suppose reserve transfer can success, then dest chain(Karura) has a sibling sovereign account on
		// source chain(Statemine).
		Balances::make_free_balance_be(&sibling_2000, 2 * UNIT);

		assert_ok!(statemine_runtime::PolkadotXcm::limited_reserve_transfer_assets(
			statemine_runtime::RuntimeOrigin::signed(ALICE.into()),
			// Unlike Statemine reserve transfer to relaychain is not allowed,
			// Here Statemine reserve transfer to parachain. let's see what happened.
			Box::new(MultiLocation::new(1, X1(Parachain(2000))).into()),
			Box::new(Junction::AccountId32 { id: BOB, network: None }.into_versioned()),
			Box::new((Parent, UNIT).into()),
			0,
			WeightLimit::Unlimited
		));

		// In sender xcm execution is successed, sender account is withdrawn.
		assert_eq!(UNIT, statemine_runtime::Balances::free_balance(&AccountId::from(ALICE)));
		// And sibling parachain sovereign account on Statemine deposited.
		assert_eq!(3 * UNIT, statemine_runtime::Balances::free_balance(&sibling_2000));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(2 * UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	// Xcm execution error on receiver: UntrustedReserveLocation.
	// This means Karura not consider Statemine as reserve chain of KSM.
	Karura::execute_with(|| {
		assert_eq!(0, Tokens::free_balance(KSM, &AccountId::from(BOB)));
	});
}

#[test]
fn karura_transfer_ksm_to_statemine_should_not_allowed() {
	TestNet::reset();
	let child_2000: AccountId = ParaId::from(2000).into_account_truncating();
	let child_1000: AccountId = ParaId::from(1000).into_account_truncating();

	KusamaNet::execute_with(|| {
		assert_eq!(2 * UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	// Karura transfer KSM to Statemine, it's `NonRerserve` scene(A->[B]->C).
	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			KSM,
			UNIT,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(1000),
						Junction::AccountId32 {
							network: None,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			WeightLimit::Limited(XcmWeight::from_parts(4_000_000_000, 0))
		));

		assert_eq!(9 * UNIT, Tokens::free_balance(KSM, &AccountId::from(ALICE)));
	});

	// In relaychain, two parachain sovereign account balance changed.
	KusamaNet::execute_with(|| {
		// source parachain sovereign account withrawn.
		assert_eq!(UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		// destination parachain sovereign account deposited.
		assert_eq!(999_758_308_574, kusama_runtime::Balances::free_balance(&child_1000));
	});

	// In receiver, xm execution error: UntrustedReserveLocation.
	// This's same as Relaychain reserve transfer to Statemine which not allowed.
	Statemine::execute_with(|| {
		assert_eq!(0, Balances::free_balance(&AccountId::from(BOB)));
	});
}

#[test]
fn karura_transfer_asset_to_statemine_works() {
	TestNet::reset();

	let para_2000: AccountId = Sibling::from(2000).into_account_truncating();

	// Alice on Statemine send USDT to Bob on Karura.
	statemine_transfer_asset_to_karura();

	// Bob on Karura send back USDT to Bob on Statemine.
	// Trying use USDT as fee when execte xcm on Statemine.
	karura_transfer_asset_to_statemine(0);

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		assert_eq!(9 * UNIT, Assets::balance(ASSET_ID, &para_2000));

		// https://github.com/paritytech/cumulus/pull/1278 support using self sufficient asset
		// for paying xcm execution fee on Statemine.
		assert_eq!(988_423_297_485, Assets::balance(ASSET_ID, &AccountId::from(BOB)));
	});
}

#[test]
fn karura_statemine_transfer_use_ksm_as_fee() {
	TestNet::reset();
	let para_2000: AccountId = Sibling::from(2000).into_account_truncating();
	let child_2000: AccountId = ParaId::from(2000).into_account_truncating();
	let child_1000: AccountId = ParaId::from(1000).into_account_truncating();

	// minimum asset should be: FEE_WEIGHT+FEE_KUSAMA+max(KUSAMA_ED,STATEMINE_ED+FEE_STATEMINE).
	// but due to current half fee, sender asset should at lease: FEE_WEIGHT + 2 * FEE_KUSAMA
	let asset = FEE_WEIGHT + 2 * 31_488_122; //  40_062_976_244

	// Alice on Statemine send USDT to Bob on Karura
	statemine_transfer_asset_to_karura();

	KusamaNet::execute_with(|| {
		let _ = kusama_runtime::Balances::make_free_balance_be(&child_2000, TEN);
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	// Bob on Karura send back USDT with KSM as fee to Bob on Statemine
	karura_transfer_asset_to_statemine(asset);

	KusamaNet::execute_with(|| {
		assert_eq!(TEN - (asset - FEE), kusama_runtime::Balances::free_balance(&child_2000));
	});

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		// Karura send back custom asset to Statemine, ensure recipient got custom asset
		assert_eq!(UNIT, Assets::balance(ASSET_ID, &AccountId::from(BOB)));
		// and withdraw sibling parachain sovereign account
		assert_eq!(9 * UNIT, Assets::balance(ASSET_ID, &para_2000));

		assert_eq!(
			UNIT + FEE - FEE_STATEMINE,
			Balances::free_balance(&AccountId::from(BOB))
		);
		assert_eq!(1_039_387_546_047, Balances::free_balance(&para_2000));
	});
}

// Karura(ForeignAsset) transfer asset(e.g. USDT) back to Statemine(assets)
// `ksm_fee_amount` is used to indicate how much KSM paying as fee.
// If specify `ksm_fee_amount` to 0, then wouldn't use KSM as fee.
fn karura_transfer_asset_to_statemine(ksm_fee_amount: u128) {
	Karura::execute_with(|| {
		init_statemine_xcm_interface();

		assert_eq!(
			9_999_919_872_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		// ensure sender has enough KSM balance to be charged as fee
		assert_ok!(Tokens::deposit(KSM, &AccountId::from(BOB), TEN));

		if ksm_fee_amount == 0 {
			// use custom asset(USDT on Statemine) as fee
			assert_ok!(XTokens::transfer(
				RuntimeOrigin::signed(BOB.into()),
				CurrencyId::ForeignAsset(0),
				UNIT,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Parachain(1000),
							Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							}
						)
					)
					.into()
				),
				WeightLimit::Limited(XcmWeight::from_parts(FEE_WEIGHT as u64, 0))
			));
		} else {
			// use KSM as fee
			assert_ok!(XTokens::transfer_multicurrencies(
				RuntimeOrigin::signed(BOB.into()),
				vec![(CurrencyId::ForeignAsset(0), UNIT), (KSM, ksm_fee_amount)],
				1,
				Box::new(
					MultiLocation::new(
						1,
						X2(
							Parachain(1000),
							Junction::AccountId32 {
								network: None,
								id: BOB.into(),
							}
						)
					)
					.into()
				),
				//WeightLimit::Limited(XcmWeight::from_parts(400_000_000, 0))
				WeightLimit::Unlimited
			));
		}

		assert_eq!(
			8_999_919_872_000,
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB))
		);
		assert_eq!(TEN - ksm_fee_amount, Tokens::free_balance(KSM, &AccountId::from(BOB)));
	});
}

// Statemine(assets) transfer custom asset(e.g. USDT) to Karura(ForeignAsset)
// Alice is using reserve transfer, and Statemine is indeed the reserve chain of USDT.
// So the reserve transfer can success. On Karura side, USDT is consider as ForeignAsset.
fn statemine_transfer_asset_to_karura() {
	register_asset();

	let para_2000: AccountId = Sibling::from(2000).into_account_truncating();

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		let origin = RuntimeOrigin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), TEN);
		Balances::make_free_balance_be(&BOB.into(), UNIT);

		// If using non root, create custom asset cost 0.1 KSM
		// We're using force_create here to make sure asset is sufficient.
		assert_ok!(Assets::force_create(
			RuntimeOrigin::root(),
			ASSET_ID.into(),
			MultiAddress::Id(ALICE.into()),
			true,
			UNIT / 100
		));

		assert_ok!(Assets::mint(
			origin.clone(),
			ASSET_ID.into(),
			MultiAddress::Id(ALICE.into()),
			1000 * UNIT
		));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&para_2000, UNIT);

		assert_ok!(PolkadotXcm::limited_reserve_transfer_assets(
			origin.clone(),
			Box::new(MultiLocation::new(1, X1(Parachain(2000))).into()),
			Box::new(Junction::AccountId32 { id: BOB, network: None }.into_versioned()),
			Box::new((X2(PalletInstance(50), GeneralIndex(ASSET_ID as u128)), TEN).into()),
			0,
			WeightLimit::Unlimited
		));

		assert_eq!(990 * UNIT, Assets::balance(ASSET_ID, &AccountId::from(ALICE)));
		assert_eq!(0, Assets::balance(ASSET_ID, &AccountId::from(BOB)));

		assert_eq!(TEN, Assets::balance(ASSET_ID, &para_2000));
		// the KSM balance of sibling parachain sovereign account is not changed
		assert_eq!(UNIT, Balances::free_balance(&para_2000));
	});

	// Rerun the Statemine::execute to actually send the egress message via XCM
	Statemine::execute_with(|| {});
}

fn register_asset() {
	Karura::execute_with(|| {
		// register foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::root(),
			Box::new(
				MultiLocation::new(
					1,
					X3(Parachain(1000), PalletInstance(50), GeneralIndex(ASSET_ID as u128))
				)
				.into()
			),
			Box::new(AssetMetadata {
				name: b"Sibling Token".to_vec(),
				symbol: b"ST".to_vec(),
				decimals: 10,
				minimal_balance: Balances::minimum_balance() / 100, // 10%
			})
		));
	});
}
