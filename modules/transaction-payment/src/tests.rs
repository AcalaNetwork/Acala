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

//! Unit tests for the transaction payment module.

#![cfg(test)]

use super::*;
use crate::mock::{AlternativeFeeSurplus, AusdFeeSwapPath, CustomFeeSurplus, DotFeeSwapPath};
use frame_support::{
	assert_noop, assert_ok,
	weights::{DispatchClass, DispatchInfo, Pays},
};
use mock::{
	AccountId, BlockWeights, Call, Currencies, DEXModule, ExtBuilder, FeePoolSize, MockPriceSource, Origin, Runtime,
	System, TransactionPayment, ACA, ALICE, AUSD, BOB, CHARLIE, DAVE, DOT, FEE_UNBALANCED_AMOUNT,
	TIP_UNBALANCED_AMOUNT,
};
use orml_traits::{MultiCurrency, MultiLockableCurrency};
use pallet_balances::ReserveData;
use primitives::currency::*;
use sp_io::TestExternalities;
use sp_runtime::{
	testing::TestXt,
	traits::{One, UniqueSaturatedInto},
};
use support::{BuyWeightRate, Price, TransactionPayment as TransactionPaymentT};
use xcm::latest::prelude::*;
use xcm::prelude::GeneralKey;

const CALL: <Runtime as frame_system::Config>::Call = Call::Currencies(module_currencies::Call::transfer {
	dest: BOB,
	currency_id: AUSD,
	amount: 100,
});

const CALL2: <Runtime as frame_system::Config>::Call =
	Call::Currencies(module_currencies::Call::transfer_native_currency { dest: BOB, amount: 12 });

const INFO: DispatchInfo = DispatchInfo {
	weight: 1000,
	class: DispatchClass::Normal,
	pays_fee: Pays::Yes,
};

const INFO2: DispatchInfo = DispatchInfo {
	weight: 100,
	class: DispatchClass::Normal,
	pays_fee: Pays::Yes,
};

const POST_INFO: PostDispatchInfo = PostDispatchInfo {
	actual_weight: Some(800),
	pays_fee: Pays::Yes,
};

fn with_fee_path_call(fee_swap_path: Vec<CurrencyId>) -> <Runtime as Config>::Call {
	let fee_call: <Runtime as Config>::Call =
		Call::TransactionPayment(crate::mock::transaction_payment::Call::with_fee_path {
			fee_swap_path,
			call: Box::new(CALL),
		});
	fee_call
}

fn with_fee_currency_call(currency_id: CurrencyId) -> <Runtime as Config>::Call {
	let fee_call: <Runtime as Config>::Call =
		Call::TransactionPayment(crate::mock::transaction_payment::Call::with_fee_currency {
			currency_id,
			call: Box::new(CALL),
		});
	fee_call
}

fn with_fee_paid_by_call(payer_addr: AccountId, payer_sig: MultiSignature) -> <Runtime as Config>::Call {
	let fee_call: <Runtime as Config>::Call =
		Call::TransactionPayment(crate::mock::transaction_payment::Call::with_fee_paid_by {
			call: Box::new(CALL),
			payer_addr,
			payer_sig,
		});
	fee_call
}

fn enable_dex_and_tx_fee_pool() {
	let treasury_account: AccountId = <Runtime as Config>::TreasuryAccount::get();
	let init_balance = FeePoolSize::get();
	assert_ok!(Currencies::update_balance(
		Origin::root(),
		treasury_account.clone(),
		ACA,
		(init_balance * 100).unique_saturated_into(),
	));
	vec![AUSD, DOT].iter().for_each(|token| {
		let ed = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			treasury_account.clone(),
			token.clone(),
			ed,
		));
	});

	let alice_balance = Currencies::free_balance(ACA, &ALICE);
	if alice_balance < 100000 {
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			100000.unique_saturated_into(),
		));
	}

	// enable dex
	assert_ok!(DEXModule::add_liquidity(
		Origin::signed(ALICE),
		ACA,
		AUSD,
		10000,
		1000,
		0,
		false
	));
	assert_ok!(DEXModule::add_liquidity(
		Origin::signed(ALICE),
		DOT,
		AUSD,
		100,
		1000,
		0,
		false
	));

	// enable tx fee pool
	assert_ok!(Pallet::<Runtime>::enable_charge_fee_pool(
		Origin::signed(ALICE),
		AUSD,
		AusdFeeSwapPath::get(),
		FeePoolSize::get(),
		crate::mock::LowerSwapThreshold::get()
	));
	assert_ok!(Pallet::<Runtime>::enable_charge_fee_pool(
		Origin::signed(ALICE),
		DOT,
		DotFeeSwapPath::get(),
		FeePoolSize::get(),
		crate::mock::LowerSwapThreshold::get()
	));

	// validate tx fee pool works
	vec![AUSD, DOT].iter().for_each(|token| {
		let ed = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
		let sub_account: AccountId = <Runtime as Config>::PalletId::get().into_sub_account_truncating(token.clone());
		assert_eq!(Currencies::free_balance(token.clone(), &treasury_account), 0);
		assert_eq!(Currencies::free_balance(token.clone(), &sub_account), ed);
		assert_eq!(Currencies::free_balance(ACA, &sub_account), init_balance);
	});

	assert_eq!(GlobalFeeSwapPath::<Runtime>::get(DOT).unwrap(), vec![DOT, AUSD, ACA]);
	assert_eq!(GlobalFeeSwapPath::<Runtime>::get(AUSD).unwrap(), vec![AUSD, ACA]);

	// manual set the exchange rate for simplify calculation
	TokenExchangeRate::<Runtime>::insert(AUSD, Ratio::saturating_from_rational(10, 1));
	let dot_rate = TokenExchangeRate::<Runtime>::get(DOT).unwrap();
	assert_eq!(dot_rate, Ratio::saturating_from_rational(1, 10));
}

fn builder_with_dex_and_fee_pool(enable_pool: bool) -> TestExternalities {
	let mut builder = ExtBuilder::default().one_hundred_thousand_for_alice_n_charlie().build();
	if enable_pool {
		builder.execute_with(|| {
			enable_dex_and_tx_fee_pool();
		});
	}
	builder
}

#[test]
fn charges_fee_when_native_is_enough_but_cannot_keep_alive() {
	ExtBuilder::default().build().execute_with(|| {
		// balance set to fee, after charge fee, balance less than ED, cannot keep alive
		// fee = len(validate method parameter) * byte_fee(constant) + weight(in DispatchInfo)
		let fee = 5000 * 2 + 1000;
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			fee.unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), fee);
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&ALICE, &CALL, &INFO, 5000),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		// after charge fee, balance=fee-fee2=ED, equal to ED, keep alive
		let fee2 = 5000 * 2 + 990;
		let info = DispatchInfo {
			weight: 990,
			class: DispatchClass::Normal,
			pays_fee: Pays::Yes,
		};
		let expect_priority = ChargeTransactionPayment::<Runtime>::get_priority(&info, 5000, fee2, fee2);
		assert_eq!(1000, expect_priority);
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, &CALL, &info, 5000)
				.unwrap()
				.priority,
			1
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), Currencies::minimum_balance(ACA));
	});
}

#[test]
fn charges_fee_when_validate_native_is_enough() {
	// Alice init 100000 ACA(native asset)
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, &CALL, &INFO, 23)
				.unwrap()
				.priority,
			1
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

		let fee2 = 18 * 2 + 1000; // len * byte + weight
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&ALICE, &CALL2, &INFO, 18)
				.unwrap()
				.priority,
			1
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - fee2);
	});
}

#[test]
fn charges_fee_when_locked_transfer_not_enough() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let fee = 12 * 2 + 1000; // len * byte + weight
		assert_ok!(Currencies::update_balance(Origin::root(), BOB, ACA, 2048,));

		// transferable=2048-1025 < fee=1024, native asset is not enough
		assert_ok!(<Currencies as MultiLockableCurrency<AccountId>>::set_lock(
			[0u8; 8], ACA, &BOB, 1025
		));
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL, &INFO, 12),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		// after remove lock, transferable=2048 > fee
		assert_ok!(<Currencies as MultiLockableCurrency<AccountId>>::remove_lock(
			[0u8; 8], ACA, &BOB
		));
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL, &INFO, 12));
		assert_eq!(Currencies::free_balance(ACA, &BOB), 2048 - fee);
	});
}

