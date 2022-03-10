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

use crate::setup::*;
use frame_support::{
	assert_ok,
	traits::{tokens::nonfungibles::Inspect, Hooks},
};
use hex_literal::hex;
use module_support::CreateExtended;

fn get_treasury_account() -> AccountId {
	#[cfg(feature = "with-mandala-runtime")]
	return TreasuryAccount::get();

	#[cfg(feature = "with-karura-runtime")]
	return KaruraTreasuryAccount::get();

	#[cfg(feature = "with-acala-runtime")]
	return AcalaTreasuryAccount::get();
}

#[test]
fn can_mint_account_token() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY))])
		.build()
		.execute_with(|| {
			Balances::make_free_balance_be(&get_treasury_account(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();
			let alice_proxy = AccountId::new(hex!["b99bbff5de2888225d1b0fcdba9c4e79117f910ae30b042618fecf87bd860316"]);
			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(
				Origin::signed(alice()),
				alice_proxy.clone(),
				1,
				0,
				0
			));
			assert!(ForeignStateOracle::query_requests(0).is_some());
			// Deposit is reserved when mint is requested.
			assert_eq!(
				Balances::reserved_balance(&alice()),
				AccountTokenizerMintRequestDeposit::get()
			);
			assert_eq!(
				Balances::free_balance(&alice()),
				1_000 * dollar(NATIVE_CURRENCY)
					- AccountTokenizerMintFee::get()
					- AccountTokenizerMintRequestDeposit::get()
					- QueryFee::get()
			);

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				0,
				vec![1]
			));

			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), Some(alice()));
			assert_eq!(AccountTokenizer::minted_account(&alice_proxy), Some(0));
			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(module_account_tokenizer::Event::AccountTokenMinted {
					owner: alice(),
					account: alice_proxy.clone(),
					token_id: 0,
				})
			);

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched { task_result: Ok(()) },
			));

			// Deposit is returned to the owner after mint is successful.
			// Oracle query fee and AccountTokenizer MintFee is deducted
			assert_eq!(
				Balances::free_balance(&alice()),
				1_000 * dollar(NATIVE_CURRENCY) - AccountTokenizerMintFee::get() - QueryFee::get()
			);
			assert!(AccountTokenizerMintFee::get() >= NFT::base_mint_fee());

			// Treasury pays for token mint and NFT class creation.
			assert_eq!(
				Balances::free_balance(&get_treasury_account()),
				1000 * dollar(NATIVE_CURRENCY) + AccountTokenizerMintFee::get()
					- NFT::base_create_class_fee()
					- NFT::base_mint_fee()
			);
		});
}

#[test]
fn can_reject_mint_request() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY))])
		.build()
		.execute_with(|| {
			Balances::make_free_balance_be(&get_treasury_account(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();
			let alice_proxy = AccountId::new(hex!["b99bbff5de2888225d1b0fcdba9c4e79117f910ae30b042618fecf87bd860316"]);

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(
				Origin::signed(alice()),
				alice_proxy.clone(),
				1,
				0,
				0
			));

			// Dispatch the request to reject the mint.
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				0,
				vec![0]
			));

			// NFT token is NOT minted
			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), None);
			assert_eq!(AccountTokenizer::minted_account(alice_proxy.clone()), None);

			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(module_account_tokenizer::Event::MintRequestRejected { requester: alice() })
			);

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched { task_result: Ok(()) },
			));

			// Deposit is repatriated to the treasury when mint is rejected.
			assert_eq!(
				Balances::free_balance(&alice()),
				1_000 * dollar(NATIVE_CURRENCY)
					- AccountTokenizerMintFee::get()
					- QueryFee::get() - AccountTokenizerMintRequestDeposit::get()
			);

			// Treasury pays for token mint and NFT class creation.
			assert_eq!(
				Balances::free_balance(&get_treasury_account()),
				1000 * dollar(NATIVE_CURRENCY)
					+ AccountTokenizerMintFee::get()
					+ AccountTokenizerMintRequestDeposit::get()
					- NFT::base_create_class_fee()
			);
		});
}

