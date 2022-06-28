// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

use crate::relaychain::fee_test::*;
use crate::relaychain::kusama_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use sp_runtime::traits::AccountIdConversion;
use xcm_builder::ParentIsPreset;

use karura_runtime::parachains::bifrost::{BNC_KEY, ID as BIFROST_ID};
use karura_runtime::{AssetRegistry, KaruraTreasuryAccount};
use module_relaychain::RelayChainCallBuilder;
use module_support::CallBuilder;
use primitives::currency::{AssetMetadata, BNC};
use xcm_emulator::TestExt;
use xcm_executor::traits::Convert;

pub const KARURA_ID: u32 = 2000;
pub const MOCK_BIFROST_ID: u32 = 2001;
pub const SIBLING_ID: u32 = 2002;

fn karura_reserve_account() -> AccountId {
	polkadot_parachain::primitives::Sibling::from(KARURA_ID).into_account_truncating()
}
fn sibling_reserve_account() -> AccountId {
	polkadot_parachain::primitives::Sibling::from(SIBLING_ID).into_account_truncating()
}

#[test]
fn transfer_from_relay_chain() {
	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(KARURA_ID).into().into()),
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
		assert_eq!(
			Tokens::free_balance(KSM, &AccountId::from(BOB)),
			dollar(KSM) - relay_per_second_as_fee(4)
		);
	});
}

#[test]
fn transfer_to_relay_chain() {
	use frame_support::weights::{Weight, WeightToFee as WeightToFeeT};
	use kusama_runtime_constants::fee::WeightToFee;

	let weight: Weight = 298_368_000;
	let fee = WeightToFee::weight_to_fee(&weight);
	assert_eq!(11_523_248, fee);

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
			weight
		));
	});

	KusamaNet::execute_with(|| {
		assert_eq!(
			kusama_runtime::Balances::free_balance(&AccountId::from(BOB)),
			dollar(KSM) - fee
		);
	});
}

#[test]
fn transfer_native_chain_asset() {
	TestNet::reset();
	let dollar = dollar(BNC);
	let minimal_balance = Balances::minimum_balance() / 10; // 10%
	let foreign_fee = foreign_per_second_as_fee(4, minimal_balance);
	let bnc_fee = bnc_per_second_as_fee(4);

	MockBifrost::execute_with(|| {
		// Register native BNC's incoming address as a foreign asset so it can receive BNC
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(0, X1(GeneralKey(BNC_KEY.to_vec()))).into()),
			Box::new(AssetMetadata {
				name: b"Native BNC".to_vec(),
				symbol: b"BNC".to_vec(),
				decimals: 12,
				minimal_balance
			})
		));
		assert_ok!(Tokens::deposit(
			CurrencyId::ForeignAsset(0),
			&karura_reserve_account(),
			100 * dollar
		));

		assert_ok!(Tokens::deposit(BNC, &AccountId::from(ALICE), 100 * dollar));

		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			BNC,
			10 * dollar,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(KARURA_ID),
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

		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 90 * dollar);
		assert_eq!(Tokens::free_balance(BNC, &karura_reserve_account()), 10 * dollar);
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(BOB)), 10 * dollar - bnc_fee);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			BNC,
			5 * dollar,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(MOCK_BIFROST_ID),
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

		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(BOB)), 5 * dollar - bnc_fee);
	});

	MockBifrost::execute_with(|| {
		// Due to the re-anchoring, BNC came back as registered ForeignAsset(0)
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 90 * dollar);
		assert_eq!(Tokens::free_balance(BNC, &karura_reserve_account()), 10 * dollar);

		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(ALICE)),
			5 * dollar - foreign_fee
		);
		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 90 * dollar);
	});
}