#[test]
fn pre_post_dispatch_and_refund_native_is_enough() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		let pre = ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

		let actual_fee = TransactionPayment::compute_actual_fee(23, &INFO, &POST_INFO, 0);
		assert_eq!(actual_fee, 23 * 2 + 800);

		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			Some(pre),
			&INFO,
			&POST_INFO,
			23,
			&Ok(())
		));

		let refund = 200; // 1000 - 800
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);
		assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()), fee - refund);
		assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), 0);

		// reset and test refund with tip
		FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow_mut() = 0);

		let tip: Balance = 5;
		let pre = ChargeTransactionPayment::<Runtime>::from(tip)
			.pre_dispatch(&CHARLIE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - tip);
		let actual_fee = TransactionPayment::compute_actual_fee(23, &INFO, &POST_INFO, tip);
		assert_eq!(actual_fee, 23 * 2 + 800 + 5);
		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			Some(pre),
			&INFO,
			&POST_INFO,
			23,
			&Ok(())
		));
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - tip + refund);
		assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()), fee - refund);
		assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), tip);
	});
}

#[test]
fn pre_post_dispatch_and_refund_with_fee_path_call() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		// with_fee_path call will swap user's AUSD out of ACA, then withdraw ACA as fee
		let fee = 500 * 2 + 1000; // len * byte + weight
		let surplus = CustomFeeSurplus::get().mul_ceil(fee);
		let fee_surplus = surplus + fee;

		let aca_init = Currencies::free_balance(ACA, &ALICE);
		let usd_init = Currencies::free_balance(AUSD, &ALICE);
		let pre = ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, &with_fee_path_call(vec![AUSD, ACA]), &INFO, 500)
			.unwrap();
		assert_eq!(pre.2, Some(pallet_balances::NegativeImbalance::new(fee_surplus)));
		assert_eq!(pre.3, fee_surplus);
		System::assert_has_event(crate::mock::Event::DEXModule(module_dex::Event::Swap {
			trader: ALICE,
			path: vec![AUSD, ACA],
			liquidity_changes: vec![429, fee_surplus], // 429 AUSD - 1569 ACA
		}));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), aca_init); // ACA not changed
		assert_eq!(Currencies::free_balance(AUSD, &ALICE), usd_init - 429); // AUSD decrements

		// the actual fee not include fee surplus
		let actual_fee = TransactionPayment::compute_actual_fee(500, &INFO, &POST_INFO, 0);
		assert_eq!(actual_fee, 500 * 2 + 800);

		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			Some(pre),
			&INFO,
			&POST_INFO,
			500,
			&Ok(())
		));

		let refund = 200; // 1000 - 800
		let refund_surplus = 100;
		assert_eq!(
			Currencies::free_balance(ACA, &ALICE),
			aca_init + refund + refund_surplus
		);
		assert_eq!(
			FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()),
			fee - refund + surplus - refund_surplus
		);
		assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), 0);

		// reset and test refund with tip
		FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow_mut() = 0);

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			CHARLIE,
			AUSD,
			8000.unique_saturated_into(),
		));
		let aca_init = Currencies::free_balance(ACA, &CHARLIE);
		let usd_init = Currencies::free_balance(AUSD, &CHARLIE);
		let tip: Balance = 200;
		let surplus = CustomFeeSurplus::get().mul_ceil(fee + tip);
		let fee_surplus = surplus + fee + tip;

		let pre = ChargeTransactionPayment::<Runtime>::from(tip)
			.pre_dispatch(&CHARLIE, &with_fee_path_call(vec![AUSD, ACA]), &INFO, 500)
			.unwrap();
		assert_eq!(pre.2, Some(pallet_balances::NegativeImbalance::new(fee_surplus)));
		assert_eq!(pre.3, fee_surplus);
		System::assert_has_event(crate::mock::Event::DEXModule(module_dex::Event::Swap {
			trader: CHARLIE,
			path: vec![AUSD, ACA],
			liquidity_changes: vec![1275, fee_surplus], // 1275 AUSD - 3300 ACA
		}));
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), aca_init);
		assert_eq!(Currencies::free_balance(AUSD, &CHARLIE), usd_init - 1275);
		let actual_fee = TransactionPayment::compute_actual_fee(500, &INFO, &POST_INFO, tip);
		assert_eq!(actual_fee, 500 * 2 + 800 + 200);
		assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
			Some(pre),
			&INFO,
			&POST_INFO,
			500,
			&Ok(())
		));
		assert_eq!(
			Currencies::free_balance(ACA, &CHARLIE),
			aca_init + refund + refund_surplus
		);
		assert_eq!(
			FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()),
			fee - refund + surplus - refund_surplus
		);
		assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), tip);
	});
}

#[test]
fn charges_fee_when_pre_dispatch_and_native_currency_is_enough() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		assert!(ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, &CALL, &INFO, 23)
			.is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);
	});
}

#[test]
fn refund_fee_according_to_actual_when_post_dispatch_and_native_currency_is_enough() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		let pre = ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

		let refund = 200; // 1000 - 800
		assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(Some(pre), &INFO, &POST_INFO, 23, &Ok(())).is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);
	});
}

#[test]
fn refund_tip_according_to_actual_when_post_dispatch_and_native_currency_is_enough() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		// tip = 0
		let fee = 23 * 2 + 1000; // len * byte + weight
		let pre = ChargeTransactionPayment::<Runtime>::from(0)
			.pre_dispatch(&ALICE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

		let refund = 200; // 1000 - 800
		assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(Some(pre), &INFO, &POST_INFO, 23, &Ok(())).is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);

		// tip = 1000
		let fee = 23 * 2 + 1000; // len * byte + weight
		let tip = 1000;
		let pre = ChargeTransactionPayment::<Runtime>::from(tip)
			.pre_dispatch(&CHARLIE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - tip);

		let refund_fee = 200; // 1000 - 800
		let refund_tip = 200; // 1000 - 800
		assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(Some(pre), &INFO, &POST_INFO, 23, &Ok(())).is_ok());
		assert_eq!(
			Currencies::free_balance(ACA, &CHARLIE),
			100000 - fee - tip + refund_fee + refund_tip
		);
	});
}

#[test]
fn refund_should_not_works() {
	builder_with_dex_and_fee_pool(false).execute_with(|| {
		let tip = 1000;
		let fee = 23 * 2 + 1000; // len * byte + weight
		let pre = ChargeTransactionPayment::<Runtime>::from(tip)
			.pre_dispatch(&ALICE, &CALL, &INFO, 23)
			.unwrap();
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - tip);

		// actual_weight > weight
		const POST_INFO: PostDispatchInfo = PostDispatchInfo {
			actual_weight: Some(INFO.weight + 1),
			pays_fee: Pays::Yes,
		};

		assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(Some(pre), &INFO, &POST_INFO, 23, &Ok(())).is_ok());
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - tip);
	});
}

