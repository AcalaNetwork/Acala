//! Unit tests for the cdp treasury module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::OnFinalize};
use mock::{
	CDPTreasuryModule, Currencies, DEXModule, ExtBuilder, Origin, Runtime, System, TestEvent, ALICE, AUSD, BOB, BTC,
	TOTAL_COLLATERAL_AUCTION, TOTAL_DEBIT_AUCTION, TOTAL_SURPLUS_AUCTION,
};
use sp_runtime::traits::BadOrigin;

#[test]
fn set_collateral_auction_maximum_size_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_eq!(CDPTreasuryModule::collateral_auction_maximum_size(BTC), 0);
		assert_noop!(
			CDPTreasuryModule::set_collateral_auction_maximum_size(Origin::signed(5), BTC, 200),
			BadOrigin
		);
		assert_ok!(CDPTreasuryModule::set_collateral_auction_maximum_size(
			Origin::signed(1),
			BTC,
			200
		));

		let update_collateral_auction_maximum_size_event =
			TestEvent::cdp_treasury(Event::CollateralAuctionMaximumSizeUpdated(BTC, 200));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_collateral_auction_maximum_size_event));

		assert_ok!(CDPTreasuryModule::set_collateral_auction_maximum_size(
			Origin::ROOT,
			BTC,
			200
		));
		assert_eq!(CDPTreasuryModule::collateral_auction_maximum_size(BTC), 200);
	});
}

#[test]
fn set_debit_and_surplus_handle_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		System::set_block_number(1);
		assert_noop!(
			CDPTreasuryModule::set_debit_and_surplus_handle_params(
				Origin::signed(5),
				Some(100),
				Some(1000),
				Some(200),
				Some(100),
			),
			BadOrigin
		);
		assert_ok!(CDPTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::signed(1),
			Some(100),
			Some(1000),
			Some(200),
			Some(100),
		));

		let update_surplus_auction_fixed_size_event =
			TestEvent::cdp_treasury(Event::SurplusAuctionFixedSizeUpdated(100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_surplus_auction_fixed_size_event));
		let update_surplus_buffer_size_event = TestEvent::cdp_treasury(Event::SurplusBufferSizeUpdated(1000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_surplus_buffer_size_event));
		let update_initial_amount_per_debit_auction_event =
			TestEvent::cdp_treasury(Event::InitialAmountPerDebitAuctionUpdated(200));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_initial_amount_per_debit_auction_event));
		let update_debit_auction_fixed_size_event = TestEvent::cdp_treasury(Event::DebitAuctionFixedSizeUpdated(100));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_debit_auction_fixed_size_event));

		assert_ok!(CDPTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(100),
			Some(1000),
			Some(200),
			Some(100),
		));
		assert_eq!(CDPTreasuryModule::surplus_auction_fixed_size(), 100);
		assert_eq!(CDPTreasuryModule::surplus_buffer_size(), 1000);
		assert_eq!(CDPTreasuryModule::initial_amount_per_debit_auction(), 200);
		assert_eq!(CDPTreasuryModule::debit_auction_fixed_size(), 100);
	});
}

#[test]
fn on_system_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(1000));
		assert_eq!(CDPTreasuryModule::debit_pool(), 1000);
	});
}

#[test]
fn on_system_surplus_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
	});
}

#[test]
fn offset_debit_and_surplus_on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 1000);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(300));
		assert_eq!(CDPTreasuryModule::debit_pool(), 300);
		CDPTreasuryModule::on_finalize(2);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 700);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 700);
		assert_eq!(CDPTreasuryModule::debit_pool(), 0);
		assert_ok!(CDPTreasuryModule::on_system_debit(800));
		assert_eq!(CDPTreasuryModule::debit_pool(), 800);
		CDPTreasuryModule::on_finalize(3);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_eq!(CDPTreasuryModule::debit_pool(), 100);
	});
}

#[test]
fn deposit_backed_debit_to_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 1000);
		assert_ok!(CDPTreasuryModule::deposit_backed_debit_to(&ALICE, 1000));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 2000);
	});
}

#[test]
fn withdraw_backed_debit_from_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 1000);
		assert_ok!(CDPTreasuryModule::withdraw_backed_debit_from(&ALICE, 1000));
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_noop!(
			CDPTreasuryModule::withdraw_backed_debit_from(&ALICE, 1000),
			orml_tokens::Error::<Runtime>::BalanceTooLow,
		);
	});
}

