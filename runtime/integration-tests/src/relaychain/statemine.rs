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
pub use orml_traits::GetByKey;
use polkadot_parachain::primitives::Sibling;
use primitives::currency::AssetMetadata;
use xcm::v1::{Junction, MultiLocation};
use xcm_emulator::TestExt;

pub const UNIT: Balance = 1_000_000_000_000;
pub const TEN: Balance = 10_000_000_000_000;
pub const FEE_WEIGHT: Balance = 4_000_000_000;
pub const FEE_STATEMINE: Balance = 10_666_664;
pub const FEE_KUSAMA: Balance = 165_940_672;

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
		let weight = FEE_WEIGHT as u64;

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
fn transfer_from_relay_chain() {
	let child_1000: AccountId = ParaId::from(1000).into_account();

	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(1000).into().into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((Here, dollar(KSM)).into()),
			0
		));
		assert_eq!(dollar(KSM), kusama_runtime::Balances::free_balance(&child_1000));
	});

	Statemine::execute_with(|| {
		assert_eq!(
			dollar(KSM) - FEE_STATEMINE,
			Balances::free_balance(&AccountId::from(BOB))
		);
	});
}

#[test]
fn statemine_transfer_ksm_to_karura_failed() {
	TestNet::reset();

	let para_2000: AccountId = Sibling::from(2000).into_account();
	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();

	Statemine::execute_with(|| {
		Balances::make_free_balance_be(&ALICE.into(), 2 * dollar(KSM));
		Balances::make_free_balance_be(&para_2000, 2 * dollar(KSM));

		assert_ok!(statemine_runtime::PolkadotXcm::reserve_transfer_assets(
			statemine_runtime::Origin::signed(ALICE.into()),
			Box::new(MultiLocation::new(1, X1(Parachain(2000))).into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((Parent, dollar(KSM)).into()),
			0
		));
		assert_eq!(
			dollar(KSM),
			statemine_runtime::Balances::free_balance(&AccountId::from(ALICE))
		);
		assert_eq!(3 * dollar(KSM), statemine_runtime::Balances::free_balance(&para_2000));
	});

	// UntrustedReserveLocation
	KusamaNet::execute_with(|| {
		assert_eq!(2 * dollar(KSM), kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	Karura::execute_with(|| {
		assert_eq!(0, Tokens::free_balance(KSM, &AccountId::from(BOB)));
	});
}

#[test]
fn karura_transfer_ksm_to_statemine() {
	TestNet::reset();

	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();

	KusamaNet::execute_with(|| {
		assert_eq!(2 * UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(0, kusama_runtime::Balances::free_balance(&child_1000));
	});

	// Karura transfer KSM to statemine, it's kind of A-[B]-C scene.
	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			KSM,
			dollar(KSM),
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
	});

	KusamaNet::execute_with(|| {
		assert_eq!(UNIT, kusama_runtime::Balances::free_balance(&child_2000));
		assert_eq!(999_834_059_328, kusama_runtime::Balances::free_balance(&child_1000));
	});

	Statemine::execute_with(|| {
		assert_eq!(999_823_392_664, Balances::free_balance(&AccountId::from(BOB)));
	});
}

#[test]
fn karura_statemine_self_sufficient_asset_as_fee() {
	TestNet::reset();

	let asset_id: u32 = 0;
	let para_2000: AccountId = Sibling::from(2000).into_account();

	// Alice in Statemine send RMRK to Bob on Karura
	statemine_side(UNIT);

	// Bob on Karura send back RMRK to Bob on Statemine
	karura_side(0);

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		// in `statemine_side()`, para_2000 account has 10 RMRK. after `karura_side()` transfered
		// 1 RMRK back to Statemine, para_2000 account in Statemine left 9 RMRK.
		assert_eq!(9 * UNIT, Assets::balance(asset_id, &para_2000));

		// Bob not received RMRK, because of `Trader` in Statemine current not support RMRK as fee.
		assert_eq!(0, Assets::balance(asset_id, &AccountId::from(BOB)));
	});
}

#[test]
fn karura_statemine_transfer_use_ksm_as_fee() {
	TestNet::reset();
	let para_2000: AccountId = Sibling::from(2000).into_account();
	let child_2000: AccountId = ParaId::from(2000).into_account();
	let child_1000: AccountId = ParaId::from(1000).into_account();
	let asset_id: u32 = 0;

	// minimum asset should be: FEE_WEIGHT+FEE_KUSAMA+max(KUSAMA_ED,STATEMINE_ED+FEE_STATEMINE).
	// but due to current half fee, sender asset should at lease: FEE_WEIGHT + 2 * FEE_KUSAMA
	let asset = FEE_WEIGHT + 2 * FEE_KUSAMA;

	// Alice in Statemine send RMRK to Bob on Karura
	statemine_side(UNIT);

	KusamaNet::execute_with(|| {
		let _ = kusama_runtime::Balances::make_free_balance_be(&child_2000, TEN);
	});

	// Bob on Karura send back RMRK with KSM as fee to Bob on Statemine
	karura_side(asset);

	KusamaNet::execute_with(|| {
		assert_eq!(
			TEN - (asset - FEE_WEIGHT),
			kusama_runtime::Balances::free_balance(&child_2000)
		);
		assert_eq!(
			asset - FEE_WEIGHT - FEE_KUSAMA,
			kusama_runtime::Balances::free_balance(&child_1000)
		);
	});

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		// Karura send back custom asset to Statemine, ensure recipient got custom asset
		assert_eq!(UNIT, Assets::balance(asset_id, &AccountId::from(BOB)));
		// and withdraw sibling parachain sovereign account
		assert_eq!(9 * UNIT, Assets::balance(asset_id, &para_2000));

		assert_eq!(
			UNIT + FEE_WEIGHT - FEE_STATEMINE,
			Balances::free_balance(&AccountId::from(BOB))
		);
		assert_eq!(
			UNIT + asset - FEE_WEIGHT - FEE_KUSAMA - FEE_STATEMINE - FEE_WEIGHT,
			Balances::free_balance(&para_2000)
		);
	});
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

		if fee_amount == 0 {
			// use custom asset(RMRK/USDT on Statemine) as fee
			assert_ok!(XTokens::transfer(
				Origin::signed(BOB.into()),
				CurrencyId::ForeignAsset(0),
				UNIT,
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
				FEE_WEIGHT as u64
			));
		} else {
			// use KSM as fee
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
				FEE_WEIGHT as u64
			));
		}

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

	let para_2000: AccountId = Sibling::from(2000).into_account();
	let asset_id: u32 = 0;

	Statemine::execute_with(|| {
		use statemine_runtime::*;

		let origin = Origin::signed(ALICE.into());
		Balances::make_free_balance_be(&ALICE.into(), TEN);
		Balances::make_free_balance_be(&BOB.into(), UNIT);

		// create custom asset cost 1 KSM
		assert_ok!(Assets::create(
			origin.clone(),
			asset_id,
			MultiAddress::Id(ALICE.into()),
			UNIT / 100
		));
		assert_eq!(9 * UNIT, Balances::free_balance(&AccountId::from(ALICE)));

		assert_ok!(Assets::mint(
			origin.clone(),
			asset_id,
			MultiAddress::Id(ALICE.into()),
			1000 * UNIT
		));

		// need to have some KSM to be able to receive user assets
		Balances::make_free_balance_be(&para_2000, para_2000_init_amount);

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
			Box::new((X2(PalletInstance(50), GeneralIndex(asset_id as u128)), TEN).into()),
			0
		));

		assert_eq!(990 * UNIT, Assets::balance(asset_id, &AccountId::from(ALICE)));
		assert_eq!(0, Assets::balance(asset_id, &AccountId::from(BOB)));

		assert_eq!(TEN, Assets::balance(asset_id, &para_2000));
		// the KSM balance of sibling parachain sovereign account is not changed
		assert_eq!(para_2000_init_amount, Balances::free_balance(&para_2000));
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
