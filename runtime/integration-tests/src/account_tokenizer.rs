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

use crate::relaychain::kusama_test_net::*;
use crate::setup::*;
use frame_support::{
	assert_ok,
	traits::{tokens::nonfungibles::Inspect, Hooks},
	weights::Weight,
};
use module_xcm_interface::XcmInterfaceOperation;
use orml_traits::CreateExtended;
use sp_runtime::MultiAddress;
use xcm_emulator::TestExt;

use hex_literal::hex;

// Weight and fee cost is related to the XCM_WEIGHT passed in.
const XCM_WEIGHT: Weight = 20_000_000_000;
const XCM_FEE: Balance = 10_000_000_000;
const ACTUAL_XCM_FEE: Balance = 639_999_960;

fn get_xcm_weight() -> Vec<(XcmInterfaceOperation, Option<Weight>, Option<Balance>)> {
	vec![(
		XcmInterfaceOperation::ProxyTransferProxy,
		Some(XCM_WEIGHT),
		Some(XCM_FEE),
	)]
}

#[test]
fn can_mint_account_token() {
	ExtBuilder::default()
		.balances(vec![(alice(), NATIVE_CURRENCY, 1_000 * dollar(NATIVE_CURRENCY))])
		.build()
		.execute_with(|| {
			Balances::make_free_balance_be(&KaruraTreasuryAccount::get(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(alice()), bob()));
			assert!(ForeignStateOracle::active_query(0).is_some());
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
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::root(), 0, vec![1]));

			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), Some(alice()));
			assert_eq!(AccountTokenizer::minted_account(bob()), Some(0));
			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(module_account_tokenizer::Event::AccountTokenMinted {
					owner: alice(),
					account: bob(),
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
				Balances::free_balance(&KaruraTreasuryAccount::get()),
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
			Balances::make_free_balance_be(&KaruraTreasuryAccount::get(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(alice()), bob()));

			// Dispatch the request to reject the mint.
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::root(), 0, vec![0]));

			// NFT token is NOT minted
			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), None);
			assert_eq!(AccountTokenizer::minted_account(bob()), None);

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
				Balances::free_balance(&KaruraTreasuryAccount::get()),
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
			let proxy = AccountId::from([0u8; 32]);
			Balances::make_free_balance_be(&KaruraTreasuryAccount::get(), 1_000 * dollar(NATIVE_CURRENCY));
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(alice()), proxy.clone()));

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::root(), 0, vec![1]));

			assert_eq!(AccountTokenizer::nft_class_id(), 0);
			assert_eq!(AccountTokenizer::minted_account(&proxy), Some(0));
			// Transfer the token to bob
			assert_ok!(NFT::transfer(Origin::signed(alice()), bob().into(), (0, 0),));

			// Only the owner is allowed to burn the token
			assert_noop!(
				AccountTokenizer::burn_account_token(Origin::signed(alice()), proxy.clone(), bob(),),
				module_account_tokenizer::Error::<Runtime>::CallerUnauthorized
			);

			assert_eq!(NFT::owner(&AccountTokenizer::nft_class_id(), &0), Some(bob()));
			assert_eq!(AccountTokenizer::minted_account(&proxy), Some(0));

			// Burn the token by the token owner
			assert_ok!(AccountTokenizer::burn_account_token(
				Origin::signed(bob()),
				proxy.clone(),
				bob(),
			));

			System::assert_last_event(Event::AccountTokenizer(
				module_account_tokenizer::Event::AccountTokenBurned {
					account: proxy,
					owner: bob(),
					token_id: 0,
					new_owner: bob(),
				},
			));
		});
}

#[test]
fn xcm_transfer_proxy_for_burn_works() {
	let mut proxy = AccountId::new([0u8; 32]);
	let mut parachain_account: AccountId = AccountId::new([0u8; 32]);

	Karura::execute_with(|| {
		parachain_account = ParachainAccount::get();
	});

	KusamaNet::execute_with(|| {
		// Give the control of an account to karura's parachain account.
		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(parachain_account.clone()),
			1_000 * dollar(RELAY_CHAIN_CURRENCY)
		));

		// Spawn a anonymous proxy account.
		assert_ok!(kusama_runtime::Proxy::anonymous(
			kusama_runtime::Origin::signed(ALICE.into()),
			Default::default(),
			0,
			0u16,
		));
		proxy = AccountId::new(hex!["a09745e940e6170996c8d0d5961dacdbf551546fbb394e0ea59841a18be7f1eb"]);
		kusama_runtime::System::assert_last_event(kusama_runtime::Event::Proxy(
			pallet_proxy::Event::AnonymousCreated {
				anonymous: proxy.clone(),
				who: ALICE.into(),
				proxy_type: Default::default(),
				disambiguation_index: 0,
			},
		));

		assert_ok!(kusama_runtime::Balances::transfer(
			kusama_runtime::Origin::signed(ALICE.into()),
			MultiAddress::Id(proxy.clone()),
			dollar(RELAY_CHAIN_CURRENCY)
		));

		// Transfer the proxy control to parachain account.
		assert_ok!(kusama_runtime::Proxy::add_proxy(
			kusama_runtime::Origin::signed(proxy.clone().into()),
			parachain_account,
			Default::default(),
			0,
		));

		assert_ok!(kusama_runtime::Proxy::remove_proxy(
			kusama_runtime::Origin::signed(proxy.clone().into()),
			ALICE.into(),
			Default::default(),
			0,
		));
	});

	Karura::execute_with(|| {
		Balances::make_free_balance_be(&KaruraTreasuryAccount::get(), 1_000 * dollar(NATIVE_CURRENCY));
		Balances::make_free_balance_be(&alice(), 1_000 * dollar(NATIVE_CURRENCY));
		Balances::make_free_balance_be(&bob(), 1_000 * dollar(NATIVE_CURRENCY));
		assert_ok!(XcmInterface::update_xcm_dest_weight_and_fee(
			Origin::root(),
			get_xcm_weight()
		));
		AccountTokenizer::on_runtime_upgrade();

		// Mint an Account Token.
		assert_ok!(AccountTokenizer::request_mint(Origin::signed(alice()), proxy.clone()));
		assert_ok!(ForeignStateOracle::dispatch_task(Origin::root(), 0, vec![1]));

		// Transfer the token to bob
		assert_ok!(NFT::transfer(Origin::signed(alice()), bob().into(), (0, 0),));

		// Burn the token by the token owner
		assert_ok!(AccountTokenizer::burn_account_token(
			Origin::signed(bob()),
			proxy.clone(),
			bob(),
		));
	});
}
