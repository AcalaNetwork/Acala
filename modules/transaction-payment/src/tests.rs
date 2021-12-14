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
use primitives::currency::*;
use sp_runtime::{
	testing::TestXt,
	traits::{One, UniqueSaturatedInto},
};
use support::Price;
use xcm::latest::prelude::*;
use xcm::prelude::GeneralKey;
use xcm_executor::Assets;

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

const INFO2: DispatchInfo = DispatchInfo {
	weight: 100,
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
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			<Runtime as Config>::TreasuryAccount::get(),
			ACA,
			// (<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
		));
		Pallet::<Runtime>::on_runtime_upgrade();

		// the fee calculation include ED, so even user has enough asset, it'll failed.
		// make sure the balance is gt bootstrap_balance, so that treasury pool failed transfer.
		let fee = 5000 * 2 + 1000; // len * byte + weight
		assert_ok!(Currencies::update_balance(
			Origin::root(),
			ALICE,
			ACA,
			fee.unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), fee);
		assert_noop!(
			ChargeTransactionPayment::<Runtime>::from(0).validate(&ALICE, CALL, &INFO, 5000),
			TransactionValidityError::Invalid(InvalidTransaction::Payment)
		);

		// fee2 = fee - ED, so native is enough
		let fee2 = 5000 * 2 + 990;
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
					5000
				)
				.unwrap()
				.priority,
			1
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
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

			let fee = 23 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&ALICE, CALL, &INFO, 23)
					.unwrap()
					.priority,
				1
			);
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

			let fee2 = 18 * 2 + 1000; // len * byte + weight
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&ALICE, CALL2, &INFO, 18)
					.unwrap()
					.priority,
				1
			);
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - fee2);
		});
}

#[test]
fn signed_extension_transaction_payment_work() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

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
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

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
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

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
fn refund_tip_according_to_actual_when_post_dispatch_and_native_currency_is_enough() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			// tip = 0
			let fee = 23 * 2 + 1000; // len * byte + weight
			let pre = ChargeTransactionPayment::<Runtime>::from(0)
				.pre_dispatch(&ALICE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee);

			let refund = 200; // 1000 - 800
			assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(pre, &INFO, &POST_INFO, 23, &Ok(())).is_ok());
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee + refund);

			// tip = 1000
			let fee = 23 * 2 + 1000; // len * byte + weight
			let tip = 1000;
			let pre = ChargeTransactionPayment::<Runtime>::from(tip)
				.pre_dispatch(&CHARLIE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &CHARLIE), 100000 - fee - tip);

			let refund_fee = 200; // 1000 - 800
			let refund_tip = 200; // 1000 - 800
			assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(pre, &INFO, &POST_INFO, 23, &Ok(())).is_ok());
			assert_eq!(
				Currencies::free_balance(ACA, &CHARLIE),
				100000 - fee - tip + refund_fee + refund_tip
			);
		});
}

#[test]
fn refund_should_not_works() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let tip = 1000;
			let fee = 23 * 2 + 1000; // len * byte + weight
			let pre = ChargeTransactionPayment::<Runtime>::from(tip)
				.pre_dispatch(&ALICE, CALL, &INFO, 23)
				.unwrap();
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - tip);

			// actual_weight > weight
			const POST_INFO: PostDispatchInfo = PostDispatchInfo {
				actual_weight: Some(INFO.weight + 1),
				pays_fee: Pays::Yes,
			};

			assert!(ChargeTransactionPayment::<Runtime>::post_dispatch(pre, &INFO, &POST_INFO, 23, &Ok(())).is_ok());
			assert_eq!(Currencies::free_balance(ACA, &ALICE), 100000 - fee - tip);
		});
}

