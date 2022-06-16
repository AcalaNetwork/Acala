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

use crate::setup::*;
use frame_support::weights::{DispatchClass, DispatchInfo, Pays, Weight};
use sp_runtime::{
	traits::{AccountIdConversion, SignedExtension, UniqueSaturatedInto},
	MultiAddress, Percent,
};
use xcm_executor::{traits::*, Assets, Config};

fn fee_pool_size() -> Balance {
	5 * dollar(NATIVE_CURRENCY)
}

fn init_charge_fee_pool(currency_id: CurrencyId, path: Vec<CurrencyId>) -> DispatchResult {
	TransactionPayment::enable_charge_fee_pool(
		Origin::root(),
		currency_id,
		path,
		fee_pool_size(),
		Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY)),
	)
}

fn init_charge_fee_pool_relay() -> DispatchResult {
	init_charge_fee_pool(RELAY_CHAIN_CURRENCY, vec![RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY])
}
fn init_charge_fee_pool_usd() -> DispatchResult {
	init_charge_fee_pool(USD_CURRENCY, vec![USD_CURRENCY, RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY])
}
fn init_charge_fee_pool_liquid() -> DispatchResult {
	init_charge_fee_pool(
		LIQUID_CURRENCY,
		vec![LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY],
	)
}

#[cfg(feature = "with-acala-runtime")]
fn add_liquidity_for_lcdot() {
	assert_ok!(Dex::add_liquidity(
		Origin::signed(AccountId::from(ALICE)),
		USD_CURRENCY,
		NATIVE_CURRENCY,
		100 * dollar(USD_CURRENCY),
		10000 * dollar(NATIVE_CURRENCY),
		0,
		false
	));
	assert_ok!(Dex::add_liquidity(
		Origin::signed(AccountId::from(ALICE)),
		RELAY_CHAIN_CURRENCY,
		LCDOT,
		100 * dollar(RELAY_CHAIN_CURRENCY),
		100 * dollar(RELAY_CHAIN_CURRENCY),
		0,
		false
	));
	assert_ok!(Dex::add_liquidity(
		Origin::signed(AccountId::from(ALICE)),
		USD_CURRENCY,
		LCDOT,
		100 * dollar(USD_CURRENCY),
		100 * dollar(RELAY_CHAIN_CURRENCY),
		0,
		false
	));
}

