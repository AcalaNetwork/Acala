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

use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn transfer_from_relay_chain() {
	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(2000).into().into()),
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
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 999_872_000_000);
	});
}

#[test]
fn transfer_to_relay_chain() {
	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			KSM,
			dollar(KSM),
			Box::new(
				MultiLocation::new(
					1,
					X1(Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any,
					})
				)
				.into()
			),
			4_000_000_000
		));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(
			kusama_runtime::Balances::free_balance(&AccountId::from(BOB)),
			999_893_333_340
		);
	});
}

#[test]
fn transact_transfer_call_to_para_chain_use_ksm() {
	Karura::execute_with(|| {
		let _ = ParaBalances::deposit_creating(&AccountId::from(ALICE), 1000 * dollar(KAR));
	});

	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE,
	});
	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer {
		dest: MultiAddress::Id(AccountId::from(BOB)),
		value: 500 * dollar(KAR),
	});
	let assets: MultiAsset = (Parent, dollar(KSM)).into();

	KusamaNet::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(dollar(KSM) as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: (dollar(KSM) as u64) / 10 as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: { (1, alice.clone()).into() },
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(2000).into(), Xcm(xcm),));
	});

	Karura::execute_with(|| {
		use {Event, System};
		assert_eq!(9983840000000, ParaTokens::free_balance(KSM, &AccountId::from(ALICE)));
		assert_eq!(500 * dollar(KAR), ParaBalances::free_balance(&AccountId::from(ALICE)));
		assert_eq!(500 * dollar(KAR), ParaBalances::free_balance(&AccountId::from(BOB)));
		System::assert_has_event(Event::Balances(pallet_balances::Event::Transfer(
			AccountId::from(ALICE),
			AccountId::from(BOB),
			500 * dollar(KAR),
		)));
	});
}

#[test]
fn transact_transfer_call_to_para_chain_use_kusd() {
	Karura::execute_with(|| {
		let _ = ParaBalances::deposit_creating(&AccountId::from(ALICE), 1000 * dollar(KUSD));
		assert_ok!(ParaTokens::deposit(KUSD, &AccountId::from(ALICE), 1000 * dollar(KUSD)));
	});

	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE,
	});
	let call = Call::Balances(pallet_balances::Call::<Runtime>::transfer {
		dest: MultiAddress::Id(AccountId::from(BOB)),
		value: 500 * dollar(KUSD),
	});
	let assets: MultiAsset = (
		(Parent, X2(Parachain(2000), GeneralKey(KUSD.encode()))),
		100 * dollar(KUSD),
	)
		.into();

	KusamaNet::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(100 * dollar(KUSD) as u64),
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: dollar(KUSD) as u64,
				call: call.encode().into(),
			},
			DepositAsset {
				assets: All.into(),
				max_assets: 1,
				beneficiary: { (0, alice.clone()).into() },
			},
		];
		assert_ok!(RelayChainPalletXcm::send_xcm(alice, Parachain(2000).into(), Xcm(xcm),));
	});

	Karura::execute_with(|| {
		assert_eq!(935936000000000, ParaTokens::free_balance(KUSD, &AccountId::from(ALICE)));
		assert_eq!(500 * dollar(KUSD), ParaBalances::free_balance(&AccountId::from(ALICE)));
		assert_eq!(500 * dollar(KUSD), ParaBalances::free_balance(&AccountId::from(BOB)));
		System::assert_has_event(Event::Balances(pallet_balances::Event::Transfer(
			AccountId::from(ALICE),
			AccountId::from(BOB),
			500 * dollar(KUSD),
		)));
	});
}

#[test]
fn batch_cdall_execute_then_send_xcm_to_para_chain() {
	Karura::execute_with(|| {
		assert_ok!(ParaTokens::deposit(KUSD, &AccountId::from(ALICE), 2000 * dollar(KUSD)));
	});

	let alice = Junctions::X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: ALICE,
	});
	let bob = X1(Junction::AccountId32 {
		network: NetworkId::Kusama,
		id: BOB,
	});
	KusamaNet::execute_with(|| {
		// current XcmExecuteFilter = Nothing cause xcm_relay_call Filtered error
		let _xcm_relay_call = kusama_runtime::Call::XcmPallet(pallet_xcm::Call::<kusama_runtime::Runtime>::execute {
			message: Box::new(xcm::VersionedXcm::from(Xcm(vec![
				WithdrawAsset((Here, 1100 * dollar(KSM)).into()),
				BuyExecution {
					fees: (Here, 1100 * dollar(KSM)).into(),
					weight_limit: Limited(dollar(KSM) as u64),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: { (0, alice.clone()).into() },
				},
			]))),
			max_weight: dollar(KSM) as u64,
		});

		let xcm_para_call = kusama_runtime::Call::XcmPallet(pallet_xcm::Call::<kusama_runtime::Runtime>::send {
			dest: Box::new(xcm::VersionedMultiLocation::from(Parachain(2000).into())),
			message: Box::new(xcm::VersionedXcm::from(Xcm(vec![
				WithdrawAsset(
					(
						(Parent, X2(Parachain(2000), GeneralKey(KUSD.encode()))),
						1000 * dollar(KUSD),
					)
						.into(),
				),
				BuyExecution {
					fees: (
						(Parent, X2(Parachain(2000), GeneralKey(KUSD.encode()))),
						1000 * dollar(KUSD),
					)
						.into(),
					weight_limit: Limited(dollar(KUSD) as u64),
				},
				DepositAsset {
					assets: All.into(),
					max_assets: 1,
					beneficiary: { (0, bob).into() },
				},
			]))),
		});

		assert_ok!(pallet_utility::Pallet::<kusama_runtime::Runtime>::batch_all(
			kusama_runtime::Origin::signed(AccountId::from(ALICE)),
			vec![xcm_para_call]
		));
	});

	Karura::execute_with(|| {
		assert_eq!(
			1000 * dollar(KUSD),
			ParaTokens::free_balance(KUSD, &AccountId::from(ALICE))
		);
		assert_eq!(999948800000000, ParaTokens::free_balance(KUSD, &AccountId::from(BOB)));
	});
}
