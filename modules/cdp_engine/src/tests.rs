//! Unit tests for the cdp engine module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	CdpEngineModule, CdpTreasury, Currencies, DefaultDebitExchangeRate, DefaultLiquidationPenalty,
	DefaultLiquidationRatio, ExtBuilder, Extrinsic, LoansModule, Origin, Runtime, System, TestEvent, ACA, ALICE, AUSD,
	BOB, BTC, DOT,
};
use primitives::offchain::{
	testing::{TestOffchainExt, TestTransactionPoolExt},
	OffchainExt, TransactionPoolExt,
};
use sp_runtime::traits::{BadOrigin, OnFinalize};

#[test]
fn offchain_worker_lock_work() {
	let mut ext = ExtBuilder::default().build();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, _state) = TestTransactionPoolExt::new();
	ext.register_extension(OffchainExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));

	ext.execute_with(|| {
		let storage_key = DB_PREFIX.to_vec();
		let storage = StorageValueRef::persistent(&storage_key);

		// manipulate to set offchain worker lock initially
		// because offchain::random_seed() is still not implemented for TestOffchainExt
		storage.set(&OffchainWorkerLock {
			previous_position: 0,
			expire_timestamp: Timestamp::from_unix_millis(0),
		});
		assert_eq!(CdpEngineModule::acquire_offchain_worker_lock().is_ok(), true);
		assert_eq!(CdpEngineModule::acquire_offchain_worker_lock().is_ok(), false);
		CdpEngineModule::release_offchain_worker_lock(1);
		assert_eq!(CdpEngineModule::acquire_offchain_worker_lock().is_ok(), true);
	});
}

#[test]
fn liquidate_specific_collateral_work() {
	let mut ext = ExtBuilder::default().build();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, state) = TestTransactionPoolExt::new();
	ext.register_extension(OffchainExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));

	ext.execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_ok!(CdpEngineModule::update_position(&BOB, BTC, 100, 50));
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_eq!(CdpEngineModule::is_unsafe_cdp(BTC, &ALICE), true);
		assert_eq!(CdpEngineModule::is_unsafe_cdp(BTC, &BOB), true);
		CdpEngineModule::liquidate_specific_collateral(BTC);
		assert_eq!(state.read().transactions.len(), 2);
		let tx = state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature, None);
	});
}

#[test]
fn settle_specific_collateral_work() {
	let mut ext = ExtBuilder::default().build();
	let (offchain, _state) = TestOffchainExt::new();
	let (pool, state) = TestTransactionPoolExt::new();
	ext.register_extension(OffchainExt::new(offchain));
	ext.register_extension(TransactionPoolExt::new(pool));

	ext.execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_ok!(CdpEngineModule::update_position(&BOB, BTC, 100, 0));
		CdpEngineModule::emergency_shutdown();
		CdpEngineModule::settle_specific_collateral(BTC);
		assert_eq!(state.read().transactions.len(), 1);
		let tx = state.write().transactions.pop().unwrap();
		let tx = Extrinsic::decode(&mut &*tx).unwrap();
		assert_eq!(tx.signature, None);
		assert_eq!(
			tx.call,
			crate::mock::Call::CdpEngineModule(crate::Call::settle(BTC, ALICE))
		);
	});
}

#[test]
fn is_unsafe_cdp_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CdpEngineModule::is_unsafe_cdp(BTC, &ALICE), false);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_eq!(CdpEngineModule::is_unsafe_cdp(BTC, &ALICE), false);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_eq!(CdpEngineModule::is_unsafe_cdp(BTC, &ALICE), true);
	});
}

#[test]
fn get_debit_exchange_rate_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CdpEngineModule::get_debit_exchange_rate(BTC),
			DefaultDebitExchangeRate::get()
		);
	});
}

#[test]
fn get_liquidation_penalty_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CdpEngineModule::get_liquidation_penalty(BTC),
			DefaultLiquidationPenalty::get()
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(5, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CdpEngineModule::get_liquidation_penalty(BTC),
			Rate::from_rational(2, 10)
		);
	});
}

#[test]
fn get_liquidation_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(
			CdpEngineModule::get_liquidation_ratio(BTC),
			DefaultLiquidationRatio::get()
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(5, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CdpEngineModule::get_liquidation_ratio(BTC), Ratio::from_rational(5, 2));
	});
}