#[test]
fn can_burn_account_token_nft() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(alice()),
				NATIVE_CURRENCY,
				1_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(bob()), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			Balances::make_free_balance_be(&get_treasury_account(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();
			let alice_proxy = AccountId::new(hex!["b99bbff5de2888225d1b0fcdba9c4e79117f910ae30b042618fecf87bd860316"]);

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(
				Origin::signed(alice()),
				alice_proxy.clone(),
				1,
				0,
				0
			));

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				0,
				vec![1]
			));

			assert_eq!(AccountTokenizer::nft_class_id(), 0);
			assert_eq!(AccountTokenizer::minted_account(&alice_proxy), Some(0));
			// Transfer the token to bob
			assert_ok!(NFT::transfer(Origin::signed(alice()), bob().into(), (0, 0),));

			// Only the owner is allowed to burn the token
			assert_noop!(
				AccountTokenizer::request_redeem(Origin::signed(alice()), alice_proxy.clone(), bob(),),
				module_account_tokenizer::Error::<Runtime>::CallerUnauthorized
			);

			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), Some(bob()));
			assert_eq!(AccountTokenizer::minted_account(&alice_proxy), Some(0));

			// Burn the token by the token owner
			assert_ok!(AccountTokenizer::request_redeem(
				Origin::signed(bob()),
				alice_proxy.clone(),
				bob(),
			));

			// Dispatch the request to accept the burn.
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				1,
				vec![]
			));

			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(module_account_tokenizer::Event::AccountTokenRedeemed {
					account: alice_proxy,
					owner: bob(),
					token_id: 0,
					new_owner: bob(),
				})
			);

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched { task_result: Ok(()) },
			));
		});
}

#[cfg(feature = "with-karura-runtime")]
pub mod xcm_test {
	use super::*;
	use crate::relaychain::kusama_test_net::*;

	use frame_support::weights::Weight;
	use module_xcm_interface::XcmInterfaceOperation;
	use sp_runtime::MultiAddress;
	use xcm_emulator::TestExt;

	// Weight and fee cost is related to the XCM_WEIGHT passed in.
	const XCM_WEIGHT: Weight = 30_000_000_000;
	const XCM_FEE: Balance = 1_500_000_000;
	const ACTUAL_XCM_FEE: Balance = 906_666_610;

	fn get_xcm_weight() -> Vec<(XcmInterfaceOperation, Option<Weight>, Option<Balance>)> {
		vec![(
			XcmInterfaceOperation::ProxyTransferProxy,
			Some(XCM_WEIGHT),
			Some(XCM_FEE),
		)]
	}

