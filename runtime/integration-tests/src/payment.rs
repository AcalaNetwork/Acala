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

use crate::setup::*;
use crate::stable_asset::enable_stable_asset;
use frame_support::{
	dispatch::{DispatchClass, DispatchInfo, Pays, PostDispatchInfo},
	weights::Weight,
};
use module_support::AggregatedSwapPath;
use sp_runtime::{
	traits::{AccountIdConversion, SignedExtension, UniqueSaturatedInto},
	transaction_validity::{InvalidTransaction, TransactionValidityError},
	MultiAddress, Percent,
};

fn fee_pool_size() -> Balance {
	5 * dollar(NATIVE_CURRENCY)
}

fn init_charge_fee_pool(currency_id: CurrencyId) -> DispatchResult {
	let treasury_account = TreasuryAccount::get();
	let sub_account: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(currency_id.clone());

	let ed = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(currency_id.clone())).unique_saturated_into();
	let fee_pool_size: u128 = fee_pool_size();

	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		MultiAddress::Id(treasury_account.clone()),
		currency_id.clone(),
		ed,
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		MultiAddress::Id(treasury_account.clone()),
		NATIVE_CURRENCY,
		fee_pool_size.unique_saturated_into(),
	));

	// enable fee pool operation will transfer ed of token and pool_size of native token to sub account.
	let native_amount: u128 = Currencies::free_balance(NATIVE_CURRENCY, &treasury_account);
	let token_amount: u128 = Currencies::free_balance(currency_id.clone(), &treasury_account);
	assert_ok!(TransactionPayment::enable_charge_fee_pool(
		RuntimeOrigin::root(),
		currency_id,
		fee_pool_size,
		Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY)),
	));
	assert!(module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(currency_id).is_some());
	let native_amount1: u128 = Currencies::free_balance(NATIVE_CURRENCY, &treasury_account);
	let token_amount1: u128 = Currencies::free_balance(currency_id.clone(), &treasury_account);
	assert_eq!(native_amount - native_amount1, fee_pool_size);
	assert_eq!(token_amount - token_amount1, ed as u128);
	assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account), fee_pool_size);
	assert_eq!(Currencies::free_balance(currency_id.clone(), &sub_account), ed as u128);
	Ok(())
}

fn add_liquidity(token1: CurrencyId, token2: CurrencyId, amount1: Balance, amount2: Balance) -> DispatchResult {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		MultiAddress::Id(AccountId::from(ALICE)),
		token1,
		amount1.unique_saturated_into(),
	));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		MultiAddress::Id(AccountId::from(ALICE)),
		token2,
		amount2.unique_saturated_into(),
	));
	Dex::add_liquidity(
		RuntimeOrigin::signed(AccountId::from(ALICE)),
		token1,
		token2,
		amount1.unique_saturated_into(),
		amount2.unique_saturated_into(),
		0,
		false,
	)
}

const CALL: <Runtime as frame_system::Config>::RuntimeCall =
	RuntimeCall::Currencies(module_currencies::Call::transfer {
		dest: MultiAddress::Id(AccountId::new([2u8; 32])),
		currency_id: USD_CURRENCY,
		amount: 12,
	});
pub const INFO: DispatchInfo = DispatchInfo {
	weight: Weight::from_parts(100, 0),
	class: DispatchClass::Normal,
	pays_fee: Pays::Yes,
};
pub const POST_INFO: PostDispatchInfo = PostDispatchInfo {
	actual_weight: Some(Weight::from_parts(80, 0)),
	pays_fee: Pays::Yes,
};

pub fn with_fee_currency_call(currency_id: CurrencyId) -> <Runtime as module_transaction_payment::Config>::RuntimeCall {
	let fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall =
		RuntimeCall::TransactionPayment(module_transaction_payment::Call::with_fee_currency {
			currency_id,
			call: Box::new(CALL),
		});
	fee_call
}

pub fn with_fee_path_call(
	fee_swap_path: Vec<CurrencyId>,
) -> <Runtime as module_transaction_payment::Config>::RuntimeCall {
	let fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall =
		RuntimeCall::TransactionPayment(module_transaction_payment::Call::with_fee_path {
			fee_swap_path,
			call: Box::new(CALL),
		});
	fee_call
}

