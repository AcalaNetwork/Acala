// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

//! Unit tests for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{RuntimeCall as MockCall, RuntimeEvent, *};
use module_support::DEXManager;
use sp_core::offchain::{testing, DbExternalities, OffchainDbExt, OffchainWorkerExt, StorageKind, TransactionPoolExt};
use sp_io::offchain;
use sp_runtime::traits::One;

fn run_to_block_offchain(n: u64) {
	while System::block_number() < n {
		System::set_block_number(System::block_number() + 1);
		AuctionManagerModule::offchain_worker(System::block_number());
		// this unlocks the concurrency storage lock so offchain_worker will fire next block
		offchain::sleep_until(offchain::timestamp().add(Duration::from_millis(LOCK_DURATION + 200)));
	}
}

#[test]
fn get_auction_time_to_close_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(AuctionManagerModule::get_auction_time_to_close(2000, 1), 100);
		assert_eq!(AuctionManagerModule::get_auction_time_to_close(2001, 1), 50);
	});
}

#[test]
fn collateral_auction_methods() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert_eq!(
			AuctionModule::auctions(0),
			Some(orml_traits::AuctionInfo {
				bid: None,
				start: 0,
				end: Some(2000)
			})
		);
		let collateral_auction_with_positive_target = AuctionManagerModule::collateral_auctions(0).unwrap();
		assert!(!collateral_auction_with_positive_target.always_forward());
		assert!(!collateral_auction_with_positive_target.in_reverse_stage(99));
		assert!(collateral_auction_with_positive_target.in_reverse_stage(100));
		assert!(collateral_auction_with_positive_target.in_reverse_stage(101));
		assert_eq!(collateral_auction_with_positive_target.payment_amount(99), 99);
		assert_eq!(collateral_auction_with_positive_target.payment_amount(100), 100);
		assert_eq!(collateral_auction_with_positive_target.payment_amount(101), 100);
		assert_eq!(collateral_auction_with_positive_target.collateral_amount(80, 100), 10);
		assert_eq!(collateral_auction_with_positive_target.collateral_amount(100, 200), 5);

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 0));
		let collateral_auction_with_zero_target = AuctionManagerModule::collateral_auctions(1).unwrap();
		assert!(collateral_auction_with_zero_target.always_forward());
		assert!(!collateral_auction_with_zero_target.in_reverse_stage(0));
		assert!(!collateral_auction_with_zero_target.in_reverse_stage(100));
		assert_eq!(collateral_auction_with_zero_target.payment_amount(99), 99);
		assert_eq!(collateral_auction_with_zero_target.payment_amount(101), 101);
		assert_eq!(collateral_auction_with_zero_target.collateral_amount(100, 200), 10);
	});
}

#[test]
fn new_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		let ref_count_0 = System::consumers(&ALICE);
		assert_noop!(
			AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 0, 100),
			Error::<Runtime>::InvalidAmount,
		);

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(crate::Event::NewCollateralAuction {
			auction_id: 0,
			collateral_type: BTC,
			collateral_amount: 10,
			target_bid_price: 100,
		}));

		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);
		assert_eq!(System::consumers(&ALICE), ref_count_0 + 1);

		assert_noop!(
			AuctionManagerModule::new_collateral_auction(&ALICE, BTC, Balance::max_value(), Balance::max_value()),
			Error::<Runtime>::InvalidAmount,
		);
	});
}