#[test]
fn emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::is_shutdown(), false);
		CDPTreasuryModule::emergency_shutdown();
		assert_eq!(CDPTreasuryModule::is_shutdown(), true);
	});
}

#[test]
fn transfer_collateral_from_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 0);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(
			CDPTreasuryModule::transfer_collateral_from(BTC, &ALICE, 10000).is_ok(),
			false
		);
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &ALICE, 500));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 500);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 500);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 500);
	});
}

#[test]
fn transfer_collateral_to_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &ALICE, 500));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 500);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 500);
		assert_eq!(Currencies::free_balance(BTC, &BOB), 1000);
		assert_noop!(
			CDPTreasuryModule::transfer_collateral_to(BTC, &BOB, 501),
			Error::<Runtime>::CollateralNotEnough,
		);
		assert_ok!(CDPTreasuryModule::transfer_collateral_to(BTC, &BOB, 400));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_eq!(Currencies::free_balance(BTC, &CDPTreasuryModule::account_id()), 100);
		assert_eq!(Currencies::free_balance(BTC, &BOB), 1400);
	});
}

#[test]
fn get_total_collaterals_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &ALICE, 500));
		assert_eq!(CDPTreasuryModule::get_total_collaterals(BTC), 500);
	});
}

#[test]
fn get_surplus_pool_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CDPTreasuryModule::on_system_surplus(1000));
		assert_eq!(CDPTreasuryModule::get_surplus_pool(), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &CDPTreasuryModule::account_id()), 1000);
	});
}

#[test]
fn get_debit_proportion_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CDPTreasuryModule::get_debit_proportion(100),
			Ratio::saturating_from_rational(100, Currencies::total_issuance(AUSD))
		);
	});
}

#[test]
fn swap_collateral_to_stable_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(DEXModule::add_liquidity(Origin::signed(ALICE), BTC, 100, 1000));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 0);
		assert_ok!(CDPTreasuryModule::transfer_collateral_from(BTC, &BOB, 100));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 100);
		assert_ok!(CDPTreasuryModule::swap_collateral_to_stable(BTC, 100, 500));
		assert_eq!(CDPTreasuryModule::total_collaterals(BTC), 0);
		assert_eq!(CDPTreasuryModule::surplus_pool(), 500);
	});
}

#[test]
fn create_collateral_auctions_work() {
	ExtBuilder::default().build().execute_with(|| {
		TotalCollaterals::mutate(BTC, |balance| *balance += 10000);
		assert_eq!(CDPTreasuryModule::collateral_auction_maximum_size(BTC), 0);

		// without collateral auction maximum size
		CDPTreasuryModule::create_collateral_auctions(BTC, 1000, 1000, ALICE);
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 1);

		// set collateral auction maximum size
		assert_ok!(CDPTreasuryModule::set_collateral_auction_maximum_size(
			Origin::signed(1),
			BTC,
			300
		));

		// not exceed lots cap
		CDPTreasuryModule::create_collateral_auctions(BTC, 1000, 1000, ALICE);
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 5);

		// exceed lots cap
		CDPTreasuryModule::create_collateral_auctions(BTC, 2000, 1000, ALICE);
		assert_eq!(TOTAL_COLLATERAL_AUCTION.with(|v| *v.borrow_mut()), 11);
	});
}

#[test]
fn create_surplus_auction_when_on_finalize() {
	ExtBuilder::default().build().execute_with(|| {
		SurplusPool::put(1000);
		assert_ok!(CDPTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			Some(300),
			None,
			None,
			None,
		));

		// not exceed lots cap
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(TOTAL_SURPLUS_AUCTION.with(|v| *v.borrow_mut()), 3);

		// exceed lots cap
		SurplusPool::put(2000);
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(TOTAL_SURPLUS_AUCTION.with(|v| *v.borrow_mut()), 8);
	});
}

#[test]
fn create_debit_auction_when_on_finalize() {
	ExtBuilder::default().build().execute_with(|| {
		DebitPool::put(1000);
		assert_ok!(CDPTreasuryModule::set_debit_and_surplus_handle_params(
			Origin::ROOT,
			None,
			None,
			Some(100),
			Some(300),
		));

		// not exceed lots cap
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(TOTAL_DEBIT_AUCTION.with(|v| *v.borrow_mut()), 3);

		// exceed lots cap
		DebitPool::put(2000);
		CDPTreasuryModule::on_finalize(1);
		assert_eq!(TOTAL_DEBIT_AUCTION.with(|v| *v.borrow_mut()), 8);
	});
}