pub fn with_fee_aggregated_path_call(
	fee_aggregated_path: Vec<AggregatedSwapPath<CurrencyId>>,
) -> <Runtime as module_transaction_payment::Config>::RuntimeCall {
	let fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall =
		RuntimeCall::TransactionPayment(module_transaction_payment::Call::with_fee_aggregated_path {
			fee_aggregated_path,
			call: Box::new(CALL),
		});
	fee_call
}

#[test]
fn initial_charge_fee_pool_works() {
	ExtBuilder::default().build().execute_with(|| {
		let treasury_account = TreasuryAccount::get();
		// FeePoolSize set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
		let ed = NativeTokenExistentialDeposit::get();
		let pool_size = fee_pool_size();
		assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &treasury_account), ed);

		assert_ok!(add_liquidity(
			RELAY_CHAIN_CURRENCY,
			NATIVE_CURRENCY,
			100 * dollar(RELAY_CHAIN_CURRENCY),
			10000 * dollar(NATIVE_CURRENCY)
		));
		assert_ok!(add_liquidity(
			RELAY_CHAIN_CURRENCY,
			USD_CURRENCY,
			100 * dollar(RELAY_CHAIN_CURRENCY),
			1000 * dollar(USD_CURRENCY)
		));

		assert_ok!(init_charge_fee_pool(RELAY_CHAIN_CURRENCY));
		assert_ok!(init_charge_fee_pool(USD_CURRENCY));

		// fee_pool_size lt ED can't enable fee pool
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			MultiAddress::Id(treasury_account.clone()),
			NATIVE_CURRENCY,
			pool_size.unique_saturated_into(),
		));
		let led = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(LIQUID_CURRENCY)).unique_saturated_into();
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			MultiAddress::Id(treasury_account.clone()),
			LIQUID_CURRENCY,
			led,
		));
		assert_noop!(
			TransactionPayment::enable_charge_fee_pool(
				RuntimeOrigin::root(),
				LIQUID_CURRENCY,
				NativeTokenExistentialDeposit::get() - 1,
				Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY))
			),
			module_transaction_payment::Error::<Runtime>::InvalidBalance
		);
		assert_noop!(
			TransactionPayment::enable_charge_fee_pool(
				RuntimeOrigin::root(),
				LIQUID_CURRENCY,
				pool_size,
				Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY))
			),
			module_transaction_payment::Error::<Runtime>::DexNotAvailable
		);
		assert_eq!(
			Currencies::free_balance(NATIVE_CURRENCY, &treasury_account),
			ed + pool_size
		);
		assert_eq!(
			Currencies::free_balance(
				NATIVE_CURRENCY,
				&TransactionPaymentPalletId::get().into_sub_account_truncating(LIQUID_CURRENCY)
			),
			0
		);
		assert_eq!(
			Currencies::free_balance(
				LIQUID_CURRENCY,
				&TransactionPaymentPalletId::get().into_sub_account_truncating(LIQUID_CURRENCY)
			),
			0
		);
	});
}