#[test]
fn transfer_sibling_chain_asset() {
	TestNet::reset();
	let dollar = dollar(BNC);
	let minimal_balance = Balances::minimum_balance() / 10; // 10%
	let foreign_fee = foreign_per_second_as_fee(4, minimal_balance);
	let bnc_fee = bnc_per_second_as_fee(4);

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(BNC, &AccountId::from(ALICE), 100 * dollar));
	});

	MockBifrost::execute_with(|| {
		// Register native BNC's incoming address as a foreign asset so it can handle reserve transfers
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(0, X1(GeneralKey(BNC_KEY.to_vec()))).into()),
			Box::new(AssetMetadata {
				name: b"Native BNC".to_vec(),
				symbol: b"BNC".to_vec(),
				decimals: 12,
				minimal_balance,
			})
		));
		assert_ok!(Tokens::deposit(
			CurrencyId::ForeignAsset(0),
			&karura_reserve_account(),
			100 * dollar
		));
	});

	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			BNC,
			10 * dollar,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(SIBLING_ID),
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

		assert_eq!(Tokens::free_balance(BNC, &AccountId::from(ALICE)), 90 * dollar);
	});

	MockBifrost::execute_with(|| {
		// Due to reanchoring BNC is not treated as native BNC due to the change of Multilocation
		assert_eq!(Tokens::free_balance(BNC, &karura_reserve_account()), 0);
		assert_eq!(Tokens::free_balance(BNC, &sibling_reserve_account()), 0);

		// Registered Foreign asset 0 is used to handle reservation for BNC token.
		// Karura -->(transfer 10_000_000_000_000)--> Sibling
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &karura_reserve_account()),
			90 * dollar
		);
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &sibling_reserve_account()),
			10 * dollar - foreign_fee
		);
	});

	Sibling::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(BNC, &AccountId::from(BOB)),
			10 * dollar - foreign_fee - bnc_fee
		);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			BNC,
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(KARURA_ID),
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

		assert_eq!(
			Tokens::free_balance(BNC, &AccountId::from(BOB)),
			5 * dollar - foreign_fee - bnc_fee
		);
	});

	MockBifrost::execute_with(|| {
		// Sibling -->(transfer 5_000_000_000_000)--> Karura
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &karura_reserve_account()),
			95 * dollar - foreign_fee
		);
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &sibling_reserve_account()),
			5 * dollar - foreign_fee
		);
	});

	Karura::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(BNC, &AccountId::from(ALICE)),
			95 * dollar - foreign_fee - bnc_fee
		);
	});
}

#[test]
fn asset_registry_module_works() {
	TestNet::reset();
	let dollar = dollar(BNC);
	let minimal_balance = Balances::minimum_balance() / 10; // 10%
	let foreign_fee = foreign_per_second_as_fee(4, minimal_balance);
	let bnc_fee = bnc_per_second_as_fee(4);

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(BNC, &AccountId::from(ALICE), 100 * dollar));
	});

	MockBifrost::execute_with(|| {
		// Register native BNC's incoming address as a foreign asset so it can handle reserve transfers
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(0, X1(GeneralKey(BNC_KEY.to_vec()))).into()),
			Box::new(AssetMetadata {
				name: b"Native BNC".to_vec(),
				symbol: b"BNC".to_vec(),
				decimals: 12,
				minimal_balance
			})
		));
		assert_ok!(Tokens::deposit(
			CurrencyId::ForeignAsset(0),
			&karura_reserve_account(),
			100 * dollar
		));
	});

	Sibling::execute_with(|| {
		// Register BNC as foreign asset(0)
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(BIFROST_ID), GeneralKey(BNC_KEY.to_vec()))).into()),
			Box::new(AssetMetadata {
				name: b"Bifrost BNC".to_vec(),
				symbol: b"BNC".to_vec(),
				decimals: 12,
				minimal_balance
			})
		));
	});

	Karura::execute_with(|| {
		// Register BNC as foreign asset(0)
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::root(),
			Box::new(MultiLocation::new(1, X2(Parachain(BIFROST_ID), GeneralKey(BNC_KEY.to_vec()))).into()),
			Box::new(AssetMetadata {
				name: b"Bifrost BNC".to_vec(),
				symbol: b"BNC".to_vec(),
				decimals: 12,
				minimal_balance
			})
		));

		assert_ok!(Tokens::deposit(
			CurrencyId::ForeignAsset(0),
			&AccountId::from(ALICE),
			100 * dollar
		));

		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			CurrencyId::ForeignAsset(0),
			10 * dollar,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(SIBLING_ID),
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
			90 * dollar
		);
	});

	MockBifrost::execute_with(|| {
		// Registered Foreign asset 0 is used to handle reservation for BNC token.
		// Karura -->(transfer 10_000_000_000_000)--> Sibling
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &karura_reserve_account()),
			90 * dollar
		);
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &sibling_reserve_account()),
			10 * dollar - foreign_fee
		);
	});

	Sibling::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB)),
			10 * dollar - foreign_fee - bnc_fee
		);

		assert_ok!(XTokens::transfer(
			Origin::signed(BOB.into()),
			CurrencyId::ForeignAsset(0),
			5_000_000_000_000,
			Box::new(
				MultiLocation::new(
					1,
					X2(
						Parachain(KARURA_ID),
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

		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(BOB)),
			5 * dollar - foreign_fee - bnc_fee
		);
	});

	MockBifrost::execute_with(|| {
		// Sibling -->(transfer 5_000_000_000_000)--> Karura
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &karura_reserve_account()),
			95 * dollar - foreign_fee
		);
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &sibling_reserve_account()),
			5 * dollar - foreign_fee
		);
	});

	Karura::execute_with(|| {
		assert_eq!(
			Tokens::free_balance(CurrencyId::ForeignAsset(0), &AccountId::from(ALICE)),
			95 * dollar - foreign_fee - bnc_fee
		);
	});
}