#[test]
fn initial_charge_fee_pool_works() {
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				NATIVE_CURRENCY,
				100000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(ALICE), USD_CURRENCY, 2000 * dollar(USD_CURRENCY)),
			#[cfg(feature = "with-acala-runtime")]
			(AccountId::from(ALICE), LCDOT, 2000 * dollar(RELAY_CHAIN_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let treasury_account = TreasuryAccount::get();
			let fee_account1: AccountId =
				TransactionPaymentPalletId::get().into_sub_account_truncating(RELAY_CHAIN_CURRENCY);
			// FeePoolSize set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
			let ed = NativeTokenExistentialDeposit::get();
			let pool_size = fee_pool_size();

			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &treasury_account), ed);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &fee_account1), 0);

			// treasury account: KAR=151*KAR_ED, and foreign asset=the ED of foreign asset
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				NATIVE_CURRENCY,
				pool_size.saturating_mul(3).unique_saturated_into(),
			));
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &treasury_account),
				ed + pool_size * 3
			);
			vec![RELAY_CHAIN_CURRENCY, USD_CURRENCY, LIQUID_CURRENCY]
				.iter()
				.for_each(|token| {
					let ed = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone()))
						.unique_saturated_into();
					assert_ok!(Currencies::update_balance(
						Origin::root(),
						MultiAddress::Id(treasury_account.clone()),
						token.clone(),
						ed,
					));
				});

			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10000 * dollar(NATIVE_CURRENCY),
				0,
				false
			));
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				1000 * dollar(USD_CURRENCY),
				0,
				false
			));

			#[cfg(feature = "with-acala-runtime")]
			add_liquidity_for_lcdot();

			assert_ok!(init_charge_fee_pool_relay());
			assert_ok!(init_charge_fee_pool_usd());
			// balance lt ED
			assert_noop!(
				TransactionPayment::enable_charge_fee_pool(
					Origin::root(),
					LIQUID_CURRENCY,
					vec![LIQUID_CURRENCY, RELAY_CHAIN_CURRENCY, NATIVE_CURRENCY],
					NativeTokenExistentialDeposit::get() - 1,
					Ratio::saturating_from_rational(35, 100).saturating_mul_int(dollar(NATIVE_CURRENCY))
				),
				module_transaction_payment::Error::<Runtime>::InvalidBalance
			);
			assert_noop!(
				init_charge_fee_pool_liquid(),
				module_transaction_payment::Error::<Runtime>::DexNotAvailable
			);
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &treasury_account),
				ed + pool_size
			);
			vec![RELAY_CHAIN_CURRENCY, USD_CURRENCY].iter().for_each(|token| {
				let ed =
					(<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
				assert_eq!(
					Currencies::free_balance(
						NATIVE_CURRENCY,
						&TransactionPaymentPalletId::get().into_sub_account_truncating(token.clone())
					),
					pool_size
				);
				assert_eq!(
					Currencies::free_balance(
						token.clone(),
						&TransactionPaymentPalletId::get().into_sub_account_truncating(token.clone())
					),
					ed
				);
			});
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
fn token_per_second_works() {
	#[cfg(feature = "with-karura-runtime")]
	{
		let kar_per_second = karura_runtime::kar_per_second();
		assert_eq!(11_655_000_000_000, kar_per_second);

		let ksm_per_second = karura_runtime::ksm_per_second();
		assert_eq!(233_100_000_000, ksm_per_second);
	}

	#[cfg(feature = "with-acala-runtime")]
	{
		let aca_per_second = acala_runtime::aca_per_second();
		assert_eq!(11_655_000_000_000, aca_per_second);

		let dot_per_second = acala_runtime::dot_per_second();
		assert_eq!(2_331_000_000, dot_per_second);
	}
}

#[test]
fn trader_works() {
	// 4 instructions, each instruction cost 200_000_000
	let mut message = Xcm(vec![
		ReserveAssetDeposited((Parent, 100).into()),
		ClearOrigin,
		BuyExecution {
			fees: (Parent, 100).into(),
			weight_limit: Limited(100),
		},
		DepositAsset {
			assets: All.into(),
			max_assets: 1,
			beneficiary: Here.into(),
		},
	]);
	#[cfg(feature = "with-mandala-runtime")]
	let expect_weight: Weight = 4_000_000;
	#[cfg(feature = "with-karura-runtime")]
	let expect_weight: Weight = 800_000_000;
	#[cfg(feature = "with-acala-runtime")]
	let expect_weight: Weight = 800_000_000;

	#[cfg(feature = "with-mandala-runtime")]
	let base_per_second = mandala_runtime::aca_per_second();
	#[cfg(feature = "with-karura-runtime")]
	let base_per_second = karura_runtime::kar_per_second();
	#[cfg(feature = "with-acala-runtime")]
	let base_per_second = acala_runtime::aca_per_second();

	let xcm_weight: Weight = <XcmConfig as Config>::Weigher::weight(&mut message).unwrap();
	assert_eq!(xcm_weight, expect_weight);

	let total_balance: Balance = 10_00_000_000;
	let asset: MultiAsset = (Parent, total_balance).into();
	let assets: Assets = asset.into();

	// ksm_per_second/kar_per_second=1/50
	// v0.9.22: kar_per_second=8KAR, ksm_per_second=0.16KSM,
	//          fee=0.16*weight=0.16*800_000_000=128_000_000
	// v0.9.23: kar_per_second=11.655KAR, ksm_per_second=0.2331KSM
	//          fee=0.2331*weight=186_480_000
	#[cfg(feature = "with-mandala-runtime")]
	let expect_unspent: MultiAsset = (Parent, 999_533_800).into(); // 466200
	#[cfg(feature = "with-karura-runtime")]
	let expect_unspent: MultiAsset = (Parent, 813_520_000).into(); // 186480000
	#[cfg(feature = "with-acala-runtime")]
	let expect_unspent: MultiAsset = (Parent, 998_135_200).into(); // 1864800

	// when no runtime upgrade, the newly `TransactionFeePoolTrader` will failed.
	ExtBuilder::default().build().execute_with(|| {
		let mut trader = Trader::new();
		let result_assets = trader.buy_weight(xcm_weight, assets.clone()).unwrap();
		let unspent: Vec<MultiAsset> = result_assets.into();
		assert_eq!(vec![expect_unspent.clone()], unspent);

		let mut period_trader = TransactionFeePoolTrader::new();
		let result_assets = period_trader.buy_weight(xcm_weight, assets.clone());
		assert!(result_assets.is_err());
	});

	// do runtime upgrade
	ExtBuilder::default()
		.balances(vec![
			(
				AccountId::from(ALICE),
				NATIVE_CURRENCY,
				100000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(ALICE), USD_CURRENCY, 2000 * dollar(USD_CURRENCY)),
			#[cfg(feature = "with-acala-runtime")]
			(AccountId::from(ALICE), LCDOT, 2000 * dollar(RELAY_CHAIN_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			let treasury_account = TreasuryAccount::get();
			let fee_account1: AccountId =
				TransactionPaymentPalletId::get().into_sub_account_truncating(RELAY_CHAIN_CURRENCY);
			// FeePoolSize set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
			let ed = NativeTokenExistentialDeposit::get();
			let relay_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(RELAY_CHAIN_CURRENCY);
			let pool_size = fee_pool_size();

			// treasury account: KAR=50*KAR_ED, KSM=KSM_ED, KUSD=KUSD_ED
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				NATIVE_CURRENCY,
				pool_size.unique_saturated_into(),
			));
			assert_eq!(
				Currencies::free_balance(NATIVE_CURRENCY, &treasury_account),
				ed + pool_size
			);
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				RELAY_CHAIN_CURRENCY,
				relay_ed.unique_saturated_into(),
			));

			// runtime upgrade
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10000 * dollar(NATIVE_CURRENCY),
				0,
				false
			));

			#[cfg(feature = "with-acala-runtime")]
			add_liquidity_for_lcdot();

			assert_ok!(init_charge_fee_pool_relay());
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &treasury_account), ed);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &fee_account1), pool_size);
			assert_eq!(Currencies::free_balance(RELAY_CHAIN_CURRENCY, &fee_account1), relay_ed);

			let relay_exchange_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(RELAY_CHAIN_CURRENCY).unwrap();
			let weight_ratio = Ratio::saturating_from_rational(
				expect_weight as u128,
				frame_support::weights::constants::WEIGHT_PER_SECOND as u128,
			);
			let asset_per_second = relay_exchange_rate.saturating_mul_int(base_per_second);
			let spent = weight_ratio.saturating_mul_int(asset_per_second);
			let expect_unspent: MultiAsset = (Parent, total_balance - spent as u128).into();

			// the newly `TransactionFeePoolTrader` works fine as first priority
			let mut period_trader = TransactionFeePoolTrader::new();
			let result_assets = period_trader.buy_weight(xcm_weight, assets);
			let unspent: Vec<MultiAsset> = result_assets.unwrap().into();
			assert_eq!(vec![expect_unspent.clone()], unspent);
		});
}

