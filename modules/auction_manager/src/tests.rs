//! Unit tests for the auction manager module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::*;

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
fn debit_auction_methods() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::new_debit_auction(200, 100));
		let debit_auction = AuctionManagerModule::debit_auctions(0).unwrap();
		assert_eq!(debit_auction.amount_for_sale(0, 100), 200);
		assert_eq!(debit_auction.amount_for_sale(100, 200), 100);
		assert_eq!(debit_auction.amount_for_sale(200, 1000), 40);
	});
}

#[test]
fn new_collateral_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(System::refs(&ALICE), 0);
		assert_noop!(
			AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 0, 100),
			Error::<Runtime>::InvalidAmount,
		);

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		let new_collateral_auction_event = TestEvent::auction_manager(RawEvent::NewCollateralAuction(0, BTC, 10, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_collateral_auction_event));

		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 10);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);
		assert_eq!(System::refs(&ALICE), 1);

		assert_noop!(
			AuctionManagerModule::new_collateral_auction(&ALICE, BTC, Balance::max_value(), Balance::max_value()),
			Error::<Runtime>::InvalidAmount,
		);
	});
}

#[test]
fn new_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			AuctionManagerModule::new_debit_auction(0, 100),
			Error::<Runtime>::InvalidAmount,
		);
		assert_noop!(
			AuctionManagerModule::new_debit_auction(200, 0),
			Error::<Runtime>::InvalidAmount,
		);

		assert_ok!(AuctionManagerModule::new_debit_auction(200, 100));
		let new_debit_auction_event = TestEvent::auction_manager(RawEvent::NewDebitAuction(0, 200, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_debit_auction_event));

		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);

		assert_noop!(
			AuctionManagerModule::new_debit_auction(200, Balance::max_value()),
			Error::<Runtime>::InvalidAmount,
		);
	});
}

#[test]
fn new_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			AuctionManagerModule::new_surplus_auction(0),
			Error::<Runtime>::InvalidAmount,
		);

		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		let new_surplus_auction_event = TestEvent::auction_manager(RawEvent::NewSurplusAuction(0, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == new_surplus_auction_event));

		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(AuctionModule::auctions_index(), 1);

		assert_noop!(
			AuctionManagerModule::new_surplus_auction(Balance::max_value()),
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
		assert_eq!(System::refs(&BOB), 0);

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
		assert_eq!(System::refs(&BOB), 1);
		assert_eq!(System::refs(&CAROL), 0);

		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(2, 0, (CAROL, 10), Some((BOB, 5))).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 10);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 990);
		assert_eq!(System::refs(&BOB), 0);
		assert_eq!(System::refs(&CAROL), 1);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).unwrap().amount, 10);

		assert_eq!(
			AuctionManagerModule::collateral_auction_bid_handler(3, 0, (BOB, 200), Some((CAROL, 10))).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 1000);
		assert_eq!(System::refs(&BOB), 1);
		assert_eq!(System::refs(&CAROL), 0);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).unwrap().amount, 5);
	});
}

#[test]
fn debit_auction_bid_handler_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::debit_auction_bid_handler(1, 0, (BOB, 99), None),
			Error::<Runtime>::AuctionNotExists,
		);

		assert_ok!(AuctionManagerModule::new_debit_auction(200, 100));
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 200);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(System::refs(&BOB), 0);

		assert_noop!(
			AuctionManagerModule::debit_auction_bid_handler(1, 0, (BOB, 99), None),
			Error::<Runtime>::InvalidBidPrice,
		);
		assert_eq!(
			AuctionManagerModule::debit_auction_bid_handler(1, 0, (BOB, 100), None).is_ok(),
			true
		);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 200);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(System::refs(&BOB), 1);
		assert_eq!(System::refs(&CAROL), 0);

		assert_eq!(
			AuctionManagerModule::debit_auction_bid_handler(2, 0, (CAROL, 200), Some((BOB, 100))).is_ok(),
			true
		);
		assert_eq!(AuctionManagerModule::debit_auctions(0).unwrap().amount, 100);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &CAROL), 900);
		assert_eq!(System::refs(&BOB), 0);
		assert_eq!(System::refs(&CAROL), 1);
	});
}

