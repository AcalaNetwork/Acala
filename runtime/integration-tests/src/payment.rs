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

use crate::setup::*;
use frame_support::traits::OnRuntimeUpgrade;
use frame_support::weights::{DispatchClass, DispatchInfo, Pays, Weight};
use karura_runtime::{
	FeePoolSize, KarPerSecondAsBased, KaruraTreasuryAccount, KsmPerSecond, NativeTokenExistentialDeposit,
	TransactionPaymentPalletId,
};
use module_transaction_payment::TransactionFeePoolTrader;
use sp_runtime::traits::SignedExtension;
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto},
	MultiAddress,
};
use xcm_builder::FixedRateOfFungible;
use xcm_executor::{traits::*, Assets, Config};

#[cfg(feature = "with-karura-runtime")]
#[test]
fn runtime_upgrade_initial_pool_works() {
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), KAR, 100000 * dollar(KAR)),
			(AccountId::from(ALICE), KSM, 200 * dollar(KSM)),
			(AccountId::from(ALICE), KUSD, 2000 * dollar(KSM)),
		])
		.build()
		.execute_with(|| {
			let treasury_account = KaruraTreasuryAccount::get();
			let fee_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account(KSM);
			// FeePoolSize set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
			let ed = NativeTokenExistentialDeposit::get();
			let pool_size = FeePoolSize::get();

			// upgrade takes no effect
			MockRuntimeUpgrade::on_runtime_upgrade();
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed);
			assert_eq!(Currencies::free_balance(KAR, &fee_account1), 0);

			// treasury account: KAR=151*KAR_ED, and foreign asset=the ED of foreign asset
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				KAR,
				pool_size.saturating_mul(3).unique_saturated_into(),
			));
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed + pool_size * 3);
			vec![KSM, KUSD, LKSM].iter().for_each(|token| {
				let ed =
					(<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
				assert_ok!(Currencies::update_balance(
					Origin::root(),
					MultiAddress::Id(treasury_account.clone()),
					token.clone(),
					ed,
				));
			});

			// the last one failed because balance lt ED
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				KSM,
				KAR,
				100 * dollar(KSM),
				10000 * dollar(KAR),
				0,
				false
			));
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				KSM,
				KUSD,
				100 * dollar(KSM),
				1000 * dollar(KAR),
				0,
				false
			));
			MockRuntimeUpgrade::on_runtime_upgrade();
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed + pool_size);
			vec![KSM, KUSD].iter().for_each(|token| {
				let ed =
					(<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
				assert_eq!(
					Currencies::free_balance(KAR, &TransactionPaymentPalletId::get().into_sub_account(token.clone())),
					pool_size
				);
				assert_eq!(
					Currencies::free_balance(
						token.clone(),
						&TransactionPaymentPalletId::get().into_sub_account(token.clone())
					),
					ed
				);
			});
			assert_eq!(
				Currencies::free_balance(KAR, &TransactionPaymentPalletId::get().into_sub_account(LKSM)),
				0
			);
			assert_eq!(
				Currencies::free_balance(LKSM, &TransactionPaymentPalletId::get().into_sub_account(LKSM)),
				0
			);

			// set_swap_balance_threshold should gt pool_size
			let pool_size: Balance = module_transaction_payment::Pallet::<Runtime>::pool_size(KSM);
			let swap_threshold = module_transaction_payment::Pallet::<Runtime>::set_swap_balance_threshold(
				Origin::signed(treasury_account),
				KSM,
				pool_size.saturating_add(1),
			);
			assert!(swap_threshold.is_err());
		});
}