#[test]
fn charge_transaction_payment_and_threshold_works() {
	let native_ed = NativeTokenExistentialDeposit::get();
	let pool_size = fee_pool_size();
	let relay_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(RELAY_CHAIN_CURRENCY);

	let sub_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(RELAY_CHAIN_CURRENCY);
	let bob_relay_balance = 100 * dollar(RELAY_CHAIN_CURRENCY);

	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(BOB), NATIVE_CURRENCY, native_ed),
			(AccountId::from(BOB), RELAY_CHAIN_CURRENCY, bob_relay_balance),
		])
		.build()
		.execute_with(|| {
			for token in vec![RELAY_CHAIN_CURRENCY, USD_CURRENCY, LIQUID_CURRENCY] {
				assert_noop!(
					TransactionPayment::enable_charge_fee_pool(
						RuntimeOrigin::root(),
						token,
						fee_pool_size(),
						Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY)),
					),
					module_transaction_payment::Error::<Runtime>::DexNotAvailable
				);
			}
			assert_ok!(add_liquidity(
				RELAY_CHAIN_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10000 * dollar(NATIVE_CURRENCY)
			));

			// before init_charge_fee_pool, treasury account has native_ed+pool_size of native token
			assert_ok!(init_charge_fee_pool(RELAY_CHAIN_CURRENCY));

			let relay_exchange_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(RELAY_CHAIN_CURRENCY).unwrap();

			let threshold: Balance =
				module_transaction_payment::Pallet::<Runtime>::swap_balance_threshold(RELAY_CHAIN_CURRENCY);
			let expect_threshold = Ratio::saturating_from_rational(350, 100).saturating_mul_int(native_ed);
			assert_eq!(threshold, expect_threshold); // 350 000 000 000

			let len = 150 as u32;
			let fee = module_transaction_payment::Pallet::<Runtime>::compute_fee(len, &INFO, 0);
			let fee_alternative_surplus_percent: Percent = ALTERNATIVE_SURPLUS;
			let surplus = fee_alternative_surplus_percent.mul_ceil(fee);
			let fee = fee + surplus;

			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					len as usize,
				)
			);
			let balance1 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay1 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);

			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					len as usize,
				)
			);
			let balance2 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay2 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);
			assert_eq!(fee, balance1 - balance2);
			assert_eq!(relay_exchange_rate.saturating_mul_int(fee), relay2 - relay1);

			for i in 0..38 {
				assert_ok!(
					<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
						&AccountId::from(BOB),
						&CALL,
						&INFO,
						len as usize,
					)
				);
				assert_eq!(
					pool_size - fee * (i + 3),
					Currencies::free_balance(NATIVE_CURRENCY, &sub_account1)
				);
			}
			let balance1 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay1 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);

			// set swap balance trigger, next tx will trigger swap from dex
			module_transaction_payment::SwapBalanceThreshold::<Runtime>::insert(
				RELAY_CHAIN_CURRENCY,
				pool_size - fee * 40,
			);

			// 5 000 000 000 000
			//   350 000 000 000
			// before execute this tx, the balance of fee pool is equal to threshold,
			// so it wouldn't trigger swap from dex.
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					len as usize,
				)
			);
			let balance2 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay2 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);
			assert_eq!(fee, balance1 - balance2);
			assert_eq!(relay_exchange_rate.saturating_mul_int(fee), relay2 - relay1);

			// this tx cause swap from dex, but the fee calculation still use the old rate.
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					len as usize,
				)
			);
			let balance1 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay1 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);
			assert_eq!(relay_ed + relay_exchange_rate.saturating_mul_int(fee), relay1);
			assert!(balance1 > balance2);
			assert!(relay2 > relay1);

			// next tx use the new rate to calculate the fee to be transfer.
			let new_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(RELAY_CHAIN_CURRENCY).unwrap();

			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					len as usize,
				)
			);
			let balance2 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay2 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);
			assert_eq!(fee, balance1 - balance2);
			assert_eq!(new_rate.saturating_mul_int(fee), relay2 - relay1);
		});
}

#[test]
fn with_fee_currency_call_works() {
	let amount = with_fee_call_works(with_fee_currency_call(LIQUID_CURRENCY), false);
	#[cfg(feature = "with-mandala-runtime")]
	assert_debug_snapshot!(amount, @"12701470465");
	#[cfg(feature = "with-karura-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
	#[cfg(feature = "with-acala-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
}

#[test]
fn with_fee_path_call_works() {
	let amount = with_fee_call_works(
		with_fee_path_call(vec![LIQUID_CURRENCY, USD_CURRENCY, NATIVE_CURRENCY]),
		false,
	);
	#[cfg(feature = "with-mandala-runtime")]
	assert_debug_snapshot!(amount, @"12701470465");
	#[cfg(feature = "with-karura-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
	#[cfg(feature = "with-acala-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
}

