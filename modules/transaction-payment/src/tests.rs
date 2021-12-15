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

//! Unit tests for the transaction payment module.

#![cfg(test)]

use super::*;
use frame_support::{
	assert_noop, assert_ok,
	weights::{DispatchClass, DispatchInfo, Pays},
};
use mock::{
	AccountId, BlockWeights, Call, Currencies, DEXModule, ExtBuilder, MockPriceSource, Origin, Runtime,
	TransactionPayment, ACA, ALICE, AUSD, BOB, CHARLIE, DOT, FEE_UNBALANCED_AMOUNT, TIP_UNBALANCED_AMOUNT,
};
use orml_traits::MultiCurrency;
use sp_runtime::{testing::TestXt, traits::One};
use support::Price;

const CALL: &<Runtime as frame_system::Config>::Call = &Call::Currencies(module_currencies::Call::transfer {
	dest: BOB,
	currency_id: AUSD,
	amount: 12,
});

const CALL2: &<Runtime as frame_system::Config>::Call =
	&Call::Currencies(module_currencies::Call::transfer_native_currency { dest: BOB, amount: 12 });

const INFO: DispatchInfo = DispatchInfo {
	weight: 1000,
	class: DispatchClass::Normal,
	pays_fee: Pays::Yes,
};

const POST_INFO: PostDispatchInfo = PostDispatchInfo {
	actual_weight: Some(800),
	pays_fee: Pays::Yes,
};

#[test]
fn charges_fee_when_native_is_enough_but_cannot_keep_alive() {
	ExtBuilder::default().build().execute_with(|| {
		let fee = 23 * 2 + 1000; // len * byte + weight
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			fee.unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), fee);
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&ALICE, CALL, &INFO, 23),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		let fee2 = 23 * 2 + 990;
		assert_eq!(
			ChargeTransactionPayment::<Runtime>::from(0)
				.validate(
					&ALICE,
					CALL,
					&DispatchInfo {
						weight: 990,
						class: DispatchClass::Normal,
						pays_fee: Pays::Yes,
					},
					23
				)
				.unwrap()
				.priority,
			fee2.saturated_into::<u64>()
		);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), Currencies::minimum_balance(ACA));
	});
}

#[test]
fn charges_fee() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let fee = 23 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&ALICE, CALL, &INFO, 23)
					.unwrap()
					.priority,
				fee
			);
			assert_eq!(Currencies::free_balance(ACA, &ALICE), (100000 - fee).into());

			let fee2 = 18 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&ALICE, CALL2, &INFO, 18)
					.unwrap()
					.priority,
				fee2
			);
			assert_eq!(
				Currencies::free_balance(ACA, &ALICE),
				(100000 - fee - fee2).unique_saturated_into()
			);
		});
}

#[test]
fn signed_extension_transaction_payment_work() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let fee = 23 * 2 + 1000; // len * byte + weight
			let pre = ChargeTransactionPayment::<Runtime>::from(0)
				.pre_dispatch(&ALICE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);
			assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
				pre,
				&INFO,
				&POST_INFO,
				23,
				&Ok(())
			));

			let refund = 200; // 1000 - 800
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);
			assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()), fee - refund);
			assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), 0);

			FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow_mut() = 0);

			let pre = ChargeTransactionPayment::<Runtime>::from(5 /* tipped */)
				.pre_dispatch(&CHARLIE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - 5);
			assert_ok!(ChargeTransactionPayment::<Runtime>::post_dispatch(
				pre,
				&INFO,
				&POST_INFO,
				23,
				&Ok(())
			));
			assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - 5 + refund);
			assert_eq!(FEE_UNBALANCED_AMOUNT.with(|a| *a.borrow()), fee - refund);
			assert_eq!(TIP_UNBALANCED_AMOUNT.with(|a| *a.borrow()), 5);
		});
}

#[test]
fn charges_fee_when_pre_dispatch_and_native_currency_is_enough() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let fee = 23 * 2 + 1000; // len * byte + weight
			assert!(ChargeTransactionPayment::<Runtime>::from(0)
				.pre_dispatch(&ALICE, CALL, &INFO, 23)
				.is_ok());
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);
		});
}

#[test]
fn refund_fee_according_to_actual_when_post_dispatch_and_native_currency_is_enough() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let fee = 23 * 2 + 1000; // len * byte + weight
			let pre = ChargeTransactionPayment::<Runtime>::from(0)
				.pre_dispatch(&ALICE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

			let refund = 200; // 1000 - 800
			assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(pre, &INFO, &POST_INFO, 23, &Ok(())).is_ok());
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);
		});
}

#[test]
fn charges_fee_when_validate_and_native_is_not_enough() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			// add liquidity to DEX
			assert_ok!(DEXModule::add_liquidity(
				Origin::signed(ALICE),
				ACA,
				AUSD,
				10000,
				1000,
				0,
				false
			));
			assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));

			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(Currencies::total_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);

			// total balance is lt ED, will swap fee and ED
			let fee = 500 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO, 500)
					.unwrap()
					.priority,
				fee
			);
			assert_eq!(Currencies::total_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 748);
			assert_eq!(
				DEXModule::get_liquidity_pool(ACA, AUSD),
				(10000 - 2000 - 10, 1000 + 252)
			);

			// total balance is gte ED, but cannot keep alive after charge,
			// will swap extra gap to keep alive
			let fee_2 = 100 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO, 100)
					.unwrap()
					.priority,
				fee_2
			);
			assert_eq!(Currencies::total_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 526);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (7990 - 1200, 1252 + 222));
		});
}

#[test]
fn charges_fee_failed_by_slippage_limit() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			// add liquidity to DEX
			assert_ok!(DEXModule::add_liquidity(
				Origin::signed(ALICE),
				ACA,
				AUSD,
				10000,
				1000,
				0,
				false
			));
			assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 1000));

			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(Currencies::total_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 1000);

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
				ChargeTransactionPayment::<Runtime>::from(0).validate(&BOB, CALL2, &INFO, 500),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
		});
}

#[test]
fn set_alternative_fee_swap_path_work() {
	ExtBuilder::default().build().execute_with(|| {
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
fn charge_fee_by_default_swap_path() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			// add liquidity to DEX
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
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100, 1000));
			assert_ok!(TransactionPayment::set_alternative_fee_swap_path(
				Origin::signed(BOB),
				Some(vec![DOT, ACA])
			));
			assert_eq!(
				TransactionPayment::alternative_fee_swap_path(&BOB).unwrap(),
				vec![DOT, ACA]
			);
			assert_ok!(<Currencies as MultiCurrency<_>>::transfer(DOT, &ALICE, &BOB, 100));
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(DOT, &BOB), 100);

			let fee = 500 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO, 500)
					.unwrap()
					.priority,
				fee
			);

			assert_eq!(Currencies::free_balance(ACA, &BOB), Currencies::minimum_balance(ACA));
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 0);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 100 - 34);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000 - 2000 - 10, 1252));
			assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100 + 34, 1000 - 252));
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
					partial_fee: 5 * 2 /* base * weight_fee */
						+ len as u128  /* len * 1 */
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