#[test]
fn charges_fee_when_validate_with_fee_path_call() {
	// Enable dex with Alice, and initialize tx charge fee pool
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let dex_acc: AccountId = PalletId(*b"aca/dexm").into_account_truncating();
		let dex_aca = Currencies::free_balance(ACA, &dex_acc);

		let fee: Balance = 50 * 2 + 100 + 10;
		let fee_surplus = fee + CustomFeeSurplus::get().mul_ceil(fee);
		assert_eq!(315, fee_surplus);

		assert_ok!(Currencies::update_balance(Origin::root(), BOB, AUSD, 10000));

		// AUSD - ACA
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&BOB,
			&with_fee_path_call(vec![AUSD, ACA]),
			&INFO2,
			50
		));
		System::assert_has_event(crate::mock::Event::DEXModule(module_dex::Event::Swap {
			trader: BOB,
			path: vec![AUSD, ACA],
			liquidity_changes: vec![33, fee_surplus], // 33 AUSD - 315 ACA
		}));
		assert_eq!(dex_aca - fee_surplus, Currencies::free_balance(ACA, &dex_acc));

		// DOT - ACA swap dex is invalid
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(
				&ALICE,
				&with_fee_path_call(vec![DOT, ACA]),
				&INFO2,
				50
			),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		// DOT - AUSD - ACA
		let fee: Balance = 50 * 2 + 100;
		let fee_surplus2 = fee + CustomFeeSurplus::get().mul_ceil(fee);
		assert_eq!(300, fee_surplus2);

		assert_ok!(Currencies::update_balance(Origin::root(), BOB, DOT, 10000));
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&BOB,
			&with_fee_path_call(vec![DOT, AUSD, ACA]),
			&INFO2,
			50
		));
		System::assert_has_event(crate::mock::Event::DEXModule(module_dex::Event::Swap {
			trader: BOB,
			path: vec![DOT, AUSD, ACA],
			liquidity_changes: vec![4, 34, fee_surplus2], // 4 DOT - 34 AUSD - 300 ACA
		}));
		assert_eq!(
			dex_aca - fee_surplus - fee_surplus2,
			Currencies::free_balance(ACA, &dex_acc)
		);
	});
}

#[test]
fn charges_fee_when_validate_with_fee_currency_call() {
	// Enable dex with Alice, and initialize tx charge fee pool
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let ausd_acc = Pallet::<Runtime>::sub_account_id(AUSD);
		let dot_acc = Pallet::<Runtime>::sub_account_id(DOT);
		let sub_ausd_aca = Currencies::free_balance(ACA, &ausd_acc);
		let sub_ausd_usd = Currencies::free_balance(AUSD, &ausd_acc);
		let sub_dot_aca = Currencies::free_balance(ACA, &dot_acc);
		let sub_dot_dot = Currencies::free_balance(DOT, &dot_acc);

		let fee: Balance = 50 * 2 + 100 + 10;
		let fee_perc = AlternativeFeeSurplus::get();
		let surplus = fee_perc.mul_ceil(fee); // 53
		let fee_amount = fee + surplus; // 263

		assert_ok!(Currencies::update_balance(Origin::root(), BOB, AUSD, 10000));
		assert_eq!(0, Currencies::free_balance(ACA, &BOB));
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&BOB,
			&with_fee_currency_call(AUSD),
			&INFO2,
			50
		));
		assert_eq!(10, Currencies::free_balance(ACA, &BOB)); // ED
		assert_eq!(7370, Currencies::free_balance(AUSD, &BOB));
		System::assert_has_event(crate::mock::Event::Tokens(orml_tokens::Event::Transfer {
			currency_id: AUSD,
			from: BOB,
			to: ausd_acc.clone(),
			amount: 2630,
		}));
		System::assert_has_event(crate::mock::Event::PalletBalances(pallet_balances::Event::Transfer {
			from: ausd_acc.clone(),
			to: BOB,
			amount: 263,
		}));

		assert_eq!(sub_ausd_aca - fee_amount, Currencies::free_balance(ACA, &ausd_acc));
		assert_eq!(
			sub_ausd_usd + fee_amount * 10,
			Currencies::free_balance(AUSD, &ausd_acc)
		);

		let fee: Balance = 50 * 2 + 100;
		let fee_perc = CustomFeeSurplus::get();
		let surplus = fee_perc.mul_ceil(fee);
		let fee_amount = fee + surplus;

		assert_ok!(Currencies::update_balance(Origin::root(), BOB, DOT, 10000));
		assert_eq!(10, Currencies::free_balance(ACA, &BOB));
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&BOB,
			&with_fee_currency_call(DOT),
			&INFO2,
			50
		));
		assert_eq!(sub_dot_aca - fee_amount, Currencies::free_balance(ACA, &dot_acc));
		assert_eq!(sub_dot_dot + fee_amount / 10, Currencies::free_balance(DOT, &dot_acc));
	});
}

#[test]
fn charges_fee_when_validate_with_fee_paid_by_native_token() {
	// Enable dex with Alice, and initialize tx charge fee pool
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		// make a fake signature
		let signature = MultiSignature::Sr25519(sp_core::sr25519::Signature([0u8; 64]));
		// payer has enough native asset
		assert_ok!(Currencies::update_balance(Origin::root(), BOB, ACA, 500,));

		let fee: Balance = 50 * 2 + 100;
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&ALICE,
			&with_fee_paid_by_call(BOB, signature),
			&INFO2,
			50
		));
		assert_eq!(500 - fee, Currencies::free_balance(ACA, &BOB));
	});
}

#[test]
fn charges_fee_when_validate_with_fee_paid_by_default_token() {
	// Enable dex with Alice, and initialize tx charge fee pool
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let ausd_acc = Pallet::<Runtime>::sub_account_id(AUSD);
		assert_eq!(100, Currencies::free_balance(AUSD, &ausd_acc));
		assert_eq!(10000, Currencies::free_balance(ACA, &ausd_acc));

		// make a fake signature
		let signature = MultiSignature::Sr25519(sp_core::sr25519::Signature([0u8; 64]));
		// payer has enough native asset
		assert_ok!(Currencies::update_balance(Origin::root(), BOB, AUSD, 5000,));

		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(
			&ALICE,
			&with_fee_paid_by_call(BOB, signature),
			&INFO2,
			50
		));
		assert_eq!(2700, Currencies::free_balance(AUSD, &ausd_acc));
		assert_eq!(9740, Currencies::free_balance(ACA, &ausd_acc));
		assert_eq!(2400, Currencies::free_balance(AUSD, &BOB));
		assert_eq!(10, Currencies::free_balance(ACA, &BOB));
	});
}

#[test]
fn charges_fee_when_validate_and_native_is_not_enough() {
	// Enable dex with Alice, and initialize tx charge fee pool
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let sub_account = Pallet::<Runtime>::sub_account_id(AUSD);
		let init_balance = FeePoolSize::get();
		let ausd_ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(AUSD);
		let ed: Balance = <Currencies as MultiCurrency<AccountId>>::minimum_balance(ACA);
		let rate: u128 = 10;

		// transfer token to Bob, and use Bob as tx sender to test
		// Bob do not have enough native asset(ACA), but he has AUSD
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 4000));
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(Currencies::total_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 4000);

		// native balance is lt ED, will swap fee and ED with foreign asset
		// none surplus: fee: 200, ed: 10, swap_out:200+10=210ACA, swap_in=260*10=2100AUSD
		// have surplus: fee: 200, ed: 10, surplus=200*0.25=50, swap_out:200+10+50=260ACA,
		// swap_in=260*10=2600AUSD
		let fee = 50 * 2 + 100; // len * byte + weight
		let surplus1 = AlternativeFeeSurplus::get().mul_ceil(fee);
		let expect_priority = ChargeTransactionPayment::<Runtime>::get_priority(&INFO2, 50, fee, fee);
		assert_eq!(expect_priority, 2010);
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&BOB, &CALL2, &INFO2, 50)
				.unwrap()
				.priority,
			10
		);

		assert_eq!(Currencies::total_balance(ACA, &BOB), ed);
		assert_eq!(Currencies::free_balance(ACA, &BOB), ed);
		// surplus=50ACA/500AUSD, balance=4000, swap_in=2600, left=1400
		// surplus=0, balance=4000, swap_in=2100, left=1900
		assert_eq!(Currencies::free_balance(AUSD, &BOB), 1900 - surplus1 * 10);
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(
			Currencies::free_balance(ACA, &sub_account),
			init_balance - (fee + ed + surplus1)
		);
		assert_eq!(
			Currencies::free_balance(AUSD, &sub_account),
			ausd_ed + (fee + ed + surplus1) * rate
		);

		// native balance is eq ED, cannot keep alive after charge, swap with foreign asset
		// fee: 112, ed: 10, surplus=110*0.25=28, swap_out:112+28=140ACA, swap_in=260*10=1400AUSD
		let fee2 = 6 * 2 + 100; // len * byte + weight
		let surplus2 = AlternativeFeeSurplus::get().mul_ceil(fee2);
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 6));
		assert_eq!(Currencies::total_balance(ACA, &BOB), ed);
		assert_eq!(Currencies::free_balance(ACA, &BOB), ed);
		assert_eq!(
			Currencies::free_balance(AUSD, &BOB),
			1900 - (surplus1 + fee2 + surplus2) * 10
		);
		assert_eq!(
			Currencies::free_balance(ACA, &sub_account),
			init_balance - (fee + ed + surplus1) - (fee2 + surplus2)
		);
		// two tx, first receive: (fee+ED+surplus)*10, second receive: (fee2+surplus)*10
		assert_eq!(
			Currencies::free_balance(AUSD, &sub_account),
			ausd_ed + (fee + ed + surplus1 + fee2 + surplus2) * rate
		);

		// Bob only has ED of native asset, but has not enough AUSD, validate failed.
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 1),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);
		assert_eq!(Currencies::total_balance(ACA, &BOB), 10);
		assert_eq!(Currencies::free_balance(ACA, &BOB), 10);
		assert_eq!(
			Currencies::free_balance(AUSD, &BOB),
			1900 - (surplus1 + fee2 + surplus2) * 10
		);
	});
}

