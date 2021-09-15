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

use super::*;
use frame_support::log;

/// PoolId for various rewards pools
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PoolIdV0<AccountId> {
	/// Rewards pool(NativeCurrencyId) for users who open CDP
	LoansIncentive(CurrencyId),

	/// Rewards pool(NativeCurrencyId) for market makers who provide dex
	/// liquidity
	DexIncentive(CurrencyId),

	/// Rewards pool(NativeCurrencyId) for users who staking by Homa protocol
	HomaIncentive,

	/// Rewards pool(StableCurrencyId) for liquidators who provide dex liquidity
	/// to participate automatic liquidation
	DexSaving(CurrencyId),

	/// Rewards pool(LiquidCurrencyId) for users who guarantee for validators by
	/// Homa protocol
	HomaValidatorAllowance(AccountId),
}

pub struct PoolIdConvertor<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> sp_runtime::traits::Convert<PoolIdV0<T::RelaychainAccountId>, Option<PoolId>> for PoolIdConvertor<T> {
	fn convert(a: PoolIdV0<T::RelaychainAccountId>) -> Option<PoolId> {
		convert_to_new_pool_id::<T>(a)
	}
}

// migrate storage `PayoutDeductionRates` to `ClaimRewardDeductionRates`
pub fn migrate_to_claim_reward_deduction_rates<T: Config>(maybe_limit: Option<usize>) -> Weight {
	let mut remove_items = 0;
	let mut insert_items = 0;

	for (old_pool_id, rate) in PayoutDeductionRates::<T>::drain().take(maybe_limit.unwrap_or(usize::MAX)) {
		remove_items += 1;
		if !rate.is_zero() {
			if let Some(pool_id) = convert_to_new_pool_id::<T>(old_pool_id) {
				ClaimRewardDeductionRates::<T>::insert(pool_id, rate);
				insert_items += 1;
			}
		}
	}

	log::info!(
		target: "incentives-migration",
		"migrate incentives PayoutDeductionRates: migrate {:?} items",
		remove_items,
	);

	let total_reads_writes = remove_items + insert_items;
	T::DbWeight::get().reads_writes(total_reads_writes, total_reads_writes)
}

// migrate storage `DexSavingRewardRate` to `DexSavingRewardRates`
pub fn migrate_to_dex_saving_reward_rates<T: Config>(maybe_limit: Option<usize>) -> Weight {
	let mut remove_items = 0;
	let mut insert_items = 0;

	for (old_pool_id, rate) in DexSavingRewardRate::<T>::drain().take(maybe_limit.unwrap_or(usize::MAX)) {
		remove_items += 1;
		if !rate.is_zero() {
			if let PoolIdV0::DexSaving(currency_id) = old_pool_id {
				DexSavingRewardRates::<T>::insert(PoolId::Dex(currency_id), rate);
				insert_items += 1;
			}
		}
	}

	log::info!(
		target: "incentives-migration",
		"migrate incentives DexSavingRewardRate: migrate {:?} items",
		remove_items,
	);

	let total_reads_writes = remove_items + insert_items;
	T::DbWeight::get().reads_writes(total_reads_writes, total_reads_writes)
}

// migrate storage `IncentiveRewardAmount` to `IncentiveRewardAmounts`
pub fn migrate_to_incentive_reward_amounts<T: Config>(maybe_limit: Option<usize>) -> Weight {
	let reward_currency_id = T::NativeCurrencyId::get();
	let mut remove_items = 0;
	let mut insert_items = 0;

	for (old_pool_id, amount) in IncentiveRewardAmount::<T>::drain().take(maybe_limit.unwrap_or(usize::MAX)) {
		remove_items += 1;
		if !amount.is_zero() {
			if let Some(pool_id) = match old_pool_id {
				PoolIdV0::DexIncentive(currency_id) => Some(PoolId::Dex(currency_id)),
				PoolIdV0::LoansIncentive(currency_id) => Some(PoolId::Loans(currency_id)),
				_ => None,
			} {
				IncentiveRewardAmounts::<T>::insert(pool_id, reward_currency_id, amount);
				insert_items += 1;
			}
		}
	}

	log::info!(
		target: "incentives-migration",
		"migrate incentives IncentiveRewardAmount: migrate {:?} items",
		remove_items,
	);

	let total_reads_writes = remove_items + insert_items;
	T::DbWeight::get().reads_writes(total_reads_writes, total_reads_writes)
}