#[test]
fn surplus_auction_bid_handler_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AuctionManagerModule::surplus_auction_bid_handler(1, 0, (BOB, 99), None),
			Error::<Runtime>::AuctionNotExists,
		);

		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(System::refs(&BOB), 0);

		assert_eq!(
			AuctionManagerModule::surplus_auction_bid_handler(1, 0, (BOB, 50), None).is_ok(),
			true
		);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 950);
		assert_eq!(Tokens::free_balance(ACA, &CAROL), 1000);
		assert_eq!(System::refs(&BOB), 1);
		assert_eq!(System::refs(&CAROL), 0);

		assert_noop!(
			AuctionManagerModule::surplus_auction_bid_handler(2, 0, (CAROL, 51), Some((BOB, 50))),
			Error::<Runtime>::InvalidBidPrice,
		);
		assert_eq!(
			AuctionManagerModule::surplus_auction_bid_handler(2, 0, (CAROL, 55), Some((BOB, 50))).is_ok(),
			true
		);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(Tokens::free_balance(ACA, &CAROL), 945);
		assert_eq!(System::refs(&BOB), 0);
		assert_eq!(System::refs(&CAROL), 1);
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
fn bid_when_soft_cap_for_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::new_debit_auction(200, 100));
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end_change,
			Change::NewValue(Some(101))
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 105), Some((BOB, 100))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 110), Some((BOB, 100))).auction_end_change,
			Change::NewValue(Some(2051))
		);
	});
}

#[test]
fn bid_when_soft_cap_for_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		assert_eq!(
			AuctionManagerModule::on_new_bid(1, 0, (BOB, 100), None).auction_end_change,
			Change::NewValue(Some(101))
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 105), Some((BOB, 100))).accept_bid,
			false
		);
		assert_eq!(
			AuctionManagerModule::on_new_bid(2001, 0, (CAROL, 110), Some((BOB, 100))).auction_end_change,
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
		assert_eq!(System::refs(&ALICE), 1);

		AuctionManagerModule::collateral_auction_end_handler(0, None);
		let auction_passed_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == auction_passed_event));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(System::refs(&ALICE), 0);
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
		assert_eq!(System::refs(&ALICE), 1);
		assert_eq!(System::refs(&BOB), 1);

		AuctionManagerModule::collateral_auction_end_handler(0, Some((BOB, 400)));
		let auction_dealt_event = TestEvent::auction_manager(RawEvent::CollateralAuctionDealt(0, BTC, 50, BOB, 200));
		assert!(System::events()
			.iter()
			.any(|record| record.event == auction_dealt_event));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(Tokens::free_balance(BTC, &ALICE), 1050);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1050);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 800);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 200);
		assert_eq!(System::refs(&ALICE), 0);
		assert_eq!(System::refs(&BOB), 0);
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
		assert_eq!(System::refs(&ALICE), 1);
		assert_eq!(System::refs(&BOB), 1);

		AuctionManagerModule::collateral_auction_end_handler(0, Some((BOB, 100)));
		let auction_dealt_event = TestEvent::auction_manager(RawEvent::CollateralAuctionDealt(0, BTC, 100, BOB, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == auction_dealt_event));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1100);
		assert_eq!(System::refs(&BOB), 0);
		assert_eq!(System::refs(&ALICE), 0);
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
		assert_ok!(DEXModule::add_liquidity(Origin::signed(CAROL), BTC, 100, 1000));
		assert_eq!(DEXModule::get_target_amount(BTC, AUSD, 100), 500);

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 200);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 100);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 980);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 20);
		assert_eq!(System::refs(&ALICE), 1);
		assert_eq!(System::refs(&BOB), 1);

		AuctionManagerModule::collateral_auction_end_handler(0, Some((BOB, 20)));
		let dex_take_collateral_auction =
			TestEvent::auction_manager(RawEvent::DEXTakeCollateralAuction(0, BTC, 100, 500));
		assert!(System::events()
			.iter()
			.any(|record| record.event == dex_take_collateral_auction));

		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(Tokens::free_balance(BTC, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(AUSD, &ALICE), 1300);
		assert_eq!(CDPTreasuryModule::debit_pool(), 320);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 520);
		assert_eq!(System::refs(&ALICE), 0);
		assert_eq!(System::refs(&BOB), 0);
	});
}

#[test]
fn debit_auction_end_handler_without_bid() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AuctionManagerModule::new_debit_auction(300, 100));
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);

		AuctionManagerModule::debit_auction_end_handler(0, None);
		let auction_passed_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == auction_passed_event));

		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 0);
	});
}

#[test]
fn debit_auction_end_handler_with_bid() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AuctionManagerModule::new_debit_auction(300, 100));
		assert_eq!(
			AuctionManagerModule::debit_auction_bid_handler(1, 0, (BOB, 100), None).is_ok(),
			true
		);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(System::refs(&BOB), 1);

		AuctionManagerModule::debit_auction_end_handler(0, Some((BOB, 100)));
		let debit_auction_deal_event = TestEvent::auction_manager(RawEvent::DebitAuctionDealt(0, 300, BOB, 100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == debit_auction_deal_event));

		assert_eq!(Tokens::free_balance(ACA, &BOB), 1300);
		assert_eq!(Tokens::total_issuance(ACA), 3300);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 0);
		assert_eq!(System::refs(&BOB), 0);
	});
}

#[test]
fn surplus_auction_end_handler_without_bid() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);

		AuctionManagerModule::surplus_auction_end_handler(0, None);
		let auction_passed_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == auction_passed_event));

		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 0);
	});
}