#[test]
fn collateral_auction_bid_handler_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::collateral_auction_bid_handler(1, 0, (BOB, 4), None),
			Error::<Runtime>::AuctionNotExists,
		);

		assert_ok!(CDPTreasuryModule::deposit_collateral(&ALICE, BTC, 10));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);

		let bob_ref_count_0 = System::consumers(&BOB);

		assert_noop!(
			AuctionManagerModule::collateral_auction_bid_handler(1, 0, (BOB, 4), None),
			Error::<Runtime>::InvalidBidPrice,
		);
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 5),
			None
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 5);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 995);

		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 + 1);
		let carol_ref_count_0 = System::consumers(&CAROL);

		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			2,
			0,
			(CAROL, 10),
			Some((BOB, 5))
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 10);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 990);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).unwrap().amount, 10);

		let bob_ref_count_2 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_2, bob_ref_count_1 - 1);
		let carol_ref_count_1 = System::consumers(&CAROL);
		assert_eq!(carol_ref_count_1, carol_ref_count_0 + 1);

		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			3,
			0,
			(BOB, 200),
			Some((CAROL, 10))
		));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 1000);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).unwrap().amount, 5);

		let bob_ref_count_3 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_3, bob_ref_count_2 + 1);
		let carol_ref_count_2 = System::consumers(&CAROL);
		assert_eq!(carol_ref_count_2, carol_ref_count_1 - 1);
	});
}

#[test]
fn bid_when_soft_cap_for_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end_change,
			Change::NewValue(Some(101))
		);
		assert!(!AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 10), Some((BOB, 5))).accept_bid,);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 15), Some((BOB, 5))).auction_end_change,
			Change::NewValue(Some(2051))
		);
	});
}

#[test]
fn always_forward_collateral_auction_without_bid_taked_by_dex() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));

		assert_ok!(AuctionManagerModule::new_collateral_auction(
			&CDPTreasuryModule::account_id(),
			BTC,
			100,
			0
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		let ref_count_0 = System::consumers(&CDPTreasuryModule::account_id());

		AuctionManagerModule::on_auction_ended(0, None);
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::DEXTakeCollateralAuction {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				supply_collateral_amount: 100,
				target_stable_amount: 500,
			},
		));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (200, 500));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 500);
		let ref_count_1 = System::consumers(&CDPTreasuryModule::account_id());
		assert_eq!(ref_count_1, ref_count_0 - 1);
	});
}

#[test]
fn always_forward_collateral_auction_without_bid_aborted() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(
			&CDPTreasuryModule::account_id(),
			BTC,
			100,
			0
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		let ref_count_0 = System::consumers(&CDPTreasuryModule::account_id());

		AuctionManagerModule::on_auction_ended(0, None);
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::CollateralAuctionAborted {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				target_stable_amount: 0,
				refund_recipient: CDPTreasuryModule::account_id(),
			},
		));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		let ref_count_1 = System::consumers(&CDPTreasuryModule::account_id());
		assert_eq!(ref_count_1, ref_count_0 - 1);
	});
}

#[test]
fn always_forward_collateral_auction_dealt() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(
			&CDPTreasuryModule::account_id(),
			BTC,
			100,
			0
		));
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 200),
			None
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		let ref_count_0 = System::consumers(&CDPTreasuryModule::account_id());
		let bob_ref_count_0 = System::consumers(&BOB);

		AuctionManagerModule::on_auction_ended(0, Some((BOB, 200)));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::CollateralAuctionDealt {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				winner: BOB,
				payment_amount: 200,
			},
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1100);
		let ref_count_1 = System::consumers(&CDPTreasuryModule::account_id());
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(ref_count_1, ref_count_0 - 1);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn always_forward_collateral_auction_with_bid_taked_by_dex() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));

		assert_ok!(AuctionManagerModule::new_collateral_auction(
			&CDPTreasuryModule::account_id(),
			BTC,
			100,
			0
		));
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 500),
			None
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 500);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 500);
		let ref_count_0 = System::consumers(&CDPTreasuryModule::account_id());
		let bob_ref_count_0 = System::consumers(&BOB);

		AuctionManagerModule::on_auction_ended(0, Some((BOB, 500)));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::DEXTakeCollateralAuction {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				supply_collateral_amount: 100,
				target_stable_amount: 500,
			},
		));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (200, 500));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 500);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		let ref_count_1 = System::consumers(&CDPTreasuryModule::account_id());
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(ref_count_1, ref_count_0 - 1);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn reverse_collateral_auction_with_bid_taked_by_dex() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 200),
			None
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (100, 1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1000);
		let bob_ref_count_0 = System::consumers(&BOB);

		AuctionManagerModule::on_auction_ended(0, Some((BOB, 200)));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::DEXTakeCollateralAuction {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				supply_collateral_amount: 26,
				target_stable_amount: 200,
			},
		));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (126, 800));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 400);
		assert_eq!(CDPTreasuryModule::debit_pool(), 200);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1074);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn reverse_collateral_auction_with_bid_dealt() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 250),
			None
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 80);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 80);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1020);
		let alice_ref_count_0 = System::consumers(&ALICE);

		AuctionManagerModule::on_auction_ended(0, Some((BOB, 250)));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::CollateralAuctionDealt {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 80,
				winner: BOB,
				payment_amount: 200,
			},
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1080);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1020);
		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
	});
}