#[test]
fn payment_reserve_fee() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		// Alice has enough native token: ACA
		assert_eq!(90000, Currencies::free_balance(ACA, &ALICE));
		let fee = <ChargeTransactionPayment<Runtime> as TransactionPaymentT<AccountId, Balance, _>>::reserve_fee(
			&ALICE, 100, None,
		);
		assert_eq!(100, fee.unwrap());
		assert_eq!(89900, Currencies::free_balance(ACA, &ALICE));

		let reserves = crate::mock::PalletBalances::reserves(&ALICE);
		let reserve_data = ReserveData {
			id: ReserveIdentifier::TransactionPayment,
			amount: 100,
		};
		assert_eq!(reserve_data, *reserves.get(0).unwrap());

		// Bob has not enough native token, but have enough none native token
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 4000));
		let fee = <ChargeTransactionPayment<Runtime> as TransactionPaymentT<AccountId, Balance, _>>::reserve_fee(
			&BOB, 100, None,
		);
		assert_eq!(100, fee.unwrap());
		assert_eq!(35, Currencies::free_balance(ACA, &BOB));
		assert_eq!(135, Currencies::total_balance(ACA, &BOB));
		assert_eq!(2650, Currencies::free_balance(AUSD, &BOB));

		// reserve fee not consider multiplier
		NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_rational(3, 2));
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &DAVE, 4000));
		let fee = <ChargeTransactionPayment<Runtime> as TransactionPaymentT<AccountId, Balance, _>>::reserve_fee(
			&DAVE, 100, None,
		);
		assert_eq!(100, fee.unwrap());

		let fee =
			<ChargeTransactionPayment<Runtime> as TransactionPaymentT<AccountId, Balance, _>>::apply_multiplier_to_fee(
				100, None,
			);
		assert_eq!(150, fee);
		let fee =
			<ChargeTransactionPayment<Runtime> as TransactionPaymentT<AccountId, Balance, _>>::apply_multiplier_to_fee(
				100,
				Some(Multiplier::saturating_from_rational(2, 1)),
			);
		assert_eq!(200, fee);
	});
}

#[test]
fn charges_fee_failed_by_slippage_limit() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));

		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(Currencies::total_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);

		assert_eq!(
			DEXModule::get_swap_amount(&vec![AUSD, ACA], SwapLimit::ExactTarget(Balance::MAX, 2010)),
			Some((252, 2010))
		);
		assert_eq!(
			DEXModule::get_swap_amount(&vec![AUSD, ACA], SwapLimit::ExactSupply(1000, 0)),
			Some((1000, 5000))
		);

		// pool is enough, but slippage limit the swap
		MockPriceSource::set_relative_price(Some(Price::saturating_from_rational(252, 4020)));
		assert_eq!(
			DEXModule::get_swap_amount(&vec![AUSD, ACA], SwapLimit::ExactTarget(Balance::MAX, 2010)),
			Some((252, 2010))
		);
		assert_eq!(
			DEXModule::get_swap_amount(&vec![AUSD, ACA], SwapLimit::ExactSupply(1000, 0)),
			Some((1000, 5000))
		);
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO, 500),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
	});
}

#[test]
fn set_alternative_fee_swap_path_work() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			assert_eq!(TransactionPayment::alternative_fee_swap_path(&ALICE), None);
			assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
				Origin::signed(ALICE),
				Some(vec![AUSD, ACA])
			));
			assert_eq!(
				TransactionPayment::alternative_fee_swap_path(&ALICE).unwrap(),
				vec![AUSD, ACA]
			);
			assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
				Origin::signed(ALICE),
				None
			));
			assert_eq!(TransactionPayment::alternative_fee_swap_path(&ALICE), None);

			assert_noop!(
				TransactionPayment::set_alternative_fee_swap_path(Origin::signed(ALICE), Some(vec![ACA])),
				Error::<Runtime>::InvalidSwapPath
			);

			assert_noop!(
				TransactionPayment::set_alternative_fee_swap_path(Origin::signed(ALICE), Some(vec![AUSD, DOT])),
				Error::<Runtime>::InvalidSwapPath
			);

			assert_noop!(
				TransactionPayment::set_alternative_fee_swap_path(Origin::signed(ALICE), Some(vec![ACA, ACA])),
				Error::<Runtime>::InvalidSwapPath
			);
		});
}

#[test]
fn charge_fee_by_alternative_swap_first_priority() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let sub_account = Pallet::<Runtime>::sub_account_id(DOT);
		let init_balance = FeePoolSize::get();
		let dot_ed = Currencies::minimum_balance(DOT);
		let ed = Currencies::minimum_balance(ACA);
		let alternative_fee_swap_deposit: u128 = <Runtime as Config>::AlternativeFeeSwapDeposit::get();

		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100, 1000));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			ACA,
			alternative_fee_swap_deposit.try_into().unwrap(),
		));

		assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
			Origin::signed(BOB),
			Some(vec![DOT, AUSD, ACA])
		));
		assert_eq!(
			TransactionPayment::alternative_fee_swap_path(&BOB).unwrap(),
			vec![DOT, AUSD, ACA]
		);
		// the `AlternativeFeeSwapDeposit` amount balance is in user reserve balance,
		// user reserve balance is not consider when check native is enough or not.
		assert_eq!(alternative_fee_swap_deposit, Currencies::total_balance(ACA, &BOB));

		// charge fee token use `DefaultFeeTokens` as `AlternativeFeeSwapPath` condition is failed.
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(DOT, &ALICE, &BOB, 300));
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(DOT, &BOB), 300);

		// use user's total_balance to check native is enough or not:
		// fee=500*2+1000=2000ACA, surplus=2000*0.25=500ACA, fee_amount=2500ACA
		// use user's free_balance to check native is enough or not:
		// fee=500*2+1000+10=2010ACA, surplus=2000*0.25=500ACA, fee_amount=2510ACA
		let surplus: u128 = AlternativeFeeSurplus::get().mul_ceil(2000);
		let fee_surplus: u128 = 2000 + ed + surplus;
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&BOB, &CALL2, &INFO, 500)
				.unwrap()
				.priority,
			1
		);
		System::assert_has_event(crate::mock::Event::DEXModule(module_dex::Event::Swap {
			trader: BOB,
			path: vec![DOT, AUSD, ACA],
			liquidity_changes: vec![51, 336, fee_surplus],
		}));

		assert_eq!(Currencies::free_balance(ACA, &BOB), ed);
		assert_eq!(Currencies::free_balance(AUSD, &BOB), 0);
		assert_eq!(Currencies::free_balance(DOT, &BOB), 249);
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (7490, 1336));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (151, 664));
		assert_eq!(Currencies::free_balance(ACA, &sub_account), init_balance,);
		assert_eq!(Currencies::free_balance(DOT, &sub_account), dot_ed);
	});
}