#[test]
fn charges_fee_when_validate_and_native_is_not_enough() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let init_balance = <Runtime as Config>::InitialBootstrapBalanceForFeePool::get();
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(init_balance * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();
			let dot_fee_account = TreasuryAccounts::<Runtime>::get(AUSD);

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
			assert_ok!(<Currencies as MultiCurrency<_>>::transfer(AUSD, &ALICE, &BOB, 4000));

			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(Currencies::total_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 4000);

			// total balance is lt ED, will swap fee and ED
			let fee = 50 * 2 + 100; // len * byte + weight
			let expect_priority = ChargeTransactionPayment::<Runtime>::get_priority(50, &INFO2, fee);
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO2, 50)
					.unwrap()
					.priority,
				expect_priority
			);

			assert_eq!(Currencies::total_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 1900);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(Currencies::free_balance(ACA, &dot_fee_account), init_balance - fee - 10);
			assert_eq!(Currencies::free_balance(AUSD, &dot_fee_account), (fee + 10) * 10);

			// total balance is gte ED, but cannot keep alive after charge,
			// will swap extra gap to keep alive
			let fee_2 = 10 * 2 + 100; // len * byte + weight
			let expect_priority = ChargeTransactionPayment::<Runtime>::get_priority(10, &INFO2, fee_2);
			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO2, 10)
					.unwrap()
					.priority,
				expect_priority
			);
			assert_eq!(Currencies::total_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(ACA, &BOB), 10);
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 700);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(
				Currencies::free_balance(ACA, &dot_fee_account),
				init_balance - fee - 10 - fee_2
			);
			assert_eq!(
				Currencies::free_balance(AUSD, &dot_fee_account),
				(fee + 10 + fee_2) * 10
			);
		});
}

#[test]
fn charges_fee_failed_by_slippage_limit() {
	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

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

			// pool is enough, but slippage limit the swap
			MockPriceSource::set_relative_price(Some(Price::saturating_from_rational(252, 4020)));
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
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				<Runtime as Config>::TreasuryAccount::get(),
				ACA,
				(<Runtime as Config>::InitialBootstrapBalanceForFeePool::get() * 100).unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

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
			assert_ok!(<Currencies as MultiCurrency<_>>::transfer(DOT, &ALICE, &BOB, 300));
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(ACA, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(AUSD, &BOB), 0);
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(DOT, &BOB), 300);

			assert_eq!(
				ChargeTransactionPayment::<Runtime>::from(0)
					.validate(&BOB, CALL2, &INFO, 500)
					.unwrap()
					.priority,
				1
			);

			assert_eq!(Currencies::free_balance(ACA, &BOB), Currencies::minimum_balance(ACA));
			assert_eq!(Currencies::free_balance(AUSD, &BOB), 0);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 300 - 201);
			assert_eq!(DEXModule::get_liquidity_pool(ACA, AUSD), (10000, 1000));
			assert_eq!(DEXModule::get_liquidity_pool(DOT, AUSD), (100, 1000));
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
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;

			assert_eq!(priority, 60);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, CALL, &normal, len)
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
				.validate(&ALICE, CALL, &op, len)
				.unwrap()
				.priority;
			// final_fee = base_fee + len_fee + adjusted_weight_fee + tip = 0 + 20 + 100 + 5 = 125
			// priority = final_fee * fee_multiplier * max_tx_per_block + (tip + 1) * max_tx_per_block
			//          = 125 * 5 * 10 + 60 = 6310
			assert_eq!(priority, 6310);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, CALL, &op, len)
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
				.validate(&ALICE, CALL, &normal, len)
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
				.validate(&ALICE, CALL, &op, len)
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
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 0);

			let priority = ChargeTransactionPayment::<Runtime>(tip - 2)
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 0);

			let priority = ChargeTransactionPayment::<Runtime>(tip - 1)
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(tip)
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip - 2)
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 10);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip - 1)
				.validate(&ALICE, CALL, &normal, len)
				.unwrap()
				.priority;
			// max_tx_per_block = 10
			assert_eq!(priority, 20);

			let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
				.validate(&ALICE, CALL, &normal, len)
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
                .validate(&ALICE, CALL, &normal, len)
                .unwrap()
                .priority;
            // max_tx_per_block = 10
            assert_eq!(priority, 10_000);

            let priority = ChargeTransactionPayment::<Runtime>(2 * tip)
                .validate(&ALICE, CALL, &normal, len)
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
			return Some(Token(KSM));
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
fn period_rate_buy_refund_weight_works() {
	use frame_support::parameter_types;
	parameter_types! {
		pub const KarPerSecond: u128 = 8_000_000_000_000;
	}
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			Pallet::<Runtime>::on_runtime_upgrade();
			let mut trader = PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecond, ()>::new();
			let mock_weight: Weight = 200_000_000;

			// KSM/KAR=1/50, rate=0.02, amount=rate*kar_per_second*weight, amount=8*weight*rate
			let asset: MultiAsset = (Parent, 35_000_000).into();
			let expect_asset: MultiAsset = (Parent, 3_000_000).into();
			let assets: Assets = asset.into();
			let unused = trader.buy_weight(mock_weight, assets);
			assert_eq!(unused.unwrap(), expect_asset.into());
			assert_eq!(trader.asset_location.is_some(), true);
			assert_eq!(trader.amount, 32_000_000);

			let refund_weight: Weight = 50_000_000;
			let expect_refund: MultiAsset = (Parent, 8_000_000).into();

			let refund = trader.refund_weight(refund_weight);
			assert_eq!(refund.unwrap(), expect_refund);
			assert_eq!(trader.amount, 24_000_000);

			// DOT/KAR=1/10, rate=0.1
			let asset: MultiAsset = ((0, X1(GeneralKey(DOT.encode()))), 170_000_000).into();
			let assets: Assets = asset.into();
			let mut trader = PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecond, ()>::new();
			let unused = trader.buy_weight(mock_weight, assets);
			let expect_asset: MultiAsset = ((0, X1(GeneralKey(DOT.encode()))), 10_000_000).into();
			assert_eq!(unused.unwrap(), expect_asset.into());
			assert_eq!(trader.amount, 160_000_000);

			// AUSD/KAR=10, rate=10
			let asset: MultiAsset = ((0, X1(GeneralKey(AUSD.encode()))), 17_000_000_000).into();
			let assets: Assets = asset.into();
			let mut trader = PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecond, ()>::new();
			let unused = trader.buy_weight(mock_weight, assets);
			let expect_asset: MultiAsset = ((0, X1(GeneralKey(AUSD.encode()))), 1_000_000_000).into();
			assert_eq!(unused.unwrap(), expect_asset.into());
			assert_eq!(trader.amount, 16_000_000_000);
		});
}