#[test]
fn transfer_from_relay_chain_deposit_to_treasury_if_below_ed() {
	let minimum = relay_per_second_as_fee(4);
	let ksm_minimum = Tokens::minimum_balance(KSM);
	assert_eq!(ksm_minimum, 100_000_000);

	fn below_ed_case(amount: Balance) {
		TestNet::reset();
		KusamaNet::execute_with(|| {
			assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
				kusama_runtime::Origin::signed(ALICE.into()),
				Box::new(Parachain(KARURA_ID).into().into()),
				Box::new(
					Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any
					}
					.into()
					.into()
				),
				Box::new((Here, amount).into()),
				0
			));
		});
		Karura::execute_with(|| {
			assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 0);
			assert_eq!(
				Tokens::free_balance(KSM, &karura_runtime::KaruraTreasuryAccount::get()),
				dollar(KSM) + amount
			);
		});
	}

	fn upper_ed_case(amount: Balance) {
		let minimum = relay_per_second_as_fee(4);

		TestNet::reset();
		KusamaNet::execute_with(|| {
			assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
				kusama_runtime::Origin::signed(ALICE.into()),
				Box::new(Parachain(KARURA_ID).into().into()),
				Box::new(
					Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any
					}
					.into()
					.into()
				),
				Box::new((Here, amount).into()),
				0
			));
		});
		Karura::execute_with(|| {
			assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), amount - minimum);
			assert_eq!(
				Tokens::free_balance(KSM, &karura_runtime::KaruraTreasuryAccount::get()),
				dollar(KSM) + minimum
			);
		});
	}

	below_ed_case(minimum);
	below_ed_case(minimum + ksm_minimum - 1);
	upper_ed_case(minimum + ksm_minimum);
	upper_ed_case(minimum + ksm_minimum + 1);
}

