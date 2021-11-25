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

use karura_runtime::AssetRegistry;
use module_asset_registry::AssetMetadata;
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
fn transfer_to_sibling() {
	TestNet::reset();

	fn karura_reserve_account() -> AccountId {
		use sp_runtime::traits::AccountIdConversion;
		polkadot_parachain::primitives::Sibling::from(2000).into_account()
	}

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(BNC, &AccountId::from(ALICE), 100_000_000_000_000));
	});

	Sibling::execute_with(|| {
		assert_ok!(Tokens::deposit(BNC, &karura_reserve_account(), 100_000_000_000_000));
	});

	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			BNC,
			10_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2001),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			1_000_000_000,
		));

		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 90_000_000_000_000);
	});

	Sibling::execute_with(|| {
		assert_eq!(Tokens::free_balance(BNC, &karura_reserve_account()), 90_000_000_000_000);
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(BOB)), 9_989_760_000_000);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			BNC,
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: ALICE.into(),
						}
					)
				)
				.into()
			),
			1_000_000_000,
		));

		assert_eq!(Tokens::free_balance(BNC, &karura_reserve_account()), 95_000_000_000_000);
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(BOB)), 4_989_760_000_000);
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 94_989_760_000_000);
	});
}

#[test]
fn xcm_transfer_execution_barrier_trader_works() {
	let expect_weight_limit = 600_000_000;
	let weight_limit_too_low = 500_000_000;
	let unit_instruction_weight = 200_000_000;

	// relay-chain use normal account to send xcm, destination para-chain can't pass Barrier check
	let message = Xcm(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		BuyExecution {
			fees: (Parent, 100).into(),
			weight_limit: Unlimited,
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	KusamaNet::execute_with(|| {
		let r = pallet_xcm::Pallet::<kusama_runtime::Runtime>::send(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(2000).into().into()),
			Box::new(xcm::VersionedXcm::from(message)),
		);
		assert_ok!(r);
	});
	Karura::execute_with(|| {
		assert!(System::events().iter().any(|r| matches!(
			r.event,
			Event::DmpQueue(cumulus_pallet_dmp_queue::Event::ExecutedDownward(
				_,
				Outcome::Error(XcmError::Barrier)
			))
		)));
	});

	// AllowTopLevelPaidExecutionFrom barrier test case:
	// para-chain use XcmExecutor `execute_xcm()` method to execute xcm.
	// if `weight_limit` in BuyExecution is less than `xcm_weight(max_weight)`, then Barrier can't pass.
	// other situation when `weight_limit` is `Unlimited` or large than `xcm_weight`, then it's ok.
	let message = Xcm::<karura_runtime::Call>(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		BuyExecution {
			fees: (Parent, 100).into(),
			weight_limit: Limited(weight_limit_too_low),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	Karura::execute_with(|| {
		let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message, expect_weight_limit);
		assert_eq!(r, Outcome::Error(XcmError::Barrier));
	});

	// trader inside BuyExecution have TooExpensive error if payment less than calculated weight amount.
	// the minimum of calculated weight amount(`FixedRateOfFungible<KsmPerSecond>`) is 96_000_000
	let message = Xcm::<karura_runtime::Call>(vec![
		ReserveAssetDeposited((Parent, 95_999_999).into()),
		BuyExecution {
			fees: (Parent, 95_999_999).into(),
			weight_limit: Limited(expect_weight_limit),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	Karura::execute_with(|| {
		let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message, expect_weight_limit);
		assert_eq!(
			r,
			Outcome::Incomplete(expect_weight_limit - unit_instruction_weight, XcmError::TooExpensive)
		);
	});

	// all situation fulfilled, execute success
	let message = Xcm::<karura_runtime::Call>(vec![
		ReserveAssetDeposited((Parent, 96_000_000).into()),
		BuyExecution {
			fees: (Parent, 96_000_000).into(),
			weight_limit: Limited(expect_weight_limit),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	Karura::execute_with(|| {
		let r = XcmExecutor::<XcmConfig>::execute_xcm(Parent, message, expect_weight_limit);
		assert_eq!(r, Outcome::Complete(expect_weight_limit));
	});
}

#[test]
fn subscribe_version_notify_works() {
	// relay chain subscribe version notify of para chain
	KusamaNet::execute_with(|| {
		let r = pallet_xcm::Pallet::<kusama_runtime::Runtime>::force_subscribe_version_notify(
			kusama_runtime::Origin::root(),
			Box::new(Parachain(2000).into().into()),
		);
		assert_ok!(r);
	});
	KusamaNet::execute_with(|| {
		kusama_runtime::System::assert_has_event(kusama_runtime::Event::XcmPallet(
			pallet_xcm::Event::SupportedVersionChanged(
				MultiLocation {
					parents: 0,
					interior: X1(Parachain(2000)),
				},
				2,
			),
		));
	});

	// para chain subscribe version notify of relay chain
	Karura::execute_with(|| {
		let r = pallet_xcm::Pallet::<karura_runtime::Runtime>::force_subscribe_version_notify(
			Origin::root(),
			Box::new(Parent.into()),
		);
		assert_ok!(r);
	});
	Karura::execute_with(|| {
		System::assert_has_event(karura_runtime::Event::PolkadotXcm(
			pallet_xcm::Event::SupportedVersionChanged(
				MultiLocation {
					parents: 1,
					interior: Here,
				},
				2,
			),
		));
	});

	// para chain subscribe version notify of sibling chain
	Karura::execute_with(|| {
		let r = pallet_xcm::Pallet::<karura_runtime::Runtime>::force_subscribe_version_notify(
			Origin::root(),
			Box::new((Parent, Parachain(2001)).into()),
		);
		assert_ok!(r);
	});
	Karura::execute_with(|| {
		assert!(karura_runtime::System::events().iter().any(|r| matches!(
			r.event,
			karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::XcmpMessageSent(Some(_)))
		)));
	});
	Sibling::execute_with(|| {
		assert!(System::events().iter().any(|r| matches!(
			r.event,
			karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::XcmpMessageSent(Some(_)))
				| karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Success(Some(_)))
		)));
	});
}

#[test]
fn test_asset_registry_module() {
	TestNet::reset();

	fn karura_reserve_account() -> AccountId {
		use sp_runtime::traits::AccountIdConversion;
		polkadot_parachain::primitives::Sibling::from(2000).into_account()
	}

	Karura::execute_with(|| {
		// register foreign asset
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(2001), GeneralKey(KAR.encode()))).into()),
			Box::new(AssetMetadata {
				name: b"Sibling Token".to_vec(),
				symbol: b"ST".to_vec(),
				decimals: 12,
				minimal_balance: Balances::minimum_balance() / 10, // 10%
			})
		));

		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &TreasuryAccount::get()),
			0
		);
	});

	Sibling::execute_with(|| {
		let _ = Balances::deposit_creating(&AccountId::from(BOB), 100_000_000_000_000);
		assert_eq!(Balances::free_balance(&karura_reserve_account()), 0);
		assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 100_000_000_000_000);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			KAR,
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: ALICE.into(),
						}
					)
				)
				.into()
			),
			1_000_000_000,
		));

		assert_eq!(Balances::free_balance(&karura_reserve_account()), 5_000_000_000_000);
		assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 95_000_000_000_000);
	});

	Karura::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(ALICE)),
			4_999_360_000_000
		);
		// ToTreasury
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &TreasuryAccount::get()),
			640_000_000
		);

		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::ForeignAsset(0),
			1_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2001),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: BOB.into(),
						}
					)
				)
				.into()
			),
			1_000_000_000,
		));

		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(ALICE)),
			3_999_360_000_000
		);
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&karura_reserve_account()), 4_000_000_000_000);
		assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 95_993_600_000_000);
	});

	// remove it
	Karura::execute_with(|| {
		// register foreign asset
		assert_ok!(AssetRegistry::update_foreign_asset(
			Origin::root(),
			0,
			Box::new(MultiLocation::new(1, X2(Parachain(2001), GeneralKey(KAR.encode()))).into()),
			Box::new(AssetMetadata {
				name: b"Sibling Token".to_vec(),
				symbol: b"ST".to_vec(),
				decimals: 12,
				minimal_balance: 0, // buy_weight 0
			})
		));
	});

	Sibling::execute_with(|| {
		assert_eq!(Balances::free_balance(&karura_reserve_account()), 4_000_000_000_000);
		assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 95_993_600_000_000);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			KAR,
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(2000),
						Junction::AccountId32 {
							network: NetworkId::Any,
							id: ALICE.into(),
						}
					)
				)
				.into()
			),
			1_000_000_000,
		));

		assert_eq!(Balances::free_balance(&karura_reserve_account()), 9_000_000_000_000);
		assert_eq!(Balances::free_balance(&AccountId::from(BOB)), 90_993_600_000_000);
	});

	Karura::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(ALICE)),
			8_999_360_000_000
		);

		// ToTreasury
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &TreasuryAccount::get()),
			640_000_000
		);
	});
}