#[test]
fn collateral_auction_with_bid_aborted() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(DEXModule::add_liquidity(
			RuntimeOrigin::signed(CAROL),
			BTC,
			AUSD,
			500,
			1000,
			0,
			false
		));

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_ok!(AuctionManagerModule::collateral_auction_bid_handler(
			1,
			0,
			(BOB, 180),
			None
		));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (500, 1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 180);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 820);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1000);
		let alice_ref_count_0 = System::consumers(&ALICE);

		AuctionManagerModule::on_auction_ended(0, Some((BOB, 180)));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(
			crate::Event::CollateralAuctionAborted {
				auction_id: 0,
				collateral_type: BTC,
				collateral_amount: 100,
				target_stable_amount: 200,
				refund_recipient: ALICE,
			},
		));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(DEXModule::get_liquidity_pool(BTC, AUSD), (500, 1000));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 180);
		assert_eq!(CDPTreasuryModule::debit_pool(), 180);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1000);
		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
	});
}

#[test]
fn swap_bidders_works() {
	ExtBuilder::default().build().execute_with(|| {
		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		AuctionManagerModule::swap_bidders(&BOB, None);

		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 + 1);

		AuctionManagerModule::swap_bidders(&ALICE, Some(&BOB));

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 + 1);
		let bob_ref_count_2 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_2, bob_ref_count_1 - 1);

		AuctionManagerModule::swap_bidders(&BOB, Some(&ALICE));

		let alice_ref_count_2 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_2, alice_ref_count_1 - 1);
		let bob_ref_count_3 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_3, bob_ref_count_2 + 1);
	});
}

#[test]
fn cancel_collateral_auction_failed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 10));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		MockPriceSource::set_relative_price(None);
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0, AuctionManagerModule::collateral_auctions(0).unwrap()),
			Error::<Runtime>::InvalidFeedPrice,
		);
		MockPriceSource::set_relative_price(Some(Price::one()));

		assert_ok!(AuctionModule::bid(RuntimeOrigin::signed(ALICE), 0, 100));
		let collateral_auction = AuctionManagerModule::collateral_auctions(0).unwrap();
		assert!(!collateral_auction.always_forward());
		assert_eq!(AuctionManagerModule::get_last_bid(0), Some((ALICE, 100)));
		assert!(collateral_auction.in_reverse_stage(100));
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0, collateral_auction),
			Error::<Runtime>::InReverseStage,
		);
	});
}