#[test]
fn charge_transaction_payment_and_threshold_works() {
	let native_ed = NativeTokenExistentialDeposit::get();
	let pool_size = fee_pool_size();
	let relay_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(RELAY_CHAIN_CURRENCY);

	let treasury_account = TreasuryAccount::get();
	let sub_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account_truncating(RELAY_CHAIN_CURRENCY);
	let bob_relay_balance = 100 * dollar(RELAY_CHAIN_CURRENCY);

	ExtBuilder::default()
		.balances(vec![
			// Alice for Dex, Bob for transaction payment
			(
				AccountId::from(ALICE),
				NATIVE_CURRENCY,
				100000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1000 * dollar(RELAY_CHAIN_CURRENCY),
			),
			(AccountId::from(ALICE), USD_CURRENCY, 2000 * dollar(USD_CURRENCY)),
			(AccountId::from(BOB), NATIVE_CURRENCY, native_ed),
			(AccountId::from(BOB), RELAY_CHAIN_CURRENCY, bob_relay_balance),
			#[cfg(feature = "with-acala-runtime")]
			(AccountId::from(ALICE), LCDOT, 2000 * dollar(RELAY_CHAIN_CURRENCY)),
		])
		.build()
		.execute_with(|| {
			// before update, treasury account has native_ed amount of native token
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				NATIVE_CURRENCY,
				pool_size.unique_saturated_into(),
			));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				RELAY_CHAIN_CURRENCY,
				relay_ed.unique_saturated_into(),
			));

			assert_noop!(
				init_charge_fee_pool_relay(),
				module_transaction_payment::Error::<Runtime>::DexNotAvailable
			);
			assert_noop!(
				init_charge_fee_pool_usd(),
				module_transaction_payment::Error::<Runtime>::DexNotAvailable
			);
			assert_noop!(
				init_charge_fee_pool_liquid(),
				module_transaction_payment::Error::<Runtime>::DexNotAvailable
			);
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10000 * dollar(NATIVE_CURRENCY),
				0,
				false
			));

			#[cfg(feature = "with-acala-runtime")]
			add_liquidity_for_lcdot();

			// before init_charge_fee_pool, treasury account has native_ed+pool_size of native token
			assert_ok!(init_charge_fee_pool_relay());
			// init_charge_fee_pool will transfer pool_size to sub_account
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &treasury_account), native_ed);
			assert_eq!(Currencies::free_balance(NATIVE_CURRENCY, &sub_account1), pool_size);
			assert_eq!(Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1), relay_ed);

			let relay_exchange_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(RELAY_CHAIN_CURRENCY).unwrap();

			let threshold: Balance =
				module_transaction_payment::Pallet::<Runtime>::swap_balance_threshold(RELAY_CHAIN_CURRENCY);
			let expect_threshold = Ratio::saturating_from_rational(350, 100).saturating_mul_int(native_ed);
			assert_eq!(threshold, expect_threshold); // 350 000 000 000

			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				NATIVE_CURRENCY,
				100 * dollar(RELAY_CHAIN_CURRENCY),
				10000 * dollar(NATIVE_CURRENCY),
				0,
				false
			));

			let len = 150 as u32;
			let call: &<Runtime as frame_system::Config>::Call = &Call::Currencies(module_currencies::Call::transfer {
				dest: MultiAddress::Id(AccountId::from(BOB)),
				currency_id: USD_CURRENCY,
				amount: 12,
			});
			let info: DispatchInfo = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let fee = module_transaction_payment::Pallet::<Runtime>::compute_fee(len, &info, 0);
			let fee_alternative_surplus_percent: Percent = ALTERNATIVE_SURPLUS;
			let surplus = fee_alternative_surplus_percent.mul_ceil(fee);
			let fee = fee + surplus;

			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					call,
					&info,
					len as usize,
				)
			);
			let balance1 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay1 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);

			assert_ok!(
				<module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					call,
					&info,
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
						call,
						&info,
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
					call,
					&info,
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
					call,
					&info,
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
					call,
					&info,
					len as usize,
				)
			);
			let balance2 = Currencies::free_balance(NATIVE_CURRENCY, &sub_account1);
			let relay2 = Currencies::free_balance(RELAY_CHAIN_CURRENCY, &sub_account1);
			assert_eq!(fee, balance1 - balance2);
			assert_eq!(new_rate.saturating_mul_int(fee), relay2 - relay1);
		});
}
