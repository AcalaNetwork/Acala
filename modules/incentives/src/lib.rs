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

//! # Incentives Module
//!
//! ## Overview
//!
//! Acala platform need support different types of rewards for some other protocol.
//! Each Pool has its own multi currencies rewards and reward accumulation
//! mechanism. ORML rewards module records the total shares, total multi currencies rewards anduser
//! shares of specific pool. Incentives module provides hooks to other protocals to manage shares,
//! accumulates rewards and distributes rewards to users based on their shares.
//!
//! Pool types:
//! 1. Loans: record the shares and rewards for users of Loans(Honzon protocol).
//! 2. Dex: record the shares and rewards for DEX makers who staking LP token.
//!
//! Rewards accumulation:
//! 1. Incentives: periodicly(AccumulatePeriod), accumulate fixed amount according to Incentive.
//! Rewards come from RewardsSource, please transfer enough tokens to RewardsSource before
//! start incentive plan.
//! 2. DexSaving: periodicly(AccumulatePeriod), the reward currency is Stable(KUSD/AUSD),
//! the accumulation amount is the multiplier of DexSavingRewardRates and the stable amount of
//! corresponding liquidity pool. CDPTreasury will issue the stable currency to RewardsSource.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{log, pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::{Happened, MultiCurrency, RewardHandler};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, One, UniqueSaturatedInto, Zero},
	DispatchResult, FixedPointNumber, Permill,
};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};
use support::{CDPTreasury, DEXIncentives, DEXManager, EmergencyShutdown, IncentivesManager, PoolId, Rate};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ orml_rewards::Config<Share = Balance, Balance = Balance, PoolId = PoolId, CurrencyId = CurrencyId>
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The period to accumulate rewards
		#[pallet::constant]
		type AccumulatePeriod: Get<Self::BlockNumber>;

		/// The native currency for earning staking
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// The reward type for dex saving.
		#[pallet::constant]
		type StableCurrencyId: Get<CurrencyId>;

		/// The source account for native token rewards.
		#[pallet::constant]
		type RewardsSource: Get<Self::AccountId>;

		/// Additional share amount from earning
		#[pallet::constant]
		type EarnShareBooster: Get<Permill>;

		/// The origin which may update incentive related params
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// CDP treasury to issue rewards in stable token
		type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

		/// Currency for transfer assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// DEX to supply liquidity info
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		/// Emergency shutdown.
		type EmergencyShutdown: EmergencyShutdown;

		/// The module id, keep DexShare LP.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Share amount is not enough
		NotEnough,
		/// Invalid currency id
		InvalidCurrencyId,
		/// Invalid pool id
		InvalidPoolId,
		/// Invalid rate
		InvalidRate,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Deposit DEX share.
		DepositDexShare {
			who: T::AccountId,
			dex_share_type: CurrencyId,
			deposit: Balance,
		},
		/// Withdraw DEX share.
		WithdrawDexShare {
			who: T::AccountId,
			dex_share_type: CurrencyId,
			withdraw: Balance,
		},
		/// Claim rewards.
		ClaimRewards {
			who: T::AccountId,
			pool: PoolId,
			reward_currency_id: CurrencyId,
			actual_amount: Balance,
			deduction_amount: Balance,
		},
		/// Incentive reward amount updated.
		IncentiveRewardAmountUpdated {
			pool: PoolId,
			reward_currency_id: CurrencyId,
			reward_amount_per_period: Balance,
		},
		/// Saving reward rate updated.
		SavingRewardRateUpdated { pool: PoolId, reward_rate_per_period: Rate },
		/// Payout deduction rate updated.
		ClaimRewardDeductionRateUpdated { pool: PoolId, deduction_rate: Rate },
	}

	/// Mapping from pool to its fixed incentive amounts of multi currencies per period.
	///
	/// IncentiveRewardAmounts: double_map Pool, RewardCurrencyId => RewardAmountPerPeriod
	#[pallet::storage]
	#[pallet::getter(fn incentive_reward_amounts)]
	pub type IncentiveRewardAmounts<T: Config> =
		StorageDoubleMap<_, Twox64Concat, PoolId, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// Mapping from pool to its fixed reward rate per period.
	///
	/// DexSavingRewardRates: map Pool => SavingRatePerPeriod
	#[pallet::storage]
	#[pallet::getter(fn dex_saving_reward_rates)]
	pub type DexSavingRewardRates<T: Config> = StorageMap<_, Twox64Concat, PoolId, Rate, ValueQuery>;

	/// Mapping from pool to its claim reward deduction rate.
	///
	/// ClaimRewardDeductionRates: map Pool => DeductionRate
	#[pallet::storage]
	#[pallet::getter(fn claim_reward_deduction_rates)]
	pub type ClaimRewardDeductionRates<T: Config> = StorageMap<_, Twox64Concat, PoolId, Rate, ValueQuery>;

	/// The pending rewards amount, actual available rewards amount may be deducted
	///
	/// PendingMultiRewards: double_map PoolId, AccountId => BTreeMap<CurrencyId, Balance>
	#[pallet::storage]
	#[pallet::getter(fn pending_multi_rewards)]
	pub type PendingMultiRewards<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		PoolId,
		Twox64Concat,
		T::AccountId,
		BTreeMap<CurrencyId, Balance>,
		ValueQuery,
	>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// accumulate reward periodically
			if now % T::AccumulatePeriod::get() == Zero::zero() {
				let mut count: u32 = 0;
				let shutdown = T::EmergencyShutdown::is_shutdown();

				for (pool_id, pool_info) in orml_rewards::PoolInfos::<T>::iter() {
					if !pool_info.total_shares.is_zero() {
						match pool_id {
							// do not accumulate incentives for PoolId::Loans after shutdown
							PoolId::Loans(_) if !shutdown => {
								count += 1;
								Self::accumulate_incentives(pool_id);
							}
							PoolId::Dex(lp_currency_id) => {
								// do not accumulate dex saving any more after shutdown
								if !shutdown {
									Self::accumulate_dex_saving(lp_currency_id, pool_id);
								}
								count += 1;
								Self::accumulate_incentives(pool_id);
							}
							_ => {}
						}
					}
				}

				T::WeightInfo::on_initialize(count)
			} else {
				0
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Stake LP token to add shares of Pool::Dex
		///
		/// The dispatch origin of this call must be `Signed` by the transactor.
		///
		/// - `lp_currency_id`: LP token type
		/// - `amount`: amount to stake
		#[pallet::weight(<T as Config>::WeightInfo::deposit_dex_share())]
		#[transactional]
		pub fn deposit_dex_share(
			origin: OriginFor<T>,
			lp_currency_id: CurrencyId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_deposit_dex_share(&who, lp_currency_id, amount)?;
			Ok(())
		}

		/// Unstake LP token to remove shares of Pool::Dex
		///
		/// The dispatch origin of this call must be `Signed` by the transactor.
		///
		/// - `lp_currency_id`: LP token type
		/// - `amount`: amount to unstake
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_dex_share())]
		#[transactional]
		pub fn withdraw_dex_share(
			origin: OriginFor<T>,
			lp_currency_id: CurrencyId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_withdraw_dex_share(&who, lp_currency_id, amount)?;
			Ok(())
		}

		/// Claim all avalible multi currencies rewards for specific PoolId.
		///
		/// The dispatch origin of this call must be `Signed` by the transactor.
		///
		/// - `pool_id`: pool type
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		#[transactional]
		pub fn claim_rewards(origin: OriginFor<T>, pool_id: PoolId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::do_claim_rewards(who, pool_id)
		}

		/// Update incentive reward amount for specific PoolId
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `updates`: Vec<(PoolId, Vec<(RewardCurrencyId, FixedAmountPerPeriod)>)>
		#[pallet::weight(<T as Config>::WeightInfo::update_incentive_rewards(
			updates.iter().fold(0, |count, x| count + x.1.len()) as u32
		))]
		#[transactional]
		pub fn update_incentive_rewards(
			origin: OriginFor<T>,
			updates: Vec<(PoolId, Vec<(CurrencyId, Balance)>)>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, update_list) in updates {
				if let PoolId::Dex(currency_id) = pool_id {
					ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidPoolId);
				}

				for (currency_id, amount) in update_list {
					IncentiveRewardAmounts::<T>::mutate_exists(pool_id, currency_id, |maybe_amount| {
						let mut v = maybe_amount.unwrap_or_default();
						if amount != v {
							v = amount;
							Self::deposit_event(Event::IncentiveRewardAmountUpdated {
								pool: pool_id,
								reward_currency_id: currency_id,
								reward_amount_per_period: amount,
							});
						}

						if v.is_zero() {
							*maybe_amount = None;
						} else {
							*maybe_amount = Some(v);
						}
					});
				}
			}
			Ok(())
		}

		/// Update DEX saving reward rate for specific PoolId
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `updates`: Vec<(PoolId, Rate)>
		#[pallet::weight(<T as Config>::WeightInfo::update_dex_saving_rewards(updates.len() as u32))]
		#[transactional]
		pub fn update_dex_saving_rewards(origin: OriginFor<T>, updates: Vec<(PoolId, Rate)>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, rate) in updates {
				match pool_id {
					PoolId::Dex(currency_id) if currency_id.is_dex_share_currency_id() => {}
					_ => return Err(Error::<T>::InvalidPoolId.into()),
				}
				ensure!(rate <= Rate::one(), Error::<T>::InvalidRate);

				DexSavingRewardRates::<T>::mutate_exists(&pool_id, |maybe_rate| {
					let mut v = maybe_rate.unwrap_or_default();
					if rate != v {
						v = rate;
						Self::deposit_event(Event::SavingRewardRateUpdated {
							pool: pool_id,
							reward_rate_per_period: rate,
						});
					}

					if v.is_zero() {
						*maybe_rate = None;
					} else {
						*maybe_rate = Some(v);
					}
				});
			}
			Ok(())
		}

		/// Update claim rewards deduction rates for all rewards currencies of specific PoolId
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `updates`: Vec<(PoolId, DecutionRate>)>
		#[pallet::weight(<T as Config>::WeightInfo::update_claim_reward_deduction_rates(updates.len() as u32))]
		#[transactional]
		pub fn update_claim_reward_deduction_rates(
			origin: OriginFor<T>,
			updates: Vec<(PoolId, Rate)>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, deduction_rate) in updates {
				if let PoolId::Dex(currency_id) = pool_id {
					ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidPoolId);
				}
				ensure!(deduction_rate <= Rate::one(), Error::<T>::InvalidRate);
				ClaimRewardDeductionRates::<T>::mutate_exists(&pool_id, |maybe_rate| {
					let mut v = maybe_rate.unwrap_or_default();
					if deduction_rate != v {
						v = deduction_rate;
						Self::deposit_event(Event::ClaimRewardDeductionRateUpdated {
							pool: pool_id,
							deduction_rate,
						});
					}

					if v.is_zero() {
						*maybe_rate = None;
					} else {
						*maybe_rate = Some(v);
					}
				});
			}
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	// accumulate incentive rewards of multi currencies
	fn accumulate_incentives(pool_id: PoolId) {
		for (reward_currency_id, reward_amount) in IncentiveRewardAmounts::<T>::iter_prefix(pool_id) {
			if reward_amount.is_zero() {
				continue;
			}

			let res = T::Currency::transfer(
				reward_currency_id,
				&T::RewardsSource::get(),
				&Self::account_id(),
				reward_amount,
			);

			match res {
				Ok(_) => {
					let _ = <orml_rewards::Pallet<T>>::accumulate_reward(
						&pool_id,
						reward_currency_id,
						reward_amount,
					)
					.map_err(|e| {
						log::error!(
							target: "incentives",
							"accumulate_reward: failed to accumulate reward to non-existen pool {:?}, reward_currency_id {:?}, reward_amount {:?}: {:?}",
							pool_id, reward_currency_id, reward_amount, e
						);
					});
				}
				Err(e) => {
					log::warn!(
						target: "incentives",
						"transfer: failed to transfer {:?} {:?} from {:?} to {:?}: {:?}. \
						This is unexpected but should be safe",
						reward_amount, reward_currency_id, T::RewardsSource::get(), Self::account_id(), e
					);
				}
			}
		}
	}

	// accumulate DEX saving reward(stable currency) for Dex Pool
	fn accumulate_dex_saving(lp_currency_id: CurrencyId, pool_id: PoolId) {
		let stable_currency_id = T::StableCurrencyId::get();
		let dex_saving_reward_rate = Self::dex_saving_reward_rates(&pool_id);

		if !dex_saving_reward_rate.is_zero() {
			if let Some((currency_id_a, currency_id_b)) = lp_currency_id.split_dex_share_currency_id() {
				// accumulate saving reward only for liquidity pool of stable currency id
				let dex_saving_reward_base = if currency_id_a == stable_currency_id {
					T::DEX::get_liquidity_pool(stable_currency_id, currency_id_b).0
				} else if currency_id_b == stable_currency_id {
					T::DEX::get_liquidity_pool(stable_currency_id, currency_id_a).0
				} else {
					Zero::zero()
				};
				let dex_saving_reward_amount = dex_saving_reward_rate.saturating_mul_int(dex_saving_reward_base);

				// issue stable currency without backing.
				if !dex_saving_reward_amount.is_zero() {
					let res = T::CDPTreasury::issue_debit(&Self::account_id(), dex_saving_reward_amount, false);
					match res {
						Ok(_) => {
							let _ = <orml_rewards::Pallet<T>>::accumulate_reward(
								&pool_id,
								stable_currency_id,
								dex_saving_reward_amount,
							)
							.map_err(|e| {
								log::error!(
									target: "incentives",
									"accumulate_reward: failed to accumulate reward to non-existen pool {:?}, reward_currency {:?}, amount {:?}: {:?}",
									pool_id, stable_currency_id, dex_saving_reward_amount, e
								);
							});
						}
						Err(e) => {
							log::warn!(
								target: "incentives",
								"issue_debit: failed to issue {:?} unbacked stable to {:?}: {:?}. \
								This is unexpected but should be safe",
								dex_saving_reward_amount, Self::account_id(), e
							);
						}
					}
				}
			}
		}
	}

	fn do_claim_rewards(who: T::AccountId, pool_id: PoolId) -> DispatchResult {
		// orml_rewards will claim rewards for all currencies rewards
		<orml_rewards::Pallet<T>>::claim_rewards(&who, &pool_id);

		let pending_multi_rewards: BTreeMap<CurrencyId, Balance> = PendingMultiRewards::<T>::take(&pool_id, &who);
		let deduction_rate = Self::claim_reward_deduction_rates(&pool_id);

		for (currency_id, pending_reward) in pending_multi_rewards {
			if pending_reward.is_zero() {
				continue;
			}
			// calculate actual rewards and deduction amount
			let (actual_amount, deduction_amount) = {
				let deduction_amount = deduction_rate.saturating_mul_int(pending_reward).min(pending_reward);
				if !deduction_amount.is_zero() {
					// re-accumulate deduction to rewards pool if deduction amount is not zero
					let _ = <orml_rewards::Pallet<T>>::accumulate_reward(&pool_id, currency_id, deduction_amount).map_err(|e| {
						log::error!(
							target: "incentives",
							"accumulate_reward: failed to accumulate reward to non-existen pool {:?}, reward_currency_id {:?}, reward_amount {:?}: {:?}",
							pool_id, currency_id, deduction_amount, e
						);
					});
				}
				(pending_reward.saturating_sub(deduction_amount), deduction_amount)
			};

			// transfer to `who` maybe fail because of the reward amount is below ED and `who` is not alive.
			// if transfer failed, do not throw err directly and try to put the tiny reward back to pool.
			let res = T::Currency::transfer(currency_id, &Self::account_id(), &who, actual_amount);
			if res.is_err() {
				let _ = <orml_rewards::Pallet<T>>::accumulate_reward(&pool_id, currency_id, actual_amount).map_err(|e| {
					log::error!(
						target: "incentives",
						"accumulate_reward: failed to accumulate reward to non-existen pool {:?}, reward_currency_id {:?}, reward_amount {:?}: {:?}",
						pool_id, currency_id, actual_amount, e
					);
				});
			}

			Self::deposit_event(Event::ClaimRewards {
				who: who.clone(),
				pool: pool_id,
				reward_currency_id: currency_id,
				actual_amount,
				deduction_amount,
			});
		}

		Ok(())
	}
}