#[test]
fn treasury_basic_setup_works() {
	ExtBuilder::default().build().execute_with(|| {
		let treasury_account = <Runtime as Config>::TreasuryAccount::get();
		let fee_account = TreasuryAccounts::<Runtime>::get(KSM);
		let expect_initial_balance = <Runtime as Config>::InitialBootstrapBalanceForFeePool::get();
		let amount = (expect_initial_balance * 100) as u128;

		assert_eq!(Currencies::free_balance(ACA, &fee_account), 0);
		assert_eq!(FeeRateOfToken::<Runtime>::get(KSM), None);

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			treasury_account.clone(),
			ACA,
			amount.unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(ACA, &treasury_account), amount);

		// to the runtime upgrade, the treasury account transfer balance to fee pool balance
		Pallet::<Runtime>::on_runtime_upgrade();

		let fee_account = TreasuryAccounts::<Runtime>::get(KSM);
		assert_eq!(Currencies::free_balance(ACA, &fee_account), expect_initial_balance);
		assert_eq!(
			FeeRateOfToken::<Runtime>::get(KSM).unwrap(),
			Ratio::saturating_from_rational(2, 100)
		);

		let _ = Pallet::<Runtime>::set_trigger_threshold(Origin::signed(ALICE), KSM, 500);
		assert_eq!(SwapBalanceThreshold::<Runtime>::get(KSM), 500);
	});
}

