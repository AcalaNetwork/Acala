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

//! Unit tests for example module.

#![cfg(test)]

use crate::mock::*;
use frame_support::{
	assert_noop, assert_ok,
	dispatch::DispatchError,
	traits::{tokens::nonfungibles::Inspect, Hooks},
};
use orml_traits::CreateExtended;

use sp_runtime::traits::{AccountIdConversion, BadOrigin};

#[test]
fn can_create_nft() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			// create a NFT so the class ID isn't 0
			assert_ok!(ModuleNFT::create_class(
				Origin::signed(ALICE),
				Default::default(),
				Default::default(),
				Default::default(),
			));

			assert_eq!(ModuleNFT::next_class_id(), 1);
			let event = Event::ModuleNFT(module_nft::Event::CreatedClass {
				owner: NftPalletId::get().into_sub_account(1),
				admin: AccountTokenizerPalletAccount::get(),
				class_id: 1,
			});

			// on runtime upgrade can create new NFT class
			AccountTokenizer::on_runtime_upgrade();

			assert_eq!(AccountTokenizer::nft_class_id(), 1);
			System::assert_last_event(event.clone());

			// Will not re-create the runtime NFT class.
			AccountTokenizer::on_runtime_upgrade();

			assert_eq!(AccountTokenizer::nft_class_id(), 1);
			System::assert_last_event(event);
		});
}

#[test]
fn can_send_mint_request() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			// on runtime upgrade can create new NFT class
			AccountTokenizer::on_runtime_upgrade();

			assert_eq!(ForeignStateOracle::query_index(), 0);
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(ALICE), ALICE));
			System::assert_last_event(Event::AccountTokenizer(crate::Event::MintRequested {
				account: ALICE,
				who: ALICE,
			}));
			// free_balance = 1000 - 1(mint fee) - 1(query fee) - 10 (deposit)
			assert_eq!(Balances::free_balance(&ALICE), dollar(988));
			assert_eq!(Balances::reserved_balance(&ALICE), dollar(10));
			assert_eq!(Balances::free_balance(&TREASURY), dollar(1));

			assert_eq!(ForeignStateOracle::query_index(), 1);

			assert!(ForeignStateOracle::active_query(0).is_some());
		});
}

#[test]
fn can_mint_account_token_nft() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(ALICE), PROXY));
			assert!(ForeignStateOracle::active_query(0).is_some());

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::signed(ORACLE), 0, vec![1]));

			assert_eq!(ModuleNFT::owner(&0, &0), Some(ALICE));
			assert_eq!(AccountTokenizer::minted_account(PROXY), Some(0));
			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(crate::Event::AccountTokenMinted {
					owner: ALICE,
					account: PROXY,
					token_id: 0,
				})
			);

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched { task_result: Ok(()) },
			));

			// Deposit is returned to the owner after mint is successful
			assert_eq!(Balances::free_balance(&ALICE), dollar(998));
			assert_eq!(Balances::reserved_balance(&ALICE), 0);
			assert_eq!(Balances::free_balance(&TREASURY), dollar(1));
		});
}

#[test]
fn can_handle_bad_oracle_data() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(ALICE), PROXY));
			assert!(ForeignStateOracle::active_query(0).is_some());

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::signed(ORACLE), 0, vec![]));

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched {
					task_result: Err(DispatchError::Module {
						index: 6u8,
						error: 3u8,
						message: None,
					}),
				},
			));
		});
}

#[test]
fn can_reject_mint_request() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			AccountTokenizer::on_runtime_upgrade();

			// Send a mint request.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(ALICE), PROXY));
			assert!(ForeignStateOracle::active_query(0).is_some());

			// Dispatch the request to accept the mint.
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::signed(ORACLE), 0, vec![0]));

			assert_eq!(ModuleNFT::owner(&0, &0), None);
			assert_eq!(AccountTokenizer::minted_account(PROXY), None);

			let events = System::events();
			assert_eq!(
				events[events.len() - 2].event,
				Event::AccountTokenizer(crate::Event::MintRequestRejected { requester: ALICE })
			);

			System::assert_last_event(Event::ForeignStateOracle(
				module_foreign_state_oracle::Event::CallDispatched { task_result: Ok(()) },
			));

			// Deposit is repatriated to the treasury due to the rejection of the request.
			assert_eq!(Balances::free_balance(&ALICE), dollar(988));
			assert_eq!(Balances::reserved_balance(&ALICE), 0);
			assert_eq!(Balances::free_balance(&TREASURY), dollar(11));
		});
}

#[test]
fn confirm_request_cannot_be_called_via_extrinsic() {
	ExtBuilder::default().build().execute_with(|| {
		AccountTokenizer::on_runtime_upgrade();

		assert_noop!(
			AccountTokenizer::confirm_mint_request(Origin::signed(ALICE), ALICE, PROXY),
			BadOrigin
		);
		assert_noop!(
			AccountTokenizer::confirm_mint_request(Origin::root(), ALICE, PROXY),
			BadOrigin
		);
		assert_noop!(
			AccountTokenizer::confirm_mint_request(Origin::signed(ORACLE), ALICE, PROXY),
			BadOrigin
		);
	});
}

#[test]
fn can_burn_account_token_nft() {
	ExtBuilder::default()
		.balances(vec![(ALICE, dollar(1_000))])
		.build()
		.execute_with(|| {
			AccountTokenizer::on_runtime_upgrade();

			// Mint the NFT.
			assert_ok!(AccountTokenizer::request_mint(Origin::signed(ALICE), PROXY));
			assert_ok!(ForeignStateOracle::dispatch_task(Origin::signed(ORACLE), 0, vec![1]));

			assert_eq!(ModuleNFT::owner(&0, &0), Some(ALICE));
			assert_eq!(AccountTokenizer::minted_account(PROXY), Some(0));

			// Burn the NFT
			// only the owner of the NFT can burn the token
			assert_noop!(
				AccountTokenizer::burn_account_token(Origin::signed(PROXY), PROXY, ALICE),
				crate::Error::<Runtime>::CallerUnauthorized
			);

			assert_ok!(AccountTokenizer::burn_account_token(
				Origin::signed(ALICE),
				PROXY,
				ALICE
			));

			assert_eq!(ModuleNFT::owner(&0, &0), None);
			assert_eq!(AccountTokenizer::minted_account(PROXY), None);

			System::assert_last_event(Event::AccountTokenizer(crate::Event::AccountTokenBurned {
				account: PROXY,
				owner: ALICE,
				token_id: 0,
				new_owner: ALICE,
			}));

			// XCM fee is burned.
			assert_eq!(Balances::free_balance(&ALICE), dollar(993));
			assert_eq!(Balances::reserved_balance(&ALICE), 0);
			assert_eq!(Balances::free_balance(&TREASURY), dollar(1));

			// cannot burn the same nft again
			assert_noop!(
				AccountTokenizer::burn_account_token(Origin::signed(ALICE), PROXY, ALICE),
				crate::Error::<Runtime>::AccountTokenNotFound
			);
		});
}