impl<T: Config> DEXIncentives<T::AccountId, CurrencyId, Balance> for Pallet<T> {
	fn do_deposit_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);

		T::Currency::transfer(lp_currency_id, who, &Self::account_id(), amount)?;
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Dex(lp_currency_id), amount.unique_saturated_into());

		Self::deposit_event(Event::DepositDexShare {
			who: who.clone(),
			dex_share_type: lp_currency_id,
			deposit: amount,
		});
		Ok(())
	}

	fn do_withdraw_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
		ensure!(
			<orml_rewards::Pallet<T>>::shares_and_withdrawn_rewards(&PoolId::Dex(lp_currency_id), &who).0 >= amount,
			Error::<T>::NotEnough,
		);

		T::Currency::transfer(lp_currency_id, &Self::account_id(), who, amount)?;
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Dex(lp_currency_id), amount.unique_saturated_into());

		Self::deposit_event(Event::WithdrawDexShare {
			who: who.clone(),
			dex_share_type: lp_currency_id,
			withdraw: amount,
		});
		Ok(())
	}
}

impl<T: Config> IncentivesManager<T::AccountId, Balance, CurrencyId, PoolId> for Pallet<T> {
	fn get_incentive_reward_amount(pool_id: PoolId, currency_id: CurrencyId) -> Balance {
		IncentiveRewardAmounts::<T>::get(pool_id, currency_id)
	}