#[test]
fn xcm_transfer_execution_barrier_trader_works() {
	let unit_instruction_weight: Weight = karura_runtime::xcm_config::UnitWeightCost::get();
	let expect_weight_limit = unit_instruction_weight.saturating_mul(3);
	let weight_limit_too_low = expect_weight_limit - 1;
	let trap_asset_limit: Balance = relay_per_second_as_fee(3);

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
		assert_ok!(pallet_xcm::Pallet::<kusama_runtime::Runtime>::send_xcm(
			X1(Junction::AccountId32 {
				network: NetworkId::Any,
				id: ALICE.into(),
			}),
			Parachain(KARURA_ID).into(),
			message
		));
	});
	Karura::execute_with(|| {
		assert!(System::events().iter().any(|r| matches!(
			r.event,
			Event::DmpQueue(cumulus_pallet_dmp_queue::Event::ExecutedDownward {
				outcome: Outcome::Error(XcmError::Barrier),
				..
			})
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
	// the minimum of calculated weight amount(`FixedRateOfFungible<KsmPerSecond>`).
	let message = Xcm::<karura_runtime::Call>(vec![
		ReserveAssetDeposited((Parent, trap_asset_limit - 1).into()),
		BuyExecution {
			fees: (Parent, trap_asset_limit - 1).into(),
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
		ReserveAssetDeposited((Parent, trap_asset_limit).into()),
		BuyExecution {
			fees: (Parent, trap_asset_limit).into(),
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
			Box::new(Parachain(KARURA_ID).into().into()),
		);
		assert_ok!(r);
	});
	KusamaNet::execute_with(|| {
		kusama_runtime::System::assert_has_event(kusama_runtime::Event::XcmPallet(
			pallet_xcm::Event::SupportedVersionChanged(
				MultiLocation {
					parents: 0,
					interior: X1(Parachain(KARURA_ID)),
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
			Box::new((Parent, Parachain(SIBLING_ID)).into()),
		);
		assert_ok!(r);
	});
	Karura::execute_with(|| {
		assert!(karura_runtime::System::events().iter().any(|r| matches!(
			r.event,
			karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::XcmpMessageSent {
				message_hash: Some(_)
			})
		)));
	});
	Sibling::execute_with(|| {
		assert!(System::events().iter().any(|r| matches!(
			r.event,
			karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::XcmpMessageSent {
				message_hash: Some(_)
			}) | karura_runtime::Event::XcmpQueue(cumulus_pallet_xcmp_queue::Event::Success {
				message_hash: Some(_),
				..
			})
		)));
	});
}

#[test]
fn unspent_xcm_fee_is_returned_correctly() {
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);
	let homa_lite_sub_account: AccountId =
		hex_literal::hex!["d7b8926b326dd349355a9a7cca6606c1e0eb6fd2b506066b518c7155ff0d8297"].into();
	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});
	let dollar_r = dollar(RELAY_CHAIN_CURRENCY);
	let dollar_n = dollar(NATIVE_CURRENCY);

	KusamaNet::execute_with(|| {
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(homa_lite_sub_account.clone()),
			1_000 * dollar_r
		));
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(parachain_account.clone()),
			1_000 * dollar_r
		));
		assert_eq!(
			kusama_runtime::Balances::free_balance(&AccountId::from(ALICE)),
			2 * dollar_r
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			1_000 * dollar_r
		);
		assert_eq!(kusama_runtime::Balances::free_balance(&AccountId::from(BOB)), 0);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account.clone()),
			1_002 * dollar_r
		);
	});

	Karura::execute_with(|| {
		// Construct a transfer XCM call with returning the deposit
		let transfer_call = RelayChainCallBuilder::<Runtime, ParachainInfo>::balances_transfer_keep_alive(
			AccountId::from(BOB),
			dollar_n,
		);
		let batch_call = RelayChainCallBuilder::<Runtime, ParachainInfo>::utility_as_derivative_call(transfer_call, 0);
		let weight = 10_000_000_000;
		// Fee to transfer into the hold register
		let asset = MultiAsset {
			id: Concrete(MultiLocation::here()),
			fun: Fungibility::Fungible(dollar_n),
		};
		let xcm_msg = Xcm(vec![
			WithdrawAsset(asset.clone().into()),
			BuyExecution {
				fees: asset,
				weight_limit: Unlimited,
			},
			Transact {
				origin_type: OriginKind::SovereignAccount,
				require_weight_at_most: weight,
				call: batch_call.encode().into(),
			},
		]);

		let res = PolkadotXcm::send_xcm(Here, Parent, xcm_msg);
		assert!(res.is_ok());
	});

	KusamaNet::execute_with(|| {
		// 1 dollar is transferred to BOB
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			999 * dollar_r
		);
		assert_eq!(kusama_runtime::Balances::free_balance(&AccountId::from(BOB)), dollar_r);
		// 1 dollar is given to Hold Register for XCM call and never returned.
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account.clone()),
			1_001 * dollar_r
		);
	});

	Karura::execute_with(|| {
		// Construct a transfer using the RelaychainCallBuilder
		let transfer_call = RelayChainCallBuilder::<Runtime, ParachainInfo>::balances_transfer_keep_alive(
			AccountId::from(BOB),
			dollar_n,
		);
		let batch_call = RelayChainCallBuilder::<Runtime, ParachainInfo>::utility_as_derivative_call(transfer_call, 0);
		let finalized_call = RelayChainCallBuilder::<Runtime, ParachainInfo>::finalize_call_into_xcm_message(
			batch_call,
			dollar_n,
			10_000_000_000,
		);

		let res = PolkadotXcm::send_xcm(Here, Parent, finalized_call);
		assert!(res.is_ok());
	});

	KusamaNet::execute_with(|| {
		// 1 dollar is transferred to BOB
		assert_eq!(
			kusama_runtime::Balances::free_balance(&homa_lite_sub_account),
			998 * dollar_r
		);
		assert_eq!(
			kusama_runtime::Balances::free_balance(&AccountId::from(BOB)),
			2 * dollar_r
		);
		// Unspent fund from the 1 dollar XCM fee is returned to the sovereign account.
		assert_eq!(
			kusama_runtime::Balances::free_balance(&parachain_account.clone()),
			1_000 * dollar_r + 999_601_783_448
		);
	});
}