#[test]
fn set_collateral_params_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CdpEngineModule::set_collateral_params(
				Origin::signed(5),
				BTC,
				Some(Some(Rate::from_rational(1, 100000))),
				Some(Some(Ratio::from_rational(3, 2))),
				Some(Some(Rate::from_rational(2, 10))),
				Some(Some(Ratio::from_rational(9, 5))),
				Some(10000),
			),
			BadOrigin
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::signed(1),
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));

		let update_stability_fee_event =
			TestEvent::cdp_engine(RawEvent::UpdateStabilityFee(BTC, Some(Rate::from_rational(1, 100000))));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_stability_fee_event));
		let update_liquidation_ratio_event =
			TestEvent::cdp_engine(RawEvent::UpdateLiquidationRatio(BTC, Some(Ratio::from_rational(3, 2))));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_ratio_event));
		let update_liquidation_penalty_event = TestEvent::cdp_engine(RawEvent::UpdateLiquidationPenalty(
			BTC,
			Some(Rate::from_rational(2, 10)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_liquidation_penalty_event));
		let update_required_collateral_ratio_event = TestEvent::cdp_engine(RawEvent::UpdateRequiredCollateralRatio(
			BTC,
			Some(Ratio::from_rational(9, 5)),
		));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_required_collateral_ratio_event));
		let update_maximum_total_debit_value_event =
			TestEvent::cdp_engine(RawEvent::UpdateMaximumTotalDebitValue(BTC, 10000));
		assert!(System::events()
			.iter()
			.any(|record| record.event == update_maximum_total_debit_value_event));

		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CdpEngineModule::stability_fee(BTC),
			Some(Rate::from_rational(1, 100000))
		);
		assert_eq!(
			CdpEngineModule::liquidation_ratio(BTC),
			Some(Ratio::from_rational(3, 2))
		);
		assert_eq!(
			CdpEngineModule::liquidation_penalty(BTC),
			Some(Rate::from_rational(2, 10))
		);
		assert_eq!(
			CdpEngineModule::required_collateral_ratio(BTC),
			Some(Ratio::from_rational(9, 5))
		);
		assert_eq!(CdpEngineModule::maximum_total_debit_value(BTC), 10000);
	});
}

#[test]
fn calculate_collateral_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(
			CdpEngineModule::calculate_collateral_ratio(BTC, 100, 50, Price::from_rational(1, 1)),
			Ratio::from_rational(100, 50)
		);
	});
}

#[test]
fn exceed_debit_value_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 9999), false);
		assert_eq!(CdpEngineModule::exceed_debit_value_cap(BTC, 10001), true);
	});
}

#[test]
fn check_position_adjustment_ratio_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::check_position_adjustment(&ALICE, BTC, 100, 50));
	});
}

#[test]
fn check_position_adjustment_ratio_when_invalid_feedprice() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			DOT,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::check_position_adjustment(&ALICE, DOT, 100, 50),
			Error::<Runtime>::InvalidFeedPrice,
		);
	});
}

#[test]
fn check_position_adjustment_ratio_below_required_ratio() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::check_position_adjustment(&ALICE, BTC, 89, 50),
			Error::<Runtime>::BelowRequiredCollateralRatio
		);
	});
}

#[test]
fn check_debit_cap_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::check_debit_cap(BTC, 9999));
	});
}

#[test]
fn check_debit_cap_exceed() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::check_debit_cap(BTC, 10001),
			Error::<Runtime>::ExceedDebitValueHardCap,
		);
	});
}

#[test]
fn update_position_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::update_position(&ALICE, ACA, 100, 50),
			Error::<Runtime>::NotValidCurrencyId,
		);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 1000);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 0);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 50);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CdpEngineModule::update_position(&ALICE, BTC, 0, 20),
			Error::<Runtime>::UpdatePositionFailed,
		);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 0, -20));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 30);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
	});
}

#[test]
fn remain_debit_value_too_small_check() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_noop!(
			CdpEngineModule::update_position(&ALICE, BTC, 0, -49),
			Error::<Runtime>::UpdatePositionFailed,
		);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, -100, -50));
	});
}