// migrate storage `PendingRewards` to `PendingMultiRewards`
pub fn migrate_to_pending_multi_rewards<T: Config>(maybe_limit: Option<usize>) -> Weight {
	let mut remove_items = 0;
	let mut insert_items = 0;

	for (old_pool_id, who, reward_amount) in PendingRewards::<T>::drain().take(maybe_limit.unwrap_or(usize::MAX)) {
		remove_items += 1;

		if !reward_amount.is_zero() {
			if let Some(pool_id) = convert_to_new_pool_id::<T>(old_pool_id.clone()) {
				PendingMultiRewards::<T>::mutate(pool_id, who, |multi_rewards| {
					multi_rewards
						.entry(get_reward_currency_id::<T>(old_pool_id))
						.and_modify(|amount| {
							*amount = amount.saturating_add(reward_amount);
						})
						.or_insert(reward_amount);
				});
				insert_items += 1;
			}
		}
	}

	log::info!(
		target: "incentives-migration",
		"migrate incentives PendingRewards: migrate {:?} items",
		remove_items,
	);

	let total_reads_writes = remove_items + insert_items;
	T::DbWeight::get().reads_writes(total_reads_writes, total_reads_writes)
}

// helper to map PoolIdV0 to PoolId
pub fn convert_to_new_pool_id<T: Config>(old_pool_id: PoolIdV0<T::RelaychainAccountId>) -> Option<PoolId> {
	match old_pool_id {
		PoolIdV0::LoansIncentive(collateral_currency_id) => Some(PoolId::Loans(collateral_currency_id)),
		PoolIdV0::DexIncentive(lp_currency_id) | PoolIdV0::DexSaving(lp_currency_id) => {
			Some(PoolId::Dex(lp_currency_id))
		}
		_ => None,
	}
}

// helper to map PoolIdV0 to reward currency id
pub fn get_reward_currency_id<T: Config>(old_pool_id: PoolIdV0<T::RelaychainAccountId>) -> CurrencyId {
	match old_pool_id {
		PoolIdV0::HomaValidatorAllowance(_) => T::LiquidCurrencyId::get(),
		PoolIdV0::DexSaving(_) => T::StableCurrencyId::get(),
		_ => T::NativeCurrencyId::get(),
	}
}

#[test]
fn migrate_to_claim_reward_deduction_rates_works() {
	use super::mock::*;

	ExtBuilder::default().build().execute_with(|| {
		PayoutDeductionRates::<Runtime>::insert(PoolIdV0::DexSaving(DOT_AUSD_LP), Rate::zero());
		PayoutDeductionRates::<Runtime>::insert(
			PoolIdV0::DexIncentive(DOT_AUSD_LP),
			Rate::saturating_from_rational(30, 100),
		);
		PayoutDeductionRates::<Runtime>::insert(PoolIdV0::LoansIncentive(DOT), Rate::saturating_from_rational(20, 100));
		PayoutDeductionRates::<Runtime>::insert(PoolIdV0::HomaIncentive, Rate::saturating_from_rational(10, 100));
		PayoutDeductionRates::<Runtime>::insert(
			PoolIdV0::HomaValidatorAllowance(ALICE::get()),
			Rate::saturating_from_rational(20, 100),
		);

		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT)),
			true
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::HomaIncentive),
			true
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::HomaValidatorAllowance(ALICE::get())),
			true
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Loans(DOT)),
			false
		);

		assert_eq!(
			migrate_to_claim_reward_deduction_rates::<Runtime>(None),
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(5 + 2, 5 + 2)
		);

		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT)),
			false
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::HomaIncentive),
			false
		);
		assert_eq!(
			PayoutDeductionRates::<Runtime>::contains_key(PoolIdV0::HomaValidatorAllowance(ALICE::get())),
			false
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			ClaimRewardDeductionRates::<Runtime>::contains_key(PoolId::Loans(DOT)),
			true
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(30, 100)
		);
		assert_eq!(
			IncentivesModule::claim_reward_deduction_rates(PoolId::Loans(DOT)),
			Rate::saturating_from_rational(20, 100)
		);
	});
}

#[test]
fn migrate_to_dex_saving_reward_rates_works() {
	use super::mock::*;

	ExtBuilder::default().build().execute_with(|| {
		DexSavingRewardRate::<Runtime>::insert(
			PoolIdV0::DexSaving(DOT_AUSD_LP),
			Rate::saturating_from_rational(20, 100),
		);
		DexSavingRewardRate::<Runtime>::insert(PoolIdV0::DexSaving(BTC_AUSD_LP), Rate::zero());
		DexSavingRewardRate::<Runtime>::insert(
			PoolIdV0::DexIncentive(BTC_AUSD_LP),
			Rate::saturating_from_rational(30, 100),
		);

		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexSaving(BTC_AUSD_LP)),
			true
		);
		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexIncentive(BTC_AUSD_LP)),
			true
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			false
		);

		assert_eq!(
			migrate_to_dex_saving_reward_rates::<Runtime>(None),
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(3 + 1, 3 + 1)
		);

		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexSaving(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			DexSavingRewardRate::<Runtime>::contains_key(PoolIdV0::DexIncentive(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			DexSavingRewardRates::<Runtime>::contains_key(PoolId::Dex(BTC_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(DOT_AUSD_LP)),
			Rate::saturating_from_rational(20, 100)
		);
		assert_eq!(
			IncentivesModule::dex_saving_reward_rates(PoolId::Dex(BTC_AUSD_LP)),
			Rate::zero()
		);
	});
}

