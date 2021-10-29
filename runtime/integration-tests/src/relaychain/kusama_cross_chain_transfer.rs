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

use frame_support::{assert_err, assert_noop, assert_ok};

use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

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
fn transfer_from_relay_chain() {
	KusamaNet::execute_with(|| {
		assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
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
		assert_eq!(RelayBalances::free_balance(&AccountId::from(ALICE)), 2001 * dollar(KSM));
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 999_936_000_000);
	});
}

#[test]
fn teleport_from_relay_chain_v1_imbalance() {
	// env_logger::init();

	KusamaNet::execute_with(|| {
		assert_ok!(RelayChainPalletXcm::teleport_assets(
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
		// RelayChain account withdrawn, but ParaChain account not deposited
		assert_eq!(RelayBalances::free_balance(&AccountId::from(ALICE)), 2001 * dollar(KSM));
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 0);
	});
}

#[test]
fn transfer_from_para_chain_v1_imbalance() {
	// env_logger::init();

	Karura::execute_with(|| {
		assert_ok!(ParachainPalletXcm::reserve_transfer_assets(
			karura_runtime::Origin::signed(ALICE.into()),
			Box::new(xcm::VersionedMultiLocation::V1(xcm::v1::Parent.into())),
			// Box::new(xcm::v1::Parent.into().into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((xcm::v1::Parent, 1000000000000).into()),
			0,
		));

		assert_eq!(ParaTokens::free_balance(KSM, &AccountId::from(ALICE)), 9 * dollar(KSM));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(RelayBalances::free_balance(&AccountId::from(BOB)), 0);
	});
}

#[test]
fn teleport_from_para_chain_v1_filtered() {
	// env_logger::init();

	Karura::execute_with(|| {
		assert_noop!(
			ParachainPalletXcm::teleport_assets(
				karura_runtime::Origin::signed(ALICE.into()),
				Box::new(xcm::VersionedMultiLocation::V1(xcm::v1::Parent.into())),
				// Box::new(xcm::v1::Parent.into().into()),
				Box::new(
					Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any
					}
					.into()
					.into()
				),
				Box::new((xcm::v1::Parent, 1000000000000).into()),
				0,
			),
			pallet_xcm::Error::<karura_runtime::Runtime>::Filtered
		);
		assert_eq!(ParaTokens::free_balance(KSM, &AccountId::from(ALICE)), 10 * dollar(KSM));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 0);
	});
}

#[test]
fn transfer_from_relay_chain_v0() {
	use xcm::v0::Junction::*;
	use xcm::v0::MultiAsset::*;
	use xcm::v0::Order::*;
	use xcm::v0::*;
	use xcm::*;

	KusamaNet::execute_with(|| {
		assert_ok!(RelayChainPalletXcm::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(VersionedMultiLocation::V0(X1(Parachain(2000)))),
			Box::new(VersionedMultiLocation::V0(X1(AccountId32 {
				network: Any,
				id: BOB.into()
			}))),
			Box::new(VersionedMultiAssets::V0(vec![ConcreteFungible {
				id: MultiLocation::Null,
				amount: 1000000000000,
			}])),
			0,
		));
		assert_eq!(RelayBalances::free_balance(&AccountId::from(ALICE)), 2001 * dollar(KSM));
		assert_eq!(
			RelayBalances::free_balance(&AccountId::from(para_karura_account())),
			3 * dollar(KSM)
		);
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 999_936_000_000);
	});
}

#[test]
fn transfer_from_para_chain_v0_imbalance() {
	// env_logger::init();
	use xcm::v0::Junction::*;
	use xcm::v0::MultiAsset::*;
	use xcm::v0::Order::*;
	use xcm::v0::*;
	use xcm::*;

	Karura::execute_with(|| {
		assert_ok!(ParachainPalletXcm::reserve_transfer_assets(
			karura_runtime::Origin::signed(ALICE.into()),
			Box::new(VersionedMultiLocation::V0(X1(Parent))),
			Box::new(VersionedMultiLocation::V0(X1(AccountId32 {
				network: Any,
				id: BOB.into()
			}))),
			Box::new(VersionedMultiAssets::V0(vec![ConcreteFungible {
				id: MultiLocation::X1(Parent),
				amount: 1000000000000,
			}])),
			0,
		));
		assert_eq!(ParaTokens::free_balance(KSM, &AccountId::from(ALICE)), 9 * dollar(KSM));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 0);
	});
}

#[test]
fn teleport_from_para_chain_v0() {
	use xcm::v0::Junction::*;
	use xcm::v0::MultiAsset::*;
	use xcm::v0::Order::*;
	use xcm::v0::*;
	use xcm::*;
	env_logger::init();

	Karura::execute_with(|| {
		assert_noop!(
			ParachainPalletXcm::teleport_assets(
				karura_runtime::Origin::signed(ALICE.into()),
				Box::new(VersionedMultiLocation::V0(X1(Parent))),
				Box::new(VersionedMultiLocation::V0(X1(AccountId32 {
					network: Any,
					id: BOB.into()
				}))),
				Box::new(VersionedMultiAssets::V0(vec![ConcreteFungible {
					id: MultiLocation::X1(Parent),
					amount: 1000000000000,
				}])),
				0,
			),
			pallet_xcm::Error::<karura_runtime::Runtime>::Filtered
		);
		assert_eq!(ParaTokens::free_balance(KSM, &AccountId::from(ALICE)), 10 * dollar(KSM));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 0);
	});
}