#[test]
fn surplus_auction_end_handler_with_bid() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(CDPTreasuryModule::on_system_surplus(100));
		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		assert_eq!(
			AuctionManagerModule::surplus_auction_bid_handler(1, 0, (BOB, 500), None).is_ok(),
			true
		);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 500);
		assert_eq!(Tokens::total_issuance(ACA), 2500);
		assert_eq!(System::refs(&BOB), 1);

		AuctionManagerModule::surplus_auction_end_handler(0, Some((BOB, 500)));
		let surplus_auction_deal_event = TestEvent::auction_manager(RawEvent::SurplusAuctionDealt(0, 100, BOB, 500));
		assert!(System::events()
			.iter()
			.any(|record| record.event == surplus_auction_deal_event));

		assert_eq!(CDPTreasuryModule::debit_pool(), 100);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 0);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1100);
		assert_eq!(Tokens::total_issuance(ACA), 2500);
		assert_eq!(System::refs(&BOB), 0);
	});
}

#[test]
fn cancel_surplus_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			AuctionManagerModule::cancel_surplus_auction(0),
			Error::<Runtime>::AuctionNotExists
		);

		assert_ok!(AuctionManagerModule::new_surplus_auction(100));
		assert_ok!(AuctionModule::bid(Origin::signed(BOB), 0, 500));
		assert_eq!(AuctionManagerModule::surplus_auctions(0).is_some(), true);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 100);
		assert_eq!(AuctionModule::auction_info(0).is_some(), true);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 500);
		assert_eq!(System::refs(&BOB), 1);

		mock_shutdown();
		assert_ok!(AuctionManagerModule::cancel(Origin::none(), 0));
		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(AuctionManagerModule::surplus_auctions(0).is_some(), false);
		assert_eq!(AuctionManagerModule::total_surplus_in_auction(), 0);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
		assert_eq!(Tokens::free_balance(ACA, &BOB), 1000);
		assert_eq!(System::refs(&BOB), 0);
	});
}

#[test]
fn cancel_debit_auction_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			AuctionManagerModule::cancel_debit_auction(0),
			Error::<Runtime>::AuctionNotExists
		);
		assert_ok!(AuctionManagerModule::new_debit_auction(200, 100));
		assert_ok!(AuctionModule::bid(Origin::signed(BOB), 0, 100));
		assert_eq!(AuctionManagerModule::debit_auctions(0).is_some(), true);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 100);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 900);
		assert_eq!(System::refs(&BOB), 1);

		mock_shutdown();
		assert_ok!(AuctionManagerModule::cancel(Origin::none(), 0));
		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(AuctionManagerModule::debit_auctions(0).is_some(), false);
		assert_eq!(AuctionManagerModule::total_debit_in_auction(), 0);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(System::refs(&BOB), 0);
	});
}

#[test]
fn cancel_collateral_auction_failed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::deposit_collateral(&CAROL, BTC, 10));
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0),
			Error::<Runtime>::AuctionNotExists
		);

		assert_ok!(AuctionManagerModule::new_collateral_auction(&ALICE, BTC, 10, 100));
		MockPriceSource::set_relative_price(None);
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0),
			Error::<Runtime>::InvalidFeedPrice,
		);
		MockPriceSource::set_relative_price(Some(Price::one()));

		assert_ok!(AuctionModule::bid(Origin::signed(ALICE), 0, 100));
		let collateral_auction = AuctionManagerModule::collateral_auctions(0).unwrap();
		assert_eq!(collateral_auction.always_forward(), false);
		assert_eq!(AuctionManagerModule::get_last_bid(0), Some((ALICE, 100)));
		assert_eq!(collateral_auction.in_reverse_stage(100), true);
		assert_noop!(
			AuctionManagerModule::cancel_collateral_auction(0),
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
		assert_eq!(System::refs(&ALICE), 1);
		assert_eq!(System::refs(&BOB), 1);

		mock_shutdown();
		assert_ok!(AuctionManagerModule::cancel(Origin::none(), 0));
		let cancel_auction_event = TestEvent::auction_manager(RawEvent::CancelAuction(0));
		assert!(System::events()
			.iter()
			.any(|record| record.event == cancel_auction_event));

		assert_eq!(Tokens::free_balance(AUSD, &BOB), 1000);
		assert_eq!(System::refs(&ALICE), 0);
		assert_eq!(System::refs(&BOB), 0);
		assert_eq!(AuctionManagerModule::total_collateral_in_auction(BTC), 0);
		assert_eq!(AuctionManagerModule::total_target_in_auction(), 0);
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 10);
		assert_eq!(CDPTreasuryModule::debit_pool(), 80);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 80);
		assert_eq!(AuctionManagerModule::collateral_auctions(0).is_some(), false);
		assert_eq!(AuctionModule::auction_info(0).is_some(), false);
	});
}