#[cfg(feature = "with-karura-runtime")]
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
	let expect_weight: Weight = 800_000_000;
	let xcm_weight: Weight = <XcmConfig as Config>::Weigher::weight(&mut message).unwrap();
	assert_eq!(xcm_weight, expect_weight);

	// fixed rate, ksm_per_second/kar_per_second=1/50, kar_per_second = 8*dollar(KAR),
	// ksm_per_second = 0.16 * dollar(KAR), fee = 0.16 * weight = 0.16 * 800_000_000 = 128_000_000
	let total_balance: Balance = 130_000_000;
	let asset: MultiAsset = (Parent, total_balance).into();
	let expect_unspent: MultiAsset = (Parent, 2_000_000).into();
	let assets: Assets = asset.into();

	// when no runtime upgrade, the newly `TransactionFeePoolTrader` will failed.
	ExtBuilder::default().build().execute_with(|| {
		let mut trader = FixedRateOfFungible::<KsmPerSecond, ()>::new();
		let result_assets = trader.buy_weight(xcm_weight, assets.clone()).unwrap();
		let unspent: Vec<MultiAsset> = result_assets.into();
		assert_eq!(vec![expect_unspent.clone()], unspent);

		let mut period_trader = TransactionFeePoolTrader::<Runtime, CurrencyIdConvert, KarPerSecondAsBased, ()>::new();
		let result_assets = period_trader.buy_weight(xcm_weight, assets.clone());
		assert!(result_assets.is_err());
	});

	// do runtime upgrade
	ExtBuilder::default()
		.balances(vec![
			(AccountId::from(ALICE), KAR, 100000 * dollar(KAR)),
			(AccountId::from(ALICE), KSM, 200 * dollar(KSM)),
		])
		.build()
		.execute_with(|| {
			let treasury_account = KaruraTreasuryAccount::get();
			let fee_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account(KSM);
			// FeePoolSize set to 5 KAR = 50*ED, the treasury already got ED balance when startup.
			let ed = NativeTokenExistentialDeposit::get();
			let ksm_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(KSM);
			let pool_size = FeePoolSize::get();

			// treasury account: KAR=50*KAR_ED, KSM=KSM_ED, KUSD=KUSD_ED
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				KAR,
				pool_size.unique_saturated_into(),
			));
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed + pool_size);
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				KSM,
				ksm_ed.unique_saturated_into(),
			));

			// runtime upgrade
			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				KSM,
				KAR,
				100 * dollar(KSM),
				10000 * dollar(KAR),
				0,
				false
			));
			MockRuntimeUpgrade::on_runtime_upgrade();
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), ed);
			assert_eq!(Currencies::free_balance(KAR, &fee_account1), pool_size);
			assert_eq!(Currencies::free_balance(KSM, &fee_account1), ksm_ed);

			let ksm_exchange_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(KSM).unwrap();
			let spent = ksm_exchange_rate.saturating_mul_int(8 * expect_weight);
			let expect_unspent: MultiAsset = (Parent, total_balance - spent as u128).into();

			// the newly `TransactionFeePoolTrader` works fine as first priority
			let mut period_trader =
				TransactionFeePoolTrader::<Runtime, CurrencyIdConvert, KarPerSecondAsBased, ()>::new();
			let result_assets = period_trader.buy_weight(xcm_weight, assets);
			let unspent: Vec<MultiAsset> = result_assets.unwrap().into();
			assert_eq!(vec![expect_unspent.clone()], unspent);
		});
}