#[test]
fn swap_from_treasury_not_enough_currency() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			let treasury_account = <Runtime as Config>::TreasuryAccount::get();
			let expect_initial_balance = <Runtime as Config>::InitialBootstrapBalanceForFeePool::get();
			let amount = (expect_initial_balance * 100) as u128;
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				treasury_account.clone(),
				ACA,
				amount.unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

			let dot_fee_account = TreasuryAccounts::<Runtime>::get(DOT);
			let usd_fee_account = TreasuryAccounts::<Runtime>::get(AUSD);

			assert_eq!(Currencies::free_balance(ACA, &dot_fee_account), expect_initial_balance);
			assert_eq!(Currencies::free_balance(ACA, &usd_fee_account), expect_initial_balance);

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
			let result = Pallet::<Runtime>::swap_from_treasury(&BOB, 1100, DOT);
			assert_eq!(result.err().unwrap(), DispatchError::Token(TokenError::BelowMinimum));
			// 11 ACA equals to 110 AUSD, but Bob only has 100 AUSD
			let result = Pallet::<Runtime>::swap_from_treasury(&BOB, 11, AUSD);
			assert_eq!(result.err().unwrap(), DispatchError::Token(TokenError::BelowMinimum));
		});
}

#[test]
fn swap_from_treasury_with_enough_balance() {
	ExtBuilder::default()
		.base_weight(100)
		.byte_fee(10)
		.build()
		.execute_with(|| {
			let treasury_account = <Runtime as Config>::TreasuryAccount::get();
			let expect_initial_balance = <Runtime as Config>::InitialBootstrapBalanceForFeePool::get();
			let amount = (expect_initial_balance * 100) as u128;
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				treasury_account.clone(),
				ACA,
				amount.unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();

			let dot_fee_account = TreasuryAccounts::<Runtime>::get(DOT);
			let usd_fee_account = TreasuryAccounts::<Runtime>::get(AUSD);

			// 1 DOT = 1 ACA, swap 500 ACA with 50 DOT
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
			let expect_treasury_aca = (expect_initial_balance - fee) as u128; // 500 ACA
			let expect_user_aca = fee; // 500 ACA

			let _ = Pallet::<Runtime>::swap_from_treasury(&BOB, fee, DOT);
			assert_eq!(expect_user_dot, Currencies::free_balance(DOT, &BOB));
			assert_eq!(expect_treasury_dot, Currencies::free_balance(DOT, &dot_fee_account));
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
			assert_eq!(0, Currencies::free_balance(AUSD, &usd_fee_account));
			let fee = balance; // 500 ACA
			let expect_treasury_ausd = (balance * 10) as u128; // 5000 AUSD
			let expect_user_ausd = balance; // (balance * 11) - (balance * 10) = balance = 500 AUSD
			let expect_treasury_aca = expect_initial_balance - fee; // 1000 ACA - 500 ACA
			let expect_user_aca = expect_user_aca + fee; // 500 ACA

			let _ = Pallet::<Runtime>::swap_from_treasury(&BOB, fee, AUSD);
			assert_eq!(expect_user_ausd, Currencies::free_balance(AUSD, &BOB));
			assert_eq!(expect_treasury_ausd, Currencies::free_balance(AUSD, &usd_fee_account));
			assert_eq!(expect_user_aca, Currencies::free_balance(ACA, &BOB));
			assert_eq!(expect_treasury_aca, Currencies::free_balance(ACA, &usd_fee_account));
		});
}