	fn get_dex_reward_rate(pool_id: PoolId) -> Rate {
		DexSavingRewardRates::<T>::get(pool_id)
	}

	fn deposit_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		Self::do_deposit_dex_share(who, lp_currency_id, amount)
	}

	fn withdraw_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		Self::do_withdraw_dex_share(who, lp_currency_id, amount)
	}

	fn claim_rewards(who: T::AccountId, pool_id: PoolId) -> DispatchResult {
		Self::do_claim_rewards(who, pool_id)
	}

	fn get_claim_reward_deduction_rate(pool_id: PoolId) -> Rate {
		ClaimRewardDeductionRates::<T>::get(pool_id)
	}

	fn get_pending_rewards(pool_id: PoolId, who: T::AccountId, reward_currencies: Vec<CurrencyId>) -> Vec<Balance> {
		let rewards_map = PendingMultiRewards::<T>::get(pool_id, who);
		let mut reward_balances = Vec::new();
		for reward_currency in reward_currencies {
			let reward_amount = rewards_map.get(&reward_currency).copied().unwrap_or_default();
			reward_balances.push(reward_amount);
		}
		reward_balances
	}
}

pub struct OnUpdateLoan<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, CurrencyId, Amount, Balance)> for OnUpdateLoan<T> {
	fn happened(info: &(T::AccountId, CurrencyId, Amount, Balance)) {
		let (who, currency_id, adjustment, _previous_amount) = info;
		let adjustment_abs = TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();

		if adjustment.is_positive() {
			<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Loans(*currency_id), adjustment_abs);
		} else {
			<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Loans(*currency_id), adjustment_abs);
		};
	}
}

impl<T: Config> RewardHandler<T::AccountId, CurrencyId> for Pallet<T> {
	type Balance = Balance;
	type PoolId = PoolId;

	fn payout(who: &T::AccountId, pool_id: &Self::PoolId, currency_id: CurrencyId, payout_amount: Self::Balance) {
		if payout_amount.is_zero() {
			return;
		}
		PendingMultiRewards::<T>::mutate(pool_id, who, |rewards| {
			rewards
				.entry(currency_id)
				.and_modify(|current| *current = current.saturating_add(payout_amount))
				.or_insert(payout_amount);
		});
	}
}

pub struct OnEarningBonded<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, Balance)> for OnEarningBonded<T> {
	fn happened((who, amount): &(T::AccountId, Balance)) {
		let share = amount.saturating_add(T::EarnShareBooster::get() * *amount);
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Loans(T::NativeCurrencyId::get()), share);
	}
}

pub struct OnEarningUnbonded<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, Balance)> for OnEarningUnbonded<T> {
	fn happened((who, amount): &(T::AccountId, Balance)) {
		let share = amount.saturating_add(T::EarnShareBooster::get() * *amount);
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Loans(T::NativeCurrencyId::get()), share);
	}
}