#[test]
fn charge_fee_by_default_fee_tokens_second_priority() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let sub_account = Pallet::<Runtime>::sub_account_id(DOT);
		let init_balance = FeePoolSize::get();
		let dot_ed = Currencies::minimum_balance(DOT);
		let ed = Currencies::minimum_balance(ACA);
		let alternative_fee_swap_deposit: u128 = <Runtime as Config>::AlternativeFeeSwapDeposit::get();

		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100, 1000));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			ACA,
			alternative_fee_swap_deposit.try_into().unwrap(),
		));

		// the alter native swap path is invalid as there are no pool for DOT to ACA.
		assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
			Origin::signed(BOB),
			Some(vec![DOT, ACA])
		));
		assert_eq!(
			TransactionPayment::alternative_fee_swap_path(&BOB).unwrap(),
			vec![DOT, ACA]
		);
		// the `AlternativeFeeSwapDeposit` amount balance is in user reserve balance,
		// user reserve balance is not consider when check native is enough or not.
		assert_eq!(alternative_fee_swap_deposit, Currencies::total_balance(ACA, &BOB));

		// charge fee token use `DefaultFeeTokens` as `AlternativeFeeSwapPath` condition is failed.
		assert_ok!(<Currencies as MultiCurrency<_>>::transfer(DOT, &ALICE, &BOB, 300));
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
		assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(DOT, &BOB), 300);

		// use user's total_balance to check native is enough or not:
		// fee=500*2+1000=2000ACA, surplus=2000*0.25=500ACA, fee_amount=2500ACA
		// use user's free_balance to check native is enough or not:
		// fee=500*2+1000+10=2010ACA, surplus=2000*0.25=500ACA, fee_amount=2510ACA
		let surplus: u128 = AlternativeFeeSurplus::get().mul_ceil(2000);
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(&BOB, &CALL2, &INFO, 500)
				.unwrap()
				.priority,
			1
		);

		assert_eq!(Currencies::free_balance(ACA, &BOB), ed);
		assert_eq!(Currencies::free_balance(AUSD, &BOB), 0);
		assert_eq!(Currencies::free_balance(DOT, &BOB), 300 - 200 - surplus / 10 - ed / 10);
		assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100, 1000));
		assert_eq!(
			Currencies::free_balance(ACA, &sub_account),
			init_balance - 2000 - surplus - ed,
		);
		assert_eq!(
			Currencies::free_balance(DOT, &sub_account),
			dot_ed + 200 + surplus / 10 + ed / 10
		);
	});
}

#[test]
fn query_info_works() {
	ExtBuilder::default()
		.base_weight(5)
		.byte_fee(1)
		.weight_fee(2)
		.build()
		.execute_with(|| {
			let call = Call::PalletBalances(pallet_balances::Call::transfer {
				dest: AccountId::new([2u8; 32]),
				value: 69,
			});
			let origin = 111111;
			let extra = ();
			let xt = TestXt::new(call, Some((origin, extra)));
			let info = xt.get_dispatch_info();
			let ext = xt.encode();
			let len = ext.len() as u32;

			// all fees should be x1.5
			NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_rational(3, 2));

			assert_eq!(
				TransactionPayment::query_info(xt, len),
				RuntimeDispatchInfo {
					weight: info.weight,
					class: info.class,
					partial_fee: 5 * 2 /* base_weight * weight_fee */
						+ len as u128  /* len * byte_fee */
						+ info.weight.min(BlockWeights::get().max_block) as u128 * 2 * 3 / 2 /* weight */
				},
			);
		});
}

#[test]
fn compute_fee_works_without_multiplier() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			// Next fee multiplier is zero
			assert_eq!(NextFeeMultiplier::<Runtime>::get(), Multiplier::one());

			// Tip only, no fees works
			let dispatch_info = DispatchInfo {
				weight: 0,
				class: DispatchClass::Operational,
				pays_fee: Pays::No,
			};
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 10), 10);
			// No tip, only base fee works
			let dispatch_info = DispatchInfo {
				weight: 0,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 0), 100);
			// Tip + base fee works
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 69), 169);
			// Len (byte fee) + base fee works
			assert_eq!(Pallet::<Runtime>::compute_fee(42, &dispatch_info, 0), 520);
			// Weight fee + base fee works
			let dispatch_info = DispatchInfo {
				weight: 1000,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 0), 1100);
		});
}

#[test]
fn compute_fee_works_with_multiplier() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			// Add a next fee multiplier. Fees will be x3/2.
			NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_rational(3, 2));
			// Base fee is unaffected by multiplier
			let dispatch_info = DispatchInfo {
				weight: 0,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 0), 100);

			// Everything works together :)
			let dispatch_info = DispatchInfo {
				weight: 123,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			// 123 weight, 456 length, 100 base
			assert_eq!(
				Pallet::<Runtime>::compute_fee(456, &dispatch_info, 789),
				100 + (3 * 123 / 2) + 4560 + 789,
			);
		});
}

#[test]
fn compute_fee_works_with_negative_multiplier() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			// Add a next fee multiplier. All fees will be x1/2.
			NextFeeMultiplier::<Runtime>::put(Multiplier::saturating_from_rational(1, 2));

			// Base fee is unaffected by multiplier.
			let dispatch_info = DispatchInfo {
				weight: 0,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			assert_eq!(Pallet::<Runtime>::compute_fee(0, &dispatch_info, 0), 100);

			// Everything works together.
			let dispatch_info = DispatchInfo {
				weight: 123,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			// 123 weight, 456 length, 100 base
			assert_eq!(
				Pallet::<Runtime>::compute_fee(456, &dispatch_info, 789),
				100 + (123 / 2) + 4560 + 789,
			);
		});
}

#[test]
fn compute_fee_does_not_overflow() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			// Overflow is handled
			let dispatch_info = DispatchInfo {
				weight: Weight::max_value(),
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			assert_eq!(
				Pallet::<Runtime>::compute_fee(<u32>::max_value(), &dispatch_info, <u128>::max_value()),
				<u128>::max_value()
			);
		});
}

#[test]
fn should_alter_operational_priority() {
	let tip = 5;
	let len = 10;

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let normal = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;

			assert_eq!(priority, 60);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;

			assert_eq!(priority, 110);
		});

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let op = DispatchInfo {
				weight: 100,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &op, len)
				.unwrap()
				.priority;
			// final_fee = base_fee + len_fee + adjusted_weight_fee + tip = 0 + 20 + 100 + 5 = 125
			// priority = final_fee * fee_multiplier * max_tx_per_block + (tip + 1) * max_tx_per_block
			//          = 125 * 5 * 10 + 60 = 6310
			assert_eq!(priority, 6310);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, &CALL, &op, len)
				.unwrap()
				.priority;
			// final_fee = base_fee + len_fee + adjusted_weight_fee + tip = 0 + 20 + 100 + 10 = 130
			// priority = final_fee * fee_multiplier * max_tx_per_block + (tip + 1) * max_tx_per_block
			//          = 130 * 5 * 10 + 110 = 6610
			assert_eq!(priority, 6610);
		});
}

#[test]
fn no_tip_has_some_priority() {
	let tip = 0;
	let len = 10;

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let normal = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);
		});

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let op = DispatchInfo {
				weight: 100,
				class: DispatchClass::Operational,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &op, len)
				.unwrap()
				.priority;
			// final_fee = base_fee + len_fee + adjusted_weight_fee + tip = 0 + 20 + 100 + 0 = 120
			// priority = final_fee * fee_multiplier * max_tx_per_block + (tip + 1) * max_tx_per_block
			//          = 120 * 5 * 10 + 10 = 6010
			assert_eq!(priority, 6010);
		});
}

#[test]
fn min_tip_has_same_priority() {
	let tip = 100;
	let len = 10;

	ExtBuilder::default()
		.tip_per_weight_step(tip)
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let normal = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(0)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 0);

			let priority = ChargeTransactionPayment::<Runtime>(tip - 2)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 0);

			let priority = ChargeTransactionPayment::<Runtime>(tip - 1)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip - 2)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip - 1)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 20);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 20);
		});
}