	#[test]
	fn xcm_transfer_proxy_for_burn_works() {
		let mut alice_proxy = AccountId::new([0u8; 32]);
		let mut parachain_account: AccountId = AccountId::new([0u8; 32]);

		Karura::execute_with(|| {
			parachain_account = ParachainAccount::get();
		});

		KusamaNet::execute_with(|| {
			// Give the control of an account to karura's parachain account.
			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(ALICE.into()),
				MultiAddress::Id(parachain_account.clone()),
				500 * dollar(RELAY_CHAIN_CURRENCY)
			));
			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(ALICE.into()),
				MultiAddress::Id(alice()),
				500 * dollar(RELAY_CHAIN_CURRENCY)
			));
			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(ALICE.into()),
				MultiAddress::Id(bob()),
				500 * dollar(RELAY_CHAIN_CURRENCY)
			));

			assert_eq!(
				kusama_runtime::Balances::free_balance(parachain_account.clone()),
				502 * dollar(RELAY_CHAIN_CURRENCY)
			);

			// Spawn a anonymous proxy account.
			assert_ok!(kusama_runtime::Proxy::anonymous(
				kusama_runtime::Origin::signed(alice()),
				Default::default(),
				0,
				0u16,
			));
			alice_proxy = AccountId::new(hex!["b99bbff5de2888225d1b0fcdba9c4e79117f910ae30b042618fecf87bd860316"]);
			kusama_runtime::System::assert_last_event(kusama_runtime::Event::Proxy(
				pallet_proxy::Event::AnonymousCreated {
					anonymous: alice_proxy.clone(),
					who: alice(),
					proxy_type: Default::default(),
					disambiguation_index: 0,
				},
			));

			assert_ok!(kusama_runtime::Balances::transfer(
				kusama_runtime::Origin::signed(alice()),
				MultiAddress::Id(alice_proxy.clone()),
				dollar(RELAY_CHAIN_CURRENCY)
			));

			// Transfer the proxy control to parachain account.
			assert_ok!(kusama_runtime::Proxy::add_proxy(
				kusama_runtime::Origin::signed(alice_proxy.clone().into()),
				parachain_account.clone(),
				Default::default(),
				0,
			));

			assert_ok!(kusama_runtime::Proxy::remove_proxy(
				kusama_runtime::Origin::signed(alice_proxy.clone().into()),
				alice(),
				Default::default(),
				0,
			));

			assert_eq!(
				kusama_runtime::Proxy::proxies(alice_proxy.clone()).0.into_inner(),
				vec![pallet_proxy::ProxyDefinition {
					delegate: parachain_account.clone(),
					proxy_type: Default::default(),
					delay: 0u32
				}]
			);

			// Uncomment this test the transfer of proxy can be done on the relaychain
			/*
			let transfer_proxy_call = kusama_runtime::Call::Utility(pallet_utility::Call::batch {
				calls: vec![
					kusama_runtime::Call::Proxy(pallet_proxy::Call::add_proxy {
						delegate: alice(),
						proxy_type: Default::default(),
						delay: 0u32,
					}),
					kusama_runtime::Call::Proxy(pallet_proxy::Call::remove_proxy {
						delegate: parachain_account.clone(),
						proxy_type: Default::default(),
						delay: 0u32,
					}),
				]
			});

			assert_ok!(kusama_runtime::Proxy::proxy(
				kusama_runtime::Origin::signed(parachain_account.clone().into()),
				alice_proxy.clone(),
				None,
				Box::new(transfer_proxy_call),
			));

			assert_eq!(kusama_runtime::Proxy::proxies(alice_proxy.clone()).0.into_inner(),
			vec![pallet_proxy::ProxyDefinition { delegate: alice(), proxy_type: Default::default(),
			delay: 0u32 }]);
			*/
		});

		Karura::execute_with(|| {
			Balances::make_free_balance_be(&get_treasury_account(), 1_000 * dollar(NATIVE_CURRENCY));
			Balances::make_free_balance_be(&alice(), 1_000 * dollar(NATIVE_CURRENCY));
			Balances::make_free_balance_be(&bob(), 1_000 * dollar(NATIVE_CURRENCY));
			assert_ok!(XcmInterface::update_xcm_dest_weight_and_fee(
				Origin::root(),
				get_xcm_weight()
			));
			AccountTokenizer::on_runtime_upgrade();
			// Mint an Account Token.
			assert_ok!(AccountTokenizer::request_mint(
				Origin::signed(alice()),
				alice_proxy.clone(),
				1,
				0,
				0
			));
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				0,
				vec![1]
			));

			// Transfer the token to bob
			assert_ok!(NFT::transfer(Origin::signed(alice()), bob().into(), (0, 0),));

			// Burn the token by the token owner
			assert_ok!(AccountTokenizer::request_redeem(
				Origin::signed(bob()),
				alice_proxy.clone(),
				bob(),
			));
			assert_ok!(ForeignStateOracle::respond_query_request(
				OriginCaller::ForeignStateOracleCommittee(pallet_collective::RawOrigin::Members(1, 1)).into(),
				1,
				vec![]
			));
		});

		KusamaNet::execute_with(|| {
			assert_eq!(
				kusama_runtime::Proxy::proxies(alice_proxy.clone()).0.into_inner(),
				vec![pallet_proxy::ProxyDefinition {
					delegate: bob(),
					proxy_type: Default::default(),
					delay: 0u32
				}]
			);
			assert_eq!(
				kusama_runtime::Balances::free_balance(parachain_account.clone()),
				502 * dollar(RELAY_CHAIN_CURRENCY) - ACTUAL_XCM_FEE
			);
		});
	}
}