#[test]
fn liquidate_unsafe_cdp_by_collateral_auction() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 50));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 50);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::<Runtime>::CollateralRatioStillSafe,
		);
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			None,
			Some(Some(Ratio::from_rational(3, 1))),
			None,
			None,
			None
		));
		assert_ok!(CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC));

		let liquidate_unsafe_cdp_event = TestEvent::cdp_engine(RawEvent::LiquidateUnsafeCdp(BTC, ALICE, 100, 50));
		assert!(System::events()
			.iter()
			.any(|record| record.event == liquidate_unsafe_cdp_event));

		assert_eq!(CdpTreasury::debit_pool(), 50);
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 50);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 0);
	});
}

#[test]
fn liquidate_unsafe_cdp_when_invalid_feedprice() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			CdpEngineModule::liquidate_unsafe_cdp(ALICE, DOT),
			Error::<Runtime>::InvalidFeedPrice,
		);
	});
}

#[test]
fn liquidate_unsafe_cdp_when_no_debit() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_noop!(
			CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::<Runtime>::NoDebitInCdp,
		);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 0));
		assert_noop!(
			CdpEngineModule::liquidate_unsafe_cdp(ALICE, BTC),
			Error::<Runtime>::NoDebitInCdp,
		);
	});
}

#[test]
fn on_finalize_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			DOT,
			Some(Some(Rate::from_rational(2, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		CdpEngineModule::on_finalize(1);
		assert_eq!(CdpEngineModule::debit_exchange_rate(BTC), None);
		assert_eq!(CdpEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 30);
		CdpEngineModule::on_finalize(2);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
		assert_eq!(CdpEngineModule::debit_exchange_rate(DOT), None);
		CdpEngineModule::on_finalize(3);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(10201, 10000))
		);
		assert_eq!(CdpEngineModule::debit_exchange_rate(DOT), None);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 0, -30));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), 0);
		CdpEngineModule::on_finalize(4);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(10201, 10000))
		);
		assert_eq!(CdpEngineModule::debit_exchange_rate(DOT), None);
	});
}

#[test]
fn emergency_shutdown_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 30));
		CdpEngineModule::on_finalize(1);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
		assert_eq!(CdpEngineModule::is_shutdown(), false);
		CdpEngineModule::emergency_shutdown();
		assert_eq!(CdpEngineModule::is_shutdown(), true);
		CdpEngineModule::on_finalize(2);
		assert_eq!(
			CdpEngineModule::debit_exchange_rate(BTC),
			Some(ExchangeRate::from_rational(101, 100))
		);
	});
}

#[test]
fn settle_cdp_has_debit_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(CdpEngineModule::set_collateral_params(
			Origin::ROOT,
			BTC,
			Some(Some(Rate::from_rational(1, 100000))),
			Some(Some(Ratio::from_rational(3, 2))),
			Some(Some(Rate::from_rational(2, 10))),
			Some(Some(Ratio::from_rational(9, 5))),
			Some(10000),
		));
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 100, 0));
		assert_eq!(Currencies::free_balance(BTC, &ALICE), 900);
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 0);
		assert_eq!(LoansModule::collaterals(ALICE, BTC), 100);
		assert_noop!(
			CdpEngineModule::settle_cdp_has_debit(ALICE, BTC),
			Error::<Runtime>::AlreadyNoDebit,
		);
		assert_ok!(CdpEngineModule::update_position(&ALICE, BTC, 0, 50));
		assert_eq!(LoansModule::debits(BTC, ALICE).0, 50);
		assert_eq!(CdpTreasury::debit_pool(), 0);
		assert_eq!(CdpTreasury::total_collaterals(BTC), 0);
		assert_ok!(CdpEngineModule::settle_cdp_has_debit(ALICE, BTC));

		let settle_cdp_in_debit_event = TestEvent::cdp_engine(RawEvent::SettleCdpInDebit(BTC, ALICE));
		assert!(System::events()
			.iter()
			.any(|record| record.event == settle_cdp_in_debit_event));

		assert_eq!(LoansModule::debits(BTC, ALICE).0, 0);
		assert_eq!(CdpTreasury::debit_pool(), 50);
		assert_eq!(CdpTreasury::total_collaterals(BTC), 50);
	});
}