#[test]
fn max_tip_has_same_priority() {
	let tip = 1000;
	let len = 10;

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let normal = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10_000);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, &CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10_000);
		});
}

struct CurrencyIdConvert;
impl Convert<MultiLocation, Option<CurrencyId>> for CurrencyIdConvert {
	fn convert(location: MultiLocation) -> Option<CurrencyId> {
		use CurrencyId::Token;
		use TokenSymbol::*;

		if location == MultiLocation::parent() {
			return Some(Token(DOT));
		}

		match location {
			MultiLocation {
				interior: X1(GeneralKey(key)),
				..
			} => match &key[..] {
				key => {
					if let Ok(currency_id) = CurrencyId::decode(&mut &*key) {
						Some(currency_id)
					} else {
						None
					}
				}
			},
			_ => None,
		}
	}
}

#[test]
fn buy_weight_transaction_fee_pool_works() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		// Location convert return None.
		let location = MultiLocation::new(1, X1(Junction::Parachain(2000)));
		let rate = <BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>::calculate_rate(location);
		assert_eq!(rate, None);

		// Token not in charge fee pool
		let currency_id = CurrencyId::Token(TokenSymbol::LDOT);
		let location = MultiLocation::new(1, X1(GeneralKey(currency_id.encode())));
		let rate = <BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>::calculate_rate(location);
		assert_eq!(rate, None);

		// DOT Token is in charge fee pool.
		let location = MultiLocation::parent();
		let rate = <BuyWeightRateOfTransactionFeePool<Runtime, CurrencyIdConvert>>::calculate_rate(location);
		assert_eq!(rate, Some(Ratio::saturating_from_rational(1, 10)));
	});
}

#[test]
fn swap_from_pool_not_enough_currency() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let balance = 100 as u128;
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			DOT,
			balance.unique_saturated_into(),
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			AUSD,
			balance.unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(DOT, &BOB), 100);
		assert_eq!(Currencies::free_balance(AUSD, &BOB), 100);

		// 1100 ACA equals to 110 DOT, but Bob only has 100 DOT
		let result = Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, 1100, DOT);
		assert!(result.is_err());
		// 11 ACA equals to 110 AUSD, but Bob only has 100 AUSD
		let result = Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, 11, AUSD);
		assert!(result.is_err());
	});
}

#[test]
fn swap_from_pool_with_enough_balance() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let pool_size = FeePoolSize::get();
		let dot_fee_account = Pallet::<Runtime>::sub_account_id(DOT);
		let usd_fee_account = Pallet::<Runtime>::sub_account_id(AUSD);
		let dot_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(DOT);
		let usd_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(AUSD);

		// 1 DOT = 10 ACA, swap 500 ACA with 50 DOT
		let balance = 500 as u128;
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			DOT,
			balance.unique_saturated_into(),
		));
		let fee = balance; // 500 ACA
		let expect_treasury_dot = (balance / 10) as u128; // 50 DOT
		let expect_user_dot = balance - expect_treasury_dot; // 450 DOT
		let expect_treasury_aca = (pool_size - fee) as u128; // 500 ACA
		let expect_user_aca = fee; // 500 ACA

		assert_ok!(Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, fee, DOT));
		assert_eq!(expect_user_dot, Currencies::free_balance(DOT, &BOB));
		assert_eq!(
			expect_treasury_dot,
			Currencies::free_balance(DOT, &dot_fee_account) - dot_ed
		);
		assert_eq!(expect_user_aca, Currencies::free_balance(ACA, &BOB));
		assert_eq!(expect_treasury_aca, Currencies::free_balance(ACA, &dot_fee_account));

		// 1 ACA = 10 AUSD, swap 500 ACA with 5000 AUSD
		let balance = 500 as u128;
		let ausd_balance = (balance * 11) as u128; // 5500 AUSD
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			AUSD,
			ausd_balance.unique_saturated_into(),
		));
		assert_eq!(0, Currencies::free_balance(AUSD, &usd_fee_account) - usd_ed);
		let fee = balance; // 500 ACA
		let expect_treasury_ausd = (balance * 10) as u128; // 5000 AUSD
		let expect_user_ausd = balance; // (balance * 11) - (balance * 10) = balance = 500 AUSD
		let expect_treasury_aca = pool_size - fee; // 1000 ACA - 500 ACA
		let expect_user_aca = expect_user_aca + fee; // 500 ACA

		assert_ok!(Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, fee, AUSD));
		assert_eq!(expect_user_ausd, Currencies::free_balance(AUSD, &BOB));
		assert_eq!(
			expect_treasury_ausd,
			Currencies::free_balance(AUSD, &usd_fee_account) - usd_ed
		);
		assert_eq!(expect_user_aca, Currencies::free_balance(ACA, &BOB));
		assert_eq!(expect_treasury_aca, Currencies::free_balance(ACA, &usd_fee_account));
	});
}

#[test]
fn swap_from_pool_and_dex_with_higher_threshold() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let pool_size = FeePoolSize::get();
		let dot_fee_account = Pallet::<Runtime>::sub_account_id(DOT);
		let dot_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(DOT);

		// Bob has 800 DOT, the fee is 800 ACA, equal to 80 DOT
		let balance = 800 as u128;
		let fee_dot = 80 as u128;
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			DOT,
			balance.unique_saturated_into(),
		));

		// First transaction success get 800 ACA as fee from pool
		Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance, DOT).unwrap();
		// Bob withdraw 80 DOT(remain 720), and deposit 800 ACA
		assert_eq!(balance - fee_dot, Currencies::free_balance(DOT, &BOB));
		assert_eq!(balance, Currencies::free_balance(ACA, &BOB));
		// sub account deposit 80 DOT, and withdraw 800 ACA(remain 9200)
		assert_eq!(fee_dot + dot_ed, Currencies::free_balance(DOT, &dot_fee_account));
		assert_eq!(pool_size - balance, Currencies::free_balance(ACA, &dot_fee_account));

		let old_exchange_rate = TokenExchangeRate::<Runtime>::get(DOT).unwrap();
		assert_eq!(old_exchange_rate, Ratio::saturating_from_rational(fee_dot, balance));

		// Set threshold(init-500) gt sub account balance(init-800), trigger swap from dex.
		SwapBalanceThreshold::<Runtime>::insert(DOT, crate::mock::HigerSwapThreshold::get());
		SwapBalanceThreshold::<Runtime>::insert(AUSD, crate::mock::HigerSwapThreshold::get());

		// swap 80 DOT out 3074 ACA
		let trading_path = DotFeeSwapPath::get();
		let supply_amount = Currencies::free_balance(DOT, &dot_fee_account) - dot_ed;
		// here just get swap out amount, the swap not happened
		let (supply_in_amount, swap_out_native) =
			module_dex::Pallet::<Runtime>::get_swap_amount(&trading_path, SwapLimit::ExactSupply(supply_amount, 0))
				.unwrap();
		assert_eq!(3074, swap_out_native);
		assert_eq!(supply_in_amount, supply_amount);
		let new_pool_size =
			(swap_out_native + Currencies::free_balance(ACA, &dot_fee_account)).saturated_into::<u128>();

		// the swap also has it's own exchange rate by input_amount divide output_amount
		let swap_exchange_rate = Ratio::saturating_from_rational(supply_in_amount, swap_out_native);
		assert_eq!(swap_exchange_rate, Ratio::saturating_from_rational(80, 3074));

		// swap_rate=80/3074, threshold=9500, pool_size=10000, threshold_rate=0.95, old_rate=1/10
		// new_rate = 1/10 * 0.95 + 80/3074 * 0.05 = 0.095 + 0.001301236174365 = 0.096301236174365
		let new_exchange_rate_val =
			Ratio::saturating_from_rational(9_630_123_6174_365_647 as u128, 1_000_000_000_000_000_000 as u128);

		// the sub account has 9200 ACA, 80 DOT, use 80 DOT to swap out some ACA
		let balance2 = 300 as u128;
		assert_ok!(Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance2, DOT));
		System::assert_has_event(crate::mock::Event::TransactionPayment(
			crate::Event::ChargeFeePoolSwapped {
				sub_account: dot_fee_account,
				supply_currency_id: DOT,
				old_exchange_rate,
				swap_exchange_rate,
				new_exchange_rate: new_exchange_rate_val,
				new_pool_size,
			},
		));

		let new_rate = TokenExchangeRate::<Runtime>::get(DOT).unwrap();
		assert_eq!(new_exchange_rate_val, new_rate);
		assert_eq!(PoolSize::<Runtime>::get(DOT), new_pool_size);
	});
}

