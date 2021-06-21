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

//! Unit tests for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use sp_runtime::traits::One;

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
		let collateral_auction_with_positive_target = AuctionManagerModule::collateral_auctions(0).unwrap();
		assert_eq!(collateral_auction_with_positive_target.always_forward(), false);
		assert_eq!(collateral_auction_with_positive_target.in_reverse_stage(99), false);
		assert_eq!(collateral_auction_with_positive_target.in_reverse_stage(100), true);
		assert_eq!(collateral_auction_with_positive_target.in_reverse_stage(101), true);
		assert_eq!(collateral_auction_with_positive_target.payment_amount(99), 99);
		assert_eq!(collateral_auction_with_positive_target.payment_amount(100), 100);
		assert_eq!(collateral_auction_with_positive_target.payment_amount(101), 100);
		assert_eq!(collateral_auction_with_positive_target.collateral_amount(80, 100), 10);
		assert_eq!(collateral_auction_with_positive_target.collateral_amount(100, 200), 5);

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 0));
		let collateral_auction_with_zero_target = AuctionManagerModule::collateral_auctions(1).unwrap();
		assert_eq!(collateral_auction_with_zero_target.always_forward(), true);
		assert_eq!(collateral_auction_with_zero_target.in_reverse_stage(0), false);
		assert_eq!(collateral_auction_with_zero_target.in_reverse_stage(100), false);
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
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::NewCollateralAuction(
			0, BTC, 10, 100,
		)));

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
		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(1, 0, (BOB, 5), None).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 5);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 995);

		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 + 1);
		let carol_ref_count_0 = System::consumers(&CAROL);

		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(2, 0, (CAROL, 10), Some((BOB, 5))).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 10);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 990);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).unwrap().amount, 10);

		let bob_ref_count_2 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_2, bob_ref_count_1 - 1);
		let carol_ref_count_1 = System::consumers(&CAROL);
		assert_eq!(carol_ref_count_1, carol_ref_count_0 + 1);

		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(3, 0, (BOB, 200), Some((CAROL, 10))).is_ok(),
			true
		);
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
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 10), Some((BOB, 5))).accept_bid,
			false,
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 15), Some((BOB, 5))).auction_end_change,
			Change::NewValue(Some(2051))
		);
	});
}

#[test]
fn collateral_auction_end_handler_without_bid() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		let alice_ref_count_0 = System::consumers(&ALICE);

		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
		AuctionManagerModule::on_auction_ended(0, None);
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::CancelAuction(0)));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::collateral_auctions(0), None);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
	});
}

#[test]
fn collateral_auction_end_handler_in_reverse_stage() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(2, 0, (BOB, 400), None).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 50);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 50);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1050);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);

		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 400)));
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::CollateralAuctionDealt(
			0, BTC, 50, BOB, 200,
		)));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::collateral_auctions(0), None);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1050);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1050);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn collateral_auction_end_handler_by_dealing_which_target_not_zero() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(1, 0, (BOB, 100), None).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);

		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 100)));
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::CollateralAuctionDealt(
			0, BTC, 100, BOB, 100,
		)));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::collateral_auctions(0), None);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1100);

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}

#[test]
fn collateral_auction_end_handler_by_dex_which_target_not_zero() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 100));
		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200));
		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(1, 0, (BOB, 20), None).is_ok(),
			true
		);
		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(CAROL),
			BTC,
			AUSD,
			100,
			1000,
			0,
			false
		));
		assert_eq!(DEXModule::get_swap_target_amount(&[BTC, AUSD], 100, None).unwrap(), 500);

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 980);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 20);

		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), true);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 20)));
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::DEXTakeCollateralAuction(
			0, BTC, 100, 500,
		)));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::collateral_auctions(0), None);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 1300);
		assert_eq!(CDPTreasuryModule::debit_pool(), 320);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 520);

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
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

		assert_ok!(AuctionModule::bid(Origin::signed(ALICE), 0, 100));
		let collateral_auction = AuctionManagerModule::collateral_auctions(0).unwrap();
		assert_eq!(collateral_auction.always_forward(), false);
		assert_eq!(AuctionManagerModule::get_last_bid(0), Some((ALICE, 100)));
		assert_eq!(collateral_auction.in_reverse_stage(100), true);
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
		assert_ok!(AuctionModule::bid(Origin::signed(BOB), 0, 80));
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 920);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 920);

		let alice_ref_count_0 = System::consumers(&ALICE);
		let bob_ref_count_0 = System::consumers(&BOB);

		mock_shutdown();
		assert_ok!(AuctionManagerModule::cancel(Origin::none(), 0));
		System::assert_last_event(Event::AuctionManagerModule(crate::Event::CancelAuction(0)));

		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::debit_pool(), 80);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), false);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);

		let alice_ref_count_1 = System::consumers(&ALICE);
		assert_eq!(alice_ref_count_1, alice_ref_count_0 - 1);
		let bob_ref_count_1 = System::consumers(&BOB);
		assert_eq!(bob_ref_count_1, bob_ref_count_0 - 1);
	});
}
