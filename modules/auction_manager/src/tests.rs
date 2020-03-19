//! Unit tests for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	Auction as AuctionModule, AuctionManagerModule, CDPTreasuryModule, ExtBuilder, Runtime, System, TestEvent, Tokens,
	ACA, ALICE, AUSD, BOB, BTC, CAROL,
};

#[test]
fn new_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);

		let new_collateral_auction_event = TestEvent::auction_manager(RawEvent::NewCollateralAuction(0, BTC, 10, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_collateral_auction_event));

		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);
	});
}

#[test]
fn new_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_debit_auction(200, 100);

		let new_debit_auction_event = TestEvent::auction_manager(RawEvent::NewDebitAuction(0, 200, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_debit_auction_event));

		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);
	});
}

#[test]
fn new_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_surplus_auction(100);

		let new_surplus_auction_event = TestEvent::auction_manager(RawEvent::NewSurplusAuction(0, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_surplus_auction_event));

		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);
	});
}

#[test]
fn on_new_bid_for_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(AuctionManagerModule::on_new_bid(1, 0, (BOB, 4), None).accept_bid, false);
		assert_eq!(AuctionManagerModule::on_new_bid(1, 0, (BOB, 5), None).accept_bid, true);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 995);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 5);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (CAROL, 10), Some((BOB, 5))).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 990);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 10);
	});
}

#[test]
fn on_new_bid_for_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_debit_auction(200, 100);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 200);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 99), None).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).accept_bid,
			true
		);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 200);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (CAROL, 200), Some((BOB, 100))).accept_bid,
			true
		);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 900);
	});
}

#[test]
fn on_new_bid_for_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_surplus_auction(100);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(AuctionManagerModule::on_new_bid(1, 0, (BOB, 0), None).accept_bid, false);
		assert_eq!(AuctionManagerModule::on_new_bid(1, 0, (BOB, 50), None).accept_bid, true);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 950);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (CAROL, 51), Some((BOB, 50))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (CAROL, 55), Some((BOB, 50))).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(Tokens::free_balance(ACA, &CAROL), 945);
	});
}

#[test]
fn bid_when_soft_cap_for_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end,
			Some(Some(101))
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 10), Some((BOB, 5))).accept_bid,
			false,
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 15), Some((BOB, 5))).auction_end,
			Some(Some(2051))
		);
	});
}

#[test]
fn bid_when_soft_cap_for_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_debit_auction(200, 100);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end,
			Some(Some(101))
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 105), Some((BOB, 100))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 110), Some((BOB, 100))).auction_end,
			Some(Some(2051))
		);
	});
}

#[test]
fn bid_when_soft_cap_for_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_surplus_auction(100);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end,
			Some(Some(101))
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 105), Some((BOB, 100))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 110), Some((BOB, 100))).auction_end,
			Some(Some(2051))
		);
	});
}

#[test]
fn reverse_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &CAROL, 100));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 200), None).accept_bid,
			true
		);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2, 0, (BOB, 400), Some((BOB, 200))).accept_bid,
			true
		);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 50);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 50);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1050);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
	});
}

#[test]
fn on_auction_ended_for_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &CAROL, 100));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 100, 200);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 200), None).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 200)));
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1100);
	});
}

#[test]
fn on_auction_ended_for_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		AuctionManagerModule::new_debit_auction(200, 100);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::total_issuance(ACA), 3000);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 200);
		AuctionManagerModule::on_auction_ended(0, None);
		assert_eq!(AuctionManagerModule::debit_auctions(1).unwrap().amount, 300);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 1, (BOB, 100), None).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		AuctionManagerModule::on_auction_ended(1, Some((BOB, 100)));
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1300);
		assert_eq!(Tokens::total_issuance(ACA), 3300);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 0);
	});
}

#[test]
fn on_auction_ended_for_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::on_system_surplus(100));
		AuctionManagerModule::new_surplus_auction(100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(Tokens::total_issuance(ACA), 3000);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 500), None).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 500);
		assert_eq!(Tokens::total_issuance(ACA), 2500);
		AuctionManagerModule::on_auction_ended(0, Some((BOB, 500)));
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1100);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 0);
	});
}

#[test]
fn cancel_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::cancel_surplus_auction(0),
			Error::<Runtime>::AuctionNotExsits
		);
		AuctionManagerModule::new_surplus_auction(100);
		assert_eq!(AuctionManagerModule::surplus_auctions(0).is_some(), true);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(AuctionModule::auction_info(0).is_some(), true);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 500), None).accept_bid,
			true
		);
		assert_ok!(AuctionManagerModule::cancel_surplus_auction(0));

		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(AuctionManagerModule::surplus_auctions(0).is_some(), false);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 0);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
	});
}

#[test]
fn cancel_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::cancel_debit_auction(0),
			Error::<Runtime>::AuctionNotExsits
		);
		AuctionManagerModule::new_debit_auction(200, 100);
		assert_eq!(AuctionManagerModule::debit_auctions(0).is_some(), true);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).accept_bid,
			true
		);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_ok!(AuctionManagerModule::cancel_debit_auction(0));

		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(AuctionManagerModule::debit_auctions(0).is_some(), false);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 0);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
	});
}

#[test]
fn collateral_auction_in_reverse_stage_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(AuctionManagerModule::collateral_auction_in_reverse_stage(0), false);
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);
		assert_eq!(AuctionManagerModule::collateral_auction_in_reverse_stage(0), false);
		assert_ok!(AuctionModule::bid(Some(BOB).into(), 0, 20));
		assert_eq!(AuctionManagerModule::collateral_auction_in_reverse_stage(0), false);
		assert_ok!(AuctionModule::bid(Some(ALICE).into(), 0, 100));
		assert_eq!(AuctionManagerModule::collateral_auction_in_reverse_stage(0), true);
	});
}

#[test]
fn cancel_collateral_auction_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0),
			Error::<Runtime>::AuctionNotExsits
		);
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);
		assert_ok!(AuctionModule::bid(Some(ALICE).into(), 0, 100));
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0),
			Error::<Runtime>::InReservedStage
		);
	});
}

#[test]
fn cancel_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &CAROL, 10));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(AuctionModule::bid(Some(BOB).into(), 0, 80));
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 920);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);

		assert_ok!(AuctionManagerModule::cancel_collateral_auction(0));

		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::debit_pool(), 80);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), false);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
	});
}
