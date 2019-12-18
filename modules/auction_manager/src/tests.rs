//! Unit tests for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::assert_ok;
use mock::{Auction, AuctionManagerModule, CdpTreasury, ExtBuilder, Origin, Tokens, ALICE, AUSD, BOB, BTC};

#[test]
fn set_maximum_auction_size_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::set_maximum_auction_size(Origin::ROOT, BTC, 20));
		assert_eq!(AuctionManagerModule::maximum_auction_size(BTC), 20);
	});
}

#[test]
fn new_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(ALICE, BTC, 10, 100, 90);
		assert_eq!(CdpTreasury::debit_pool(), 90);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(Auction::auctions_count(), 1);
	});
}

#[test]
fn on_new_bid_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(ALICE, BTC, 10, 100, 90);
		assert_eq!(CdpTreasury::debit_pool(), 90);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(CdpTreasury::surplus_pool(), 0);
		assert_eq!(
			AuctionManagerModule::on_new_bid(10, 0, (BOB, 4), None).accept_bid,
			false
		);
		assert_eq!(AuctionManagerModule::on_new_bid(10, 0, (BOB, 5), None).accept_bid, true);
		assert_eq!(CdpTreasury::surplus_pool(), 5);
	});
}

#[test]
fn bid_when_soft_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(ALICE, BTC, 10, 100, 90);
		assert_eq!(
			AuctionManagerModule::on_new_bid(10, 0, (BOB, 5), None).auction_end,
			Some(Some(110))
		);
		assert_eq!(CdpTreasury::surplus_pool(), 5);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2111, 0, (BOB, 10), Some((BOB, 5))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2111, 0, (BOB, 15), Some((BOB, 5))).auction_end,
			Some(Some(2161))
		);
	});
}

#[test]
fn reverse_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(ALICE, BTC, 100, 200, 90);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::balance(BTC, &ALICE), 1000);
		assert_eq!(Tokens::balance(AUSD, &BOB), 1000);
		assert_eq!(CdpTreasury::surplus_pool(), 0);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 200), None).accept_bid,
			true
		);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::balance(BTC, &ALICE), 1000);
		assert_eq!(Tokens::balance(AUSD, &BOB), 800);
		assert_eq!(CdpTreasury::surplus_pool(), 200);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (BOB, 400), Some((BOB, 200))).accept_bid,
			true
		);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 50);
		assert_eq!(Tokens::balance(BTC, &ALICE), 1050);
		assert_eq!(Tokens::balance(AUSD, &BOB), 800);
		assert_eq!(CdpTreasury::surplus_pool(), 200);
	});
}

#[test]
fn on_auction_ended_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(ALICE, BTC, 100, 200, 90);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::balance(AUSD, &BOB), 1000);
		assert_eq!(CdpTreasury::surplus_pool(), 0);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 200), None).accept_bid,
			true
		);
		assert_eq!(Tokens::balance(AUSD, &BOB), 800);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 200)));
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(Tokens::balance(BTC, &BOB), 1100);
	});
}