#[test]
fn swap_from_pool_and_dex_with_midd_threshold() {
	builder_with_dex_and_fee_pool(true).execute_with(|| {
		let sub_account: AccountId = <Runtime as Config>::PalletId::get().into_sub_account_truncating(DOT);
		let dot_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(DOT);
		let trading_path = vec![DOT, AUSD, ACA];

		// the pool size has 10000 ACA, and set threshold to half of pool size: 5000 ACA
		let balance = 3000 as u128;
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			DOT,
			balance.unique_saturated_into(),
		));

		SwapBalanceThreshold::<Runtime>::insert(DOT, crate::mock::MiddSwapThreshold::get());
		SwapBalanceThreshold::<Runtime>::insert(AUSD, crate::mock::MiddSwapThreshold::get());

		// After tx#1, ACA balance of sub account is large than threshold(5000 ACA)
		Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance, DOT).unwrap();
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 7000);
		assert_eq!(Currencies::free_balance(DOT, &sub_account), 301);

		// After tx#2, ACA balance of sub account is less than threshold(5000 ACA)
		Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance, DOT).unwrap();
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 4000);
		assert_eq!(Currencies::free_balance(DOT, &sub_account), 601);

		let supply_amount = Currencies::free_balance(DOT, &sub_account) - dot_ed;
		// Given different DOT, get the swap out ACA
		// DOT | ACA  | SwapRate
		// 001 | 0089 | FixedU128(0.011235955056179775)
		// 050 | 2498 | FixedU128(0.020016012810248198)
		// 100 | 3333 | FixedU128(0.030003000300030003)
		// 200 | 3997 | FixedU128(0.050037528146109582)
		// 300 | 4285 | FixedU128(0.070011668611435239)
		// 500 | 4544 | FixedU128(0.110035211267605633)
		// 600 | 4614 | FixedU128(0.130039011703511053) <- this case hit here
		let (supply_in_amount, swap_out_native) =
			module_dex::Pallet::<Runtime>::get_swap_amount(&trading_path, SwapLimit::ExactSupply(supply_amount, 0))
				.unwrap();
		assert_eq!(600, supply_in_amount);
		assert_eq!(4614, swap_out_native);
		// new pool size = swap_out_native + ACA balance of sub account
		let new_pool_size = swap_out_native + 4000;

		// When execute tx#3, trigger swap from dex, but this tx still use old rate(1/10)
		Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance, DOT).unwrap();
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 5614); // 4000+4614-3000=5614
		assert_eq!(Currencies::free_balance(DOT, &sub_account), 301);

		let old_exchange_rate = Ratio::saturating_from_rational(1, 10);
		let swap_exchange_rate = Ratio::saturating_from_rational(supply_in_amount, swap_out_native);
		// (0.1 + 0.130039011703511053)/2 = 0.230039011703511053/2 = 0.115019505851755526
		let new_exchange_rate_val =
			Ratio::saturating_from_rational(115_019_505_851_755_526 as u128, 1_000_000_000_000_000_000 as u128);

		System::assert_has_event(crate::mock::Event::TransactionPayment(
			crate::Event::ChargeFeePoolSwapped {
				sub_account: sub_account.clone(),
				supply_currency_id: DOT,
				old_exchange_rate,
				swap_exchange_rate,
				new_exchange_rate: new_exchange_rate_val,
				new_pool_size,
			},
		));

		// tx#3 use new exchange rate
		Pallet::<Runtime>::swap_from_pool_or_dex(&BOB, balance, DOT).unwrap();
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 2614);
		assert_eq!(Currencies::free_balance(DOT, &sub_account), 301 + 3 * 115);
	});
}

#[test]
#[should_panic(expected = "Swap tx fee pool should not fail!")]
fn charge_fee_failed_when_disable_dex() {
	use module_dex::TradingPairStatus;
	use primitives::TradingPair;

	ExtBuilder::default().build().execute_with(|| {
		let fee_account = Pallet::<Runtime>::sub_account_id(AUSD);
		let pool_size = FeePoolSize::get();
		let swap_balance_threshold = (pool_size - 200) as u128;
		let ausd_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(AUSD);
		let ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(ACA);
		let trading_path = AusdFeeSwapPath::get();

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			BOB,
			AUSD,
			100000.unique_saturated_into(),
		));

		// tx failed because of dex not enabled even though user has enough AUSD
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		enable_dex_and_tx_fee_pool();

		// after runtime upgrade, tx success because of dex enabled and has enough token balance
		// fee=50*2+100=200, ED=10, surplus=200*0.25=50, fee_amount=260, ausd_swap=260*10=2600
		let surplus = AlternativeFeeSurplus::get().mul_ceil(200);
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50));
		assert_eq!(100000 - (210 + surplus) * 10, Currencies::free_balance(AUSD, &BOB));

		// update threshold, next tx will trigger swap
		SwapBalanceThreshold::<Runtime>::insert(AUSD, swap_balance_threshold);

		// trading pair is enabled
		let pair = TradingPair::from_currency_ids(AUSD, ACA).unwrap();
		assert_eq!(
			module_dex::Pallet::<Runtime>::trading_pair_statuses(pair),
			TradingPairStatus::Enabled
		);
		// make sure swap is valid
		let swap_result = module_dex::Pallet::<Runtime>::get_swap_amount(&trading_path, SwapLimit::ExactSupply(1, 0));
		assert!(swap_result.is_some());
		assert_ok!(module_dex::Pallet::<Runtime>::swap_with_specific_path(
			&ALICE,
			&trading_path,
			SwapLimit::ExactSupply(100, 0)
		));

		// balance lt threshold, trigger swap from dex
		assert_eq!(
			ausd_ed + (210 + surplus) * 10,
			Currencies::free_balance(AUSD, &fee_account)
		);
		assert_eq!(9790 - surplus, Currencies::free_balance(ACA, &fee_account));
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50));
		// AlternativeFeeSurplus=25%, swap 2600 AUSD with 6388 ACA, pool_size=9740+6388=16128
		// fee=50*2+100=200, surplus=200*0.25=50, fee_amount=250, ausd_swap=250*10=2500
		let fee_aca = Currencies::free_balance(ACA, &fee_account);
		assert_eq!(
			ausd_ed + (200 + surplus) * 10,
			Currencies::free_balance(AUSD, &fee_account)
		);
		if AlternativeFeeSurplus::get() == Percent::from_percent(25) {
			// pool_size=16128, one tx cost ACA=250(with surplus), result=16128-250=15878
			assert_eq!(15878, fee_aca);
			System::assert_has_event(crate::mock::Event::TransactionPayment(
				crate::Event::ChargeFeePoolSwapped {
					sub_account: fee_account.clone(),
					supply_currency_id: AUSD,
					old_exchange_rate: Ratio::saturating_from_rational(10, 1),
					swap_exchange_rate: Ratio::saturating_from_rational(
						407_013_149_655_604_257 as u128,
						1_000_000_000_000_000_000 as u128,
					),
					new_exchange_rate: Ratio::saturating_from_rational(
						9_808_140_262_993_112_085 as u128,
						1_000_000_000_000_000_000 as u128,
					),
					new_pool_size: 16128,
				},
			));
		} else if AlternativeFeeSurplus::get() == Percent::from_percent(0) {
			// pool_size=15755, one tx cost ACA=200(without surplus), result=15755-200=15555
			assert_eq!(15555, fee_aca);
			System::assert_has_event(crate::mock::Event::TransactionPayment(
				crate::Event::ChargeFeePoolSwapped {
					sub_account: fee_account.clone(),
					supply_currency_id: AUSD,
					old_exchange_rate: Ratio::saturating_from_rational(10, 1),
					swap_exchange_rate: Ratio::saturating_from_rational(
						352053646269907795 as u128,
						1_000_000_000_000_000_000 as u128,
					),
					new_exchange_rate: Ratio::saturating_from_rational(
						9807041072925398155 as u128,
						1_000_000_000_000_000_000 as u128,
					),
					new_pool_size: 15755,
				},
			));
		}

		// when trading pair disabled, the swap action will failed
		assert_ok!(module_dex::Pallet::<Runtime>::disable_trading_pair(
			Origin::signed(AccountId::new([0u8; 32])),
			AUSD,
			ACA
		));
		assert_eq!(
			module_dex::Pallet::<Runtime>::trading_pair_statuses(pair),
			TradingPairStatus::Disabled
		);
		let res = module_dex::Pallet::<Runtime>::swap_with_specific_path(
			&ALICE,
			&trading_path,
			SwapLimit::ExactSupply(100, 0),
		);
		assert!(res.is_err());

		// but `swap_from_pool_or_dex` still can work, because tx fee pool is not disabled.
		// after swap, the balance gt threshold, tx still success because not trigger swap.
		// the rate is using new exchange rate, but swap native asset still keep 250 ACA.
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50));

		let fee_balance = Currencies::free_balance(ACA, &fee_account);
		assert_eq!(fee_aca - (200 + surplus), fee_balance);
		assert_eq!(fee_balance > swap_balance_threshold, true);
		let swap_balance_threshold = (fee_balance - 199) as u128;

		SwapBalanceThreshold::<Runtime>::insert(AUSD, swap_balance_threshold);

		// this tx success because before execution, native_balance > threshold
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50));
		// assert_eq!(15378, Currencies::free_balance(ACA, &fee_account));
		assert_eq!(
			fee_aca - (200 + surplus) * 2,
			Currencies::free_balance(ACA, &fee_account)
		);
		assert_eq!(ed, Currencies::free_balance(ACA, &BOB));

		// this tx failed because when execute, native_balance < threshold, the dex swap failed
		assert_ok!(ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, &CALL2, &INFO2, 50));
	});
}