#[test]
fn with_fee_aggregated_path_call_works() {
	let aggregated_path = vec![
		AggregatedSwapPath::<CurrencyId>::Taiga(0, 0, 1),
		AggregatedSwapPath::<CurrencyId>::Dex(vec![LIQUID_CURRENCY, USD_CURRENCY, NATIVE_CURRENCY]),
	];
	let amount = with_fee_call_works(with_fee_aggregated_path_call(aggregated_path), true);
	#[cfg(feature = "with-mandala-runtime")]
	assert_debug_snapshot!(amount, @"12701470465");
	#[cfg(feature = "with-karura-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
	#[cfg(feature = "with-acala-runtime")]
	assert_debug_snapshot!(amount, @"12726949844");
}

fn with_fee_call_works(
	with_fee_call: <Runtime as module_transaction_payment::Config>::RuntimeCall,
	is_aggregated_call: bool,
) -> Balance {
	let init_amount = 100 * dollar(LIQUID_CURRENCY);
	let ausd_acc: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(USD_CURRENCY);
	return ExtBuilder::default()
		.balances(vec![
			// ALICE for stable asset, BOB and CHARLIE for transaction payment
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				2000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(ALICE), LIQUID_CURRENCY, 2000 * dollar(LIQUID_CURRENCY)),
			(AccountId::from(BOB), LIQUID_CURRENCY, init_amount),
			(AccountId::from(BOB), RELAY_CHAIN_CURRENCY, init_amount),
			(AccountId::from(CHARLIE), USD_CURRENCY, init_amount),
		])
		.build()
		.execute_with(|| {
			if is_aggregated_call {
				enable_stable_asset(
					vec![RELAY_CHAIN_CURRENCY, LIQUID_CURRENCY],
					vec![100 * dollar(RELAY_CHAIN_CURRENCY), 100 * dollar(LIQUID_CURRENCY)],
					None,
				);
			}

			// USD - ACA
			assert_ok!(add_liquidity(
				USD_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(USD_CURRENCY),
				1000 * dollar(NATIVE_CURRENCY)
			));
			assert_ok!(add_liquidity(
				LIQUID_CURRENCY,
				USD_CURRENCY,
				100 * dollar(LIQUID_CURRENCY),
				1000 * dollar(USD_CURRENCY)
			));

			// enable USD as charge fee pool token.
			assert_ok!(init_charge_fee_pool(USD_CURRENCY));

			// un-wrapped call use dex swap only `AlternativeFeeSwapPath` is set, otherwise use fee pool.
			// user don't have USD(which use fee pool), and also don't have native token, then failed.
			assert_noop!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&CALL,
					&INFO,
					50,
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_call,
					&INFO,
					50
				)
			);
			if is_aggregated_call {
				assert!(System::events().iter().any(|r| matches!(
					r.event,
					RuntimeEvent::StableAsset(nutsfinance_stable_asset::Event::TokenSwapped {
						pool_id: 0,
						a: 1000,
						input_asset: RELAY_CHAIN_CURRENCY,
						output_asset: LIQUID_CURRENCY,
						..
					})
				)));
			}
			assert!(System::events().iter().any(|r| matches!(
				r.event,
				// LIQUID_CURRENCY, USD_CURRENCY, NATIVE_CURRENCY
				RuntimeEvent::Dex(module_dex::Event::Swap { .. })
			)));
			// Bob don't have any USD currency.
			assert_noop!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					&with_fee_currency_call(USD_CURRENCY),
					&INFO,
					50
				),
				TransactionValidityError::Invalid(InvalidTransaction::Payment)
			);

			// Charlie have USD currency.
			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment::<Runtime>>::from(0).validate(
					&AccountId::from(CHARLIE),
					&with_fee_currency_call(USD_CURRENCY),
					&INFO,
					50
				)
			);

			let amount = System::events()
				.iter()
				.filter_map(|r| {
					if let RuntimeEvent::Tokens(orml_tokens::Event::Transfer {
						ref currency_id,
						ref from,
						ref to,
						amount,
					}) = r.event
					{
						if *currency_id == USD_CURRENCY && *from == AccountId::from(CHARLIE) && *to == ausd_acc {
							Some(amount)
						} else {
							None
						}
					} else {
						None
					}
				})
				.next()
				.unwrap();

			return amount;
		});
}