#[test]
fn cancel_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 10));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(AuctionModule::bid(RuntimeOrigin::signed(BOB), 0, 80));
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 920);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 920);

		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		mock_shutdown();
		assert_ok!(AuctionManagerModule::cancel(RuntimeOrigin::none(), 0));
		System::assert_last_event(RuntimeEvent::AuctionManagerModule(crate::Event::CancelAuction {
			auction_id: 0,
		}));

		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::debit_pool(), 80);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert!(!AuctionManagerModule::collateral_auctions(0).is_some());
		assert!(!AuctionModule::auction_info(0).is_some());

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn offchain_worker_cancels_auction_in_shutdown() {
	let (offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain));

	ext.execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert!(AuctionManagerModule::collateral_auctions(0).is_some());
		run_to_block_offchain(2);
		// offchain worker does not have any tx because shutdown is false
		assert!(!MockEmergencyShutdown::is_shutdown());
		assert!(pool_state.write().transactions.pop().is_none());
		mock_shutdown();
		assert!(MockEmergencyShutdown::is_shutdown());

		// now offchain worker will cancel auction as shutdown is true
		run_to_block_offchain(3);
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::AuctionManagerModule(crate::Call::cancel { id: auction_id }) = tx.call {
			assert_ok!(AuctionManagerModule::cancel(RuntimeOrigin::none(), auction_id));
		}

		// auction is canceled
		assert!(AuctionManagerModule::collateral_auctions(0).is_none());
		assert!(pool_state.write().transactions.pop().is_none());
	});
}

#[test]
fn offchain_worker_max_iterations_check() {
	let (mut offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::default().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		// sets max iterations value to 1
		offchain.local_storage_set(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS, &1u32.encode());
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&BOB, BTC, 10, 100));
		assert!(AuctionManagerModule::collateral_auctions(1).is_some());
		assert!(AuctionManagerModule::collateral_auctions(0).is_some());
		mock_shutdown();
		assert!(MockEmergencyShutdown::is_shutdown());

		run_to_block_offchain(2);
		// now offchain worker will cancel one auction but the other one will cancel next block
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::AuctionManagerModule(crate::Call::cancel { id: auction_id }) = tx.call {
			assert_ok!(AuctionManagerModule::cancel(RuntimeOrigin::none(), auction_id));
		}
		assert!(
			AuctionManagerModule::collateral_auctions(1).is_some()
				|| AuctionManagerModule::collateral_auctions(0).is_some()
		);
		// only one auction canceled so offchain tx pool is empty
		assert!(pool_state.write().transactions.pop().is_none());

		run_to_block_offchain(3);
		// now offchain worker will cancel the next auction
		let tx = pool_state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		if let MockCall::AuctionManagerModule(crate::Call::cancel { id: auction_id }) = tx.call {
			assert_ok!(AuctionManagerModule::cancel(RuntimeOrigin::none(), auction_id));
		}
		assert!(AuctionManagerModule::collateral_auctions(1).is_none());
		assert!(AuctionManagerModule::collateral_auctions(0).is_none());
		assert!(pool_state.write().transactions.pop().is_none());
	});
}

#[test]
fn offchain_default_max_iterator_works() {
	let (mut offchain, _offchain_state) = testing::TestOffchainExt::new();
	let (pool, pool_state) = testing::TestTransactionPoolExt::new();
	let mut ext = ExtBuilder::lots_of_accounts().build();
	ext.register_extension(OffchainWorkerExt::new(offchain.clone()));
	ext.register_extension(TransactionPoolExt::new(pool));
	ext.register_extension(OffchainDbExt::new(offchain.clone()));

	ext.execute_with(|| {
		System::set_block_number(1);
		// checks that max iterations is stored as none
		assert!(offchain
			.local_storage_get(StorageKind::PERSISTENT, OFFCHAIN_WORKER_MAX_ITERATIONS)
			.is_none());

		for i in 0..1001 {
			let account_id: AccountId = i;
			assert_ok!(AuctionManagerModule::new_collateral_auction(&account_id, BTC, 1, 10));
		}

		mock_shutdown();
		run_to_block_offchain(2);
		// should only run 1000 iterations stopping due to DEFAULT_MAX_ITERATION
		assert_eq!(pool_state.write().transactions.len(), 1000);
		run_to_block_offchain(3);
		// next block iterator starts where it left off and adds the final account to tx pool
		assert_eq!(pool_state.write().transactions.len(), 1001);
	});
}