#[test]
fn charge_fee_pool_operation_works() {
	ExtBuilder::default().build().execute_with(|| {
		let alternative_fee_swap_deposit: u128 = <Runtime as Config>::AlternativeFeeSwapDeposit::get();
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			alternative_fee_swap_deposit.try_into().unwrap(),
		));
		assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
			Origin::signed(ALICE),
			Some(vec![AUSD, ACA])
		));
		assert_eq!(
			TransactionPayment::alternative_fee_swap_path(&ALICE).unwrap(),
			vec![AUSD, ACA]
		);

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			10000.unique_saturated_into(),
		));

		assert_ok!(DEXModule::add_liquidity(
			Origin::signed(ALICE),
			ACA,
			AUSD,
			10000,
			1000,
			0,
			false
		));

		let treasury_account: AccountId = <Runtime as Config>::TreasuryAccount::get();
		let sub_account: AccountId = <Runtime as Config>::PalletId::get().into_sub_account_truncating(AUSD);
		let usd_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(AUSD);
		let pool_size = FeePoolSize::get();
		let swap_threshold = crate::mock::MiddSwapThreshold::get();

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			treasury_account.clone(),
			ACA,
			(pool_size * 2).unique_saturated_into(),
		));
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			treasury_account.clone(),
			AUSD,
			(usd_ed * 2).unique_saturated_into(),
		));

		assert_ok!(Pallet::<Runtime>::enable_charge_fee_pool(
			Origin::signed(ALICE),
			AUSD,
			AusdFeeSwapPath::get(),
			pool_size,
			swap_threshold
		));
		let rate = TokenExchangeRate::<Runtime>::get(AUSD);
		assert_eq!(rate, Some(Ratio::saturating_from_rational(2, 10)));
		System::assert_has_event(crate::mock::Event::TransactionPayment(
			crate::Event::ChargeFeePoolEnabled {
				sub_account: sub_account.clone(),
				currency_id: AUSD,
				fee_swap_path: AusdFeeSwapPath::get(),
				exchange_rate: Ratio::saturating_from_rational(2, 10),
				pool_size,
				swap_threshold,
			},
		));

		assert_noop!(
			Pallet::<Runtime>::enable_charge_fee_pool(
				Origin::signed(ALICE),
				AUSD,
				AusdFeeSwapPath::get(),
				pool_size,
				swap_threshold
			),
			Error::<Runtime>::ChargeFeePoolAlreadyExisted
		);

		assert_noop!(
			Pallet::<Runtime>::enable_charge_fee_pool(
				Origin::signed(ALICE),
				KSM,
				vec![KSM, ACA],
				pool_size,
				swap_threshold
			),
			Error::<Runtime>::DexNotAvailable
		);
		assert_noop!(
			Pallet::<Runtime>::disable_charge_fee_pool(Origin::signed(ALICE), KSM),
			Error::<Runtime>::InvalidToken
		);

		let ausd_amount1 = <Currencies as MultiCurrency<AccountId>>::free_balance(AUSD, &sub_account);
		let aca_amount1 = crate::mock::PalletBalances::free_balance(&sub_account);
		assert_ok!(Pallet::<Runtime>::disable_charge_fee_pool(Origin::signed(ALICE), AUSD));
		assert_eq!(TokenExchangeRate::<Runtime>::get(AUSD), None);
		System::assert_has_event(crate::mock::Event::TransactionPayment(
			crate::Event::ChargeFeePoolDisabled {
				currency_id: AUSD,
				foreign_amount: ausd_amount1,
				native_amount: aca_amount1,
			},
		));
		let ausd_amount2 = <Currencies as MultiCurrency<AccountId>>::free_balance(AUSD, &sub_account);
		let aca_amount2 = crate::mock::PalletBalances::free_balance(&sub_account);
		assert_eq!(aca_amount2, 0);
		assert_eq!(ausd_amount2, 0);

		assert_ok!(Pallet::<Runtime>::enable_charge_fee_pool(
			Origin::signed(ALICE),
			AUSD,
			AusdFeeSwapPath::get(),
			pool_size,
			swap_threshold
		));
	});
}

#[test]
fn with_fee_path_currency_call_validation_works() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			// fee swap path invalid
			assert_noop!(
				ChargeTransactionPayment::<Runtime>::from(0).pre_dispatch(
					&ALICE,
					&with_fee_path_call(vec![AUSD, DOT]),
					&INFO,
					500
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
			assert_noop!(
				ChargeTransactionPayment::<Runtime>::from(0).pre_dispatch(
					&ALICE,
					&with_fee_path_call(vec![ACA]),
					&INFO,
					500
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
			// swap failed
			assert_noop!(
				ChargeTransactionPayment::<Runtime>::from(0).pre_dispatch(
					&ALICE,
					&with_fee_path_call(vec![AUSD, ACA]),
					&INFO,
					500
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);

			assert_ok!(TransactionPayment::with_fee_path(
				Origin::signed(ALICE),
				vec![],
				Box::new(CALL),
			),);
			assert_eq!(9900, Currencies::free_balance(AUSD, &ALICE));
			assert_eq!(100, Currencies::free_balance(AUSD, &BOB));

			assert_ok!(TransactionPayment::with_fee_path(
				Origin::signed(ALICE),
				vec![DOT, ACA],
				Box::new(CALL),
			));
			assert_eq!(9800, Currencies::free_balance(AUSD, &ALICE));
			assert_eq!(200, Currencies::free_balance(AUSD, &BOB));

			assert_noop!(
				ChargeTransactionPayment::<Runtime>::from(0).pre_dispatch(
					&ALICE,
					&with_fee_currency_call(DOT),
					&INFO,
					500
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
			assert_ok!(TransactionPayment::with_fee_currency(
				Origin::signed(ALICE),
				DOT,
				Box::new(CALL),
			),);
			assert_eq!(9700, Currencies::free_balance(AUSD, &ALICE));
			assert_eq!(300, Currencies::free_balance(AUSD, &BOB));
		});
}