#[cfg(feature = "with-karura-runtime")]
#[test]
fn charge_transaction_payment_and_threshold_works() {
	let native_ed = NativeTokenExistentialDeposit::get();
	let pool_size = FeePoolSize::get();
	let ksm_ed = <Currencies as MultiCurrency<AccountId>>::minimum_balance(KSM);

	let treasury_account = KaruraTreasuryAccount::get();
	let sub_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account(KSM);
	let bob_ksm_balance = 100 * dollar(KSM);

	ExtBuilder::default()
		.balances(vec![
			// Alice for Dex, Bob for transaction payment
			(AccountId::from(ALICE), KAR, 100000 * dollar(KAR)),
			(AccountId::from(ALICE), KSM, 200 * dollar(KSM)),
			(AccountId::from(BOB), KAR, native_ed),
			(AccountId::from(BOB), KSM, bob_ksm_balance),
		])
		.build()
		.execute_with(|| {
			// treasury account for on_runtime_upgrade
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				KAR,
				pool_size.unique_saturated_into(),
			));
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				KSM,
				ksm_ed.unique_saturated_into(),
			));

			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				KSM,
				KAR,
				100 * dollar(KSM),
				10000 * dollar(KAR),
				0,
				false
			));
			MockRuntimeUpgrade::on_runtime_upgrade();
			assert_eq!(Currencies::free_balance(KAR, &treasury_account), native_ed);
			assert_eq!(Currencies::free_balance(KAR, &sub_account1), pool_size);
			assert_eq!(Currencies::free_balance(KSM, &sub_account1), ksm_ed);

			let ksm_exchange_rate: Ratio =
				module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(KSM).unwrap();

			let threshold: Balance = module_transaction_payment::Pallet::<Runtime>::swap_balance_threshold(KSM);
			let expect_threshold = Ratio::saturating_from_rational(350, 100).saturating_mul_int(native_ed);
			assert_eq!(threshold, expect_threshold); // 350 000 000 000

			assert_ok!(Dex::add_liquidity(
				Origin::signed(AccountId::from(ALICE)),
				KSM,
				KAR,
				100 * dollar(KSM),
				10000 * dollar(KAR),
				0,
				false
			));

			let len = 150 as u32;
			let call: &<Runtime as frame_system::Config>::Call = &Call::Currencies(module_currencies::Call::transfer {
				dest: MultiAddress::Id(AccountId::from(BOB)),
				currency_id: KUSD,
				amount: 12,
			});
			let info: DispatchInfo = DispatchInfo {
				weight: 100,
				class: DispatchClass::Normal,
				pays_fee: Pays::Yes,
			};
			let fee = module_transaction_payment::Pallet::<Runtime>::compute_fee(len, &info, 0);

			let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
				&AccountId::from(BOB),
				call,
				&info,
				len as usize,
			);
			let kar1 = Currencies::free_balance(KAR, &sub_account1);
			let ksm1 = Currencies::free_balance(KSM, &sub_account1);

			let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
				&AccountId::from(BOB),
				call,
				&info,
				len as usize,
			);
			let kar2 = Currencies::free_balance(KAR, &sub_account1);
			let ksm2 = Currencies::free_balance(KSM, &sub_account1);
			assert_eq!(fee, kar1 - kar2);
			assert_eq!(ksm_exchange_rate.saturating_mul_int(fee), ksm2 - ksm1);

			for _ in 0..38 {
				let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
					&AccountId::from(BOB),
					call,
					&info,
					len as usize,
				);
			}
			let kar1 = Currencies::free_balance(KAR, &sub_account1);
			let ksm1 = Currencies::free_balance(KSM, &sub_account1);

			// set swap balance trigger, next tx will trigger swap from dex
			let _ = <module_transaction_payment::Pallet<Runtime>>::set_swap_balance_threshold(
				Origin::signed(KaruraTreasuryAccount::get()),
				KSM,
				pool_size - fee * 40,
			);

			// before execute this tx, the balance of fee pool is equal to threshold,
			// so it wouldn't trigger swap from dex.
			let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
				&AccountId::from(BOB),
				call,
				&info,
				len as usize,
			);
			let kar2 = Currencies::free_balance(KAR, &sub_account1);
			let ksm2 = Currencies::free_balance(KSM, &sub_account1);
			assert_eq!(fee, kar1 - kar2);
			assert_eq!(ksm_exchange_rate.saturating_mul_int(fee), ksm2 - ksm1);

			// this tx cause swap from dex, but the fee calculation still use the old rate.
			let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
				&AccountId::from(BOB),
				call,
				&info,
				len as usize,
			);
			let kar1 = Currencies::free_balance(KAR, &sub_account1);
			let ksm1 = Currencies::free_balance(KSM, &sub_account1);
			assert_eq!(ksm_ed + ksm_exchange_rate.saturating_mul_int(fee), ksm1);
			assert_eq!(kar1 > kar2, true);
			assert_eq!(ksm2 > ksm1, true);

			// next tx use the new rate to calculate the fee to be transfer.
			let new_rate: Ratio = module_transaction_payment::Pallet::<Runtime>::token_exchange_rate(KSM).unwrap();

			let _ = <module_transaction_payment::ChargeTransactionPayment<Runtime>>::from(0).validate(
				&AccountId::from(BOB),
				call,
				&info,
				len as usize,
			);
			let kar2 = Currencies::free_balance(KAR, &sub_account1);
			let ksm2 = Currencies::free_balance(KSM, &sub_account1);
			assert_eq!(fee, kar1 - kar2);
			assert_eq!(new_rate.saturating_mul_int(fee), ksm2 - ksm1);
		});
}

#[cfg(feature = "with-acala-runtime")]
#[test]
fn acala_dex_disable_works() {
	use acala_runtime::{
		AcalaTreasuryAccount, FeePoolSize, NativeTokenExistentialDeposit, TransactionPaymentPalletId,
		TransactionPaymentUpgrade,
	};

	ExtBuilder::default().build().execute_with(|| {
		let treasury_account = AcalaTreasuryAccount::get();
		let fee_account1: AccountId = TransactionPaymentPalletId::get().into_sub_account(DOT);
		let fee_account2: AccountId = TransactionPaymentPalletId::get().into_sub_account(AUSD);
		let ed = NativeTokenExistentialDeposit::get();
		let pool_size = FeePoolSize::get();

		assert_ok!(Currencies::update_balance(
			Origin::root(),
			MultiAddress::Id(treasury_account.clone()),
			ACA,
			pool_size.saturating_mul(3).unique_saturated_into(),
		));
		assert_eq!(Currencies::free_balance(ACA, &treasury_account), ed + pool_size * 3);
		vec![DOT, AUSD].iter().for_each(|token| {
			let ed = (<Currencies as MultiCurrency<AccountId>>::minimum_balance(token.clone())).unique_saturated_into();
			assert_ok!(Currencies::update_balance(
				Origin::root(),
				MultiAddress::Id(treasury_account.clone()),
				token.clone(),
				ed,
			));
		});

		TransactionPaymentUpgrade::on_runtime_upgrade();
		assert_eq!(Currencies::free_balance(ACA, &fee_account1), 0);
		assert_eq!(Currencies::free_balance(ACA, &fee_account2), 0);
	});
}