#[test]
fn swap_from_treasury_and_dex_update_rate() {
	use frame_support::parameter_types;
	parameter_types! {
		pub const KarPerSecond: u128 = 8_000_000_000_000;
	}

	ExtBuilder::default()
		.one_hundred_thousand_for_alice_n_charlie()
		.build()
		.execute_with(|| {
			let treasury_account = <Runtime as Config>::TreasuryAccount::get();
			let expect_initial_balance = <Runtime as Config>::InitialBootstrapBalanceForFeePool::get();
			let amount = (expect_initial_balance * 100) as u128;
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				treasury_account.clone(),
				ACA,
				amount.unique_saturated_into(),
			));
			Pallet::<Runtime>::on_runtime_upgrade();
			let dot_fee_account = TreasuryAccounts::<Runtime>::get(DOT);

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
			Pallet::<Runtime>::swap_from_treasury(&BOB, balance, DOT).unwrap();
			// Bob withdraw 80 DOT(remain 720), and deposit 800 ACA
			assert_eq!(balance - fee_dot, Currencies::free_balance(DOT, &BOB));
			assert_eq!(balance, Currencies::free_balance(ACA, &BOB));
			// Treasury deposit 80 DOT, and withdraw 800 ACA(remain 9200)
			assert_eq!(fee_dot, Currencies::free_balance(DOT, &dot_fee_account));
			assert_eq!(
				expect_initial_balance - balance,
				Currencies::free_balance(ACA, &dot_fee_account)
			);

			// Set threshold(init-500) gt treasury balance(init-800), trigger swap from dex.
			let swap_balance_threshold = (expect_initial_balance - 500) as u128;
			Pallet::<Runtime>::set_trigger_threshold(Origin::signed(ALICE), DOT, swap_balance_threshold).unwrap();
			Pallet::<Runtime>::set_trigger_threshold(Origin::signed(ALICE), AUSD, swap_balance_threshold).unwrap();

			// before swap from dex, need add liquidity pool
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

			// swap 80 DOT out 3074 ACA
			let trading_path = Pallet::<Runtime>::get_trading_path_by_currency(&ALICE, DOT).unwrap();
			let swap_native = module_dex::Pallet::<Runtime>::get_swap_target_amount(
				&trading_path,
				Currencies::free_balance(DOT, &dot_fee_account),
			)
			.unwrap();
			assert_eq!(3074, swap_native);
			// calculate the new balance of ACA = 3074+(init-800)=3074+9200=12270
			let current_native_balance =
				(swap_native + Currencies::free_balance(ACA, &dot_fee_account)).saturated_into::<u128>();
			let base_native: u128 = current_native_balance / 10;
			assert_eq!(1227, base_native);
			// compare to the old balance:10000 and rate:1/10, the new rate is (12270/10000)*(1/10)=0.1227
			let rate = Ratio::saturating_from_rational(base_native, expect_initial_balance);
			assert_eq!(Ratio::saturating_from_rational(1227, 10000), rate);

			// the treasury has 9200 ACA, 80 DOT, use 80 DOT to swap out some ACA
			let balance2 = 300 as u128;
			let _ = Pallet::<Runtime>::swap_from_treasury(&BOB, balance2, DOT);
			assert_eq!(FeeRateOfToken::<Runtime>::get(DOT).unwrap(), rate);

			// Bob swap 98 DOT(use the new updated rate) to get 300 ACA
			let exchange = rate.saturating_mul_int(balance2);
			assert_eq!(balance - fee_dot - exchange, Currencies::free_balance(DOT, &BOB));
			assert_eq!(balance + balance2, Currencies::free_balance(ACA, &BOB));
			// When swap from dex finished, the treasury DOT got 0(no ED), as treasury use DOT to swap ACA.
			// Only the newly exchange DOT from user to treasury.
			assert_eq!(exchange, Currencies::free_balance(DOT, &dot_fee_account));
			assert_eq!(
				current_native_balance - balance2,
				Currencies::free_balance(ACA, &dot_fee_account)
			);

			// rate=0.1227, amount=8*rate*weight=8*0.1227*200_000_000
			let mut trader = PeriodUpdatedRateOfFungible::<Runtime, CurrencyIdConvert, KarPerSecond, ()>::new();
			let mock_weight: Weight = 200_000_000;
			let asset: u128 = 500_000_000;

			let expect_amount: u128 = 8 * base_native * 20_000;
			let expect_unused: u128 = asset - expect_amount;

			let asset: MultiAsset = ((0, X1(GeneralKey(DOT.encode()))), asset).into();
			let expect_asset: MultiAsset = ((0, X1(GeneralKey(DOT.encode()))), expect_unused).into();
			let assets: Assets = asset.into();
			let unused = trader.buy_weight(mock_weight, assets);
			assert_eq!(unused.unwrap(), expect_asset.into());
			assert_eq!(expect_amount, trader.amount);
		});
}