#[test]
fn trap_assets_larger_than_ed_works() {
	TestNet::reset();

	let mut kar_treasury_amount = 0;
	let (ksm_asset_amount, kar_asset_amount) = (dollar(KSM), dollar(KAR));
	let trader_weight_to_treasury: u128 = relay_per_second_as_fee(3);

	let parent_account: AccountId = ParentIsPreset::<AccountId>::convert(Parent.into()).unwrap();

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(KSM, &parent_account, 100 * dollar(KSM)));
		let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&parent_account, 100 * dollar(KAR));

		kar_treasury_amount = Currencies::free_balance(KAR, &KaruraTreasuryAccount::get());
	});

	let assets: MultiAsset = (Parent, ksm_asset_amount).into();
	KusamaNet::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(dollar(KSM) as u64),
			},
			WithdrawAsset(((0, GeneralKey(KAR.encode())), kar_asset_amount).into()),
		];
		assert_ok!(pallet_xcm::Pallet::<kusama_runtime::Runtime>::send_xcm(
			Here,
			Parachain(KARURA_ID).into(),
			Xcm(xcm),
		));
	});

	Karura::execute_with(|| {
		assert!(System::events()
			.iter()
			.any(|r| matches!(r.event, Event::PolkadotXcm(pallet_xcm::Event::AssetsTrapped(_, _, _)))));

		assert_eq!(
			trader_weight_to_treasury + dollar(KSM),
			Currencies::free_balance(KSM, &KaruraTreasuryAccount::get())
		);
		assert_eq!(
			kar_treasury_amount,
			Currencies::free_balance(KAR, &KaruraTreasuryAccount::get())
		);
	});
}

#[test]
fn trap_assets_lower_than_ed_works() {
	TestNet::reset();

	let mut kar_treasury_amount = 0;
	let (ksm_asset_amount, kar_asset_amount) = (150_000_000, cent(KAR));

	let parent_account: AccountId = ParentIsPreset::<AccountId>::convert(Parent.into()).unwrap();

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(KSM, &parent_account, dollar(KSM)));
		let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&parent_account, dollar(KAR));
		kar_treasury_amount = Currencies::free_balance(KAR, &KaruraTreasuryAccount::get());
	});

	let assets: MultiAsset = (Parent, ksm_asset_amount).into();
	KusamaNet::execute_with(|| {
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Limited(dollar(KSM) as u64),
			},
			WithdrawAsset(((0, X1(GeneralKey(KAR.encode()))), kar_asset_amount).into()),
			// two asset left in holding register, they both lower than ED, so goes to treasury.
		];
		assert_ok!(pallet_xcm::Pallet::<kusama_runtime::Runtime>::send_xcm(
			Here,
			Parachain(KARURA_ID).into(),
			Xcm(xcm),
		));
	});

	Karura::execute_with(|| {
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::PolkadotXcm(pallet_xcm::Event::AssetsTrapped(_, _, _)))),
			None
		);

		assert_eq!(
			ksm_asset_amount + dollar(KSM),
			Currencies::free_balance(KSM, &KaruraTreasuryAccount::get())
		);
		assert_eq!(
			kar_asset_amount,
			Currencies::free_balance(KAR, &KaruraTreasuryAccount::get()) - kar_treasury_amount
		);
	});
}

#[test]
fn sibling_trap_assets_works() {
	TestNet::reset();

	let mut kar_treasury_amount = 0;
	let (bnc_asset_amount, kar_asset_amount) = (cent(BNC) / 10, cent(KAR));

	Karura::execute_with(|| {
		assert_ok!(Tokens::deposit(BNC, &sibling_reserve_account(), dollar(BNC)));
		let _ = pallet_balances::Pallet::<Runtime>::deposit_creating(&sibling_reserve_account(), dollar(KAR));
		kar_treasury_amount = Currencies::free_balance(KAR, &KaruraTreasuryAccount::get());
	});

	Sibling::execute_with(|| {
		let assets: MultiAsset = ((0, X1(GeneralKey(KAR.encode()))), kar_asset_amount).into();
		let xcm = vec![
			WithdrawAsset(assets.clone().into()),
			BuyExecution {
				fees: assets,
				weight_limit: Unlimited,
			},
			WithdrawAsset(
				(
					(Parent, X2(Parachain(BIFROST_ID), GeneralKey(BNC_KEY.to_vec()))),
					bnc_asset_amount,
				)
					.into(),
			),
		];
		assert_ok!(pallet_xcm::Pallet::<Runtime>::send_xcm(
			Here,
			(Parent, Parachain(KARURA_ID)),
			Xcm(xcm),
		));
	});

	Karura::execute_with(|| {
		assert_eq!(
			System::events()
				.iter()
				.find(|r| matches!(r.event, Event::PolkadotXcm(pallet_xcm::Event::AssetsTrapped(_, _, _)))),
			None
		);
		assert_eq!(
			Currencies::free_balance(KAR, &KaruraTreasuryAccount::get()) - kar_treasury_amount,
			kar_asset_amount
		);
		assert_eq!(
			Currencies::free_balance(BNC, &KaruraTreasuryAccount::get()),
			bnc_asset_amount
		);
	});
}