#[test]
fn migrate_to_incentive_reward_amounts_works() {
	use super::mock::*;

	ExtBuilder::default().build().execute_with(|| {
		IncentiveRewardAmount::<Runtime>::insert(PoolIdV0::DexIncentive(DOT_AUSD_LP), 0);
		IncentiveRewardAmount::<Runtime>::insert(PoolIdV0::DexSaving(DOT_AUSD_LP), 100);
		IncentiveRewardAmount::<Runtime>::insert(PoolIdV0::LoansIncentive(DOT), 1000);
		IncentiveRewardAmount::<Runtime>::insert(PoolIdV0::HomaIncentive, 2000);

		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			true
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT)),
			true
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::HomaIncentive),
			true
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), ACA),
			false
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Loans(DOT), ACA),
			false
		);

		assert_eq!(
			migrate_to_incentive_reward_amounts::<Runtime>(None),
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(4 + 1, 4 + 1)
		);

		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP)),
			false
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT)),
			false
		);
		assert_eq!(
			IncentiveRewardAmount::<Runtime>::contains_key(PoolIdV0::HomaIncentive),
			false
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), ACA),
			false
		);
		assert_eq!(
			IncentiveRewardAmounts::<Runtime>::contains_key(PoolId::Loans(DOT), ACA),
			true
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Dex(DOT_AUSD_LP), ACA),
			0
		);
		assert_eq!(
			IncentivesModule::incentive_reward_amounts(PoolId::Loans(DOT), ACA),
			1000
		);
	});
}

#[test]
fn migrate_to_pending_multi_rewards_works() {
	use super::mock::*;

	ExtBuilder::default().build().execute_with(|| {
		PendingRewards::<Runtime>::insert(PoolIdV0::DexIncentive(DOT_AUSD_LP), ALICE::get(), 100);
		PendingRewards::<Runtime>::insert(PoolIdV0::DexSaving(DOT_AUSD_LP), ALICE::get(), 200);
		PendingRewards::<Runtime>::insert(PoolIdV0::LoansIncentive(DOT), ALICE::get(), 300);
		PendingRewards::<Runtime>::insert(PoolIdV0::HomaIncentive, ALICE::get(), 400);
		PendingRewards::<Runtime>::insert(PoolIdV0::HomaValidatorAllowance(BOB::get()), ALICE::get(), 500);

		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP), ALICE::get()),
			true
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP), ALICE::get()),
			true
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT), ALICE::get()),
			true
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::HomaIncentive, ALICE::get()),
			true
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::HomaValidatorAllowance(BOB::get()), ALICE::get()),
			true
		);
		assert_eq!(
			PendingMultiRewards::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), ALICE::get()),
			false
		);
		assert_eq!(
			PendingMultiRewards::<Runtime>::contains_key(PoolId::Loans(DOT), ALICE::get()),
			false
		);

		assert_eq!(
			migrate_to_pending_multi_rewards::<Runtime>(None),
			<Runtime as frame_system::Config>::DbWeight::get().reads_writes(5 + 3, 5 + 3)
		);

		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::DexIncentive(DOT_AUSD_LP), ALICE::get()),
			false
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::DexSaving(DOT_AUSD_LP), ALICE::get()),
			false
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::LoansIncentive(DOT), ALICE::get()),
			false
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::HomaIncentive, ALICE::get()),
			false
		);
		assert_eq!(
			PendingRewards::<Runtime>::contains_key(PoolIdV0::HomaValidatorAllowance(BOB::get()), ALICE::get()),
			false
		);
		assert_eq!(
			PendingMultiRewards::<Runtime>::contains_key(PoolId::Dex(DOT_AUSD_LP), ALICE::get()),
			true
		);
		assert_eq!(
			PendingMultiRewards::<Runtime>::contains_key(PoolId::Loans(DOT), ALICE::get()),
			true
		);

		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Dex(DOT_AUSD_LP), ALICE::get()),
			vec![(ACA, 100), (AUSD, 200)].into_iter().collect()
		);
		assert_eq!(
			IncentivesModule::pending_multi_rewards(PoolId::Loans(DOT), ALICE::get()),
			vec![(ACA, 300)].into_iter().collect()
		);
	});
}
