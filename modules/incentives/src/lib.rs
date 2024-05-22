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

//! # Incentives Module
//!
//! ## Overview
//!
//! Acala platform need support different types of rewards for some other protocol.
//! Each Pool has its own multi currencies rewards and reward accumulation
//! mechanism. ORML rewards module records the total shares, total multi currencies rewards anduser
//! shares of specific pool. Incentives module provides hooks to other protocols to manage shares,
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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use module_support::{DEXIncentives, EmergencyShutdown, FractionalRate, IncentivesManager, PoolId, Rate};
use orml_traits::{Handler, MultiCurrency, RewardHandler};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto, Zero},
	DispatchResult, FixedPointNumber,
};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The period to accumulate rewards
		#[pallet::constant]
		type AccumulatePeriod: Get<BlockNumberFor<Self>>;

		/// The native currency for earning staking
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// The source account for native token rewards.
		#[pallet::constant]
		type RewardsSource: Get<Self::AccountId>;

		/// The origin which may update incentive related params
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Currency for transfer assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

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
		/// Payout deduction rate updated.
		ClaimRewardDeductionRateUpdated { pool: PoolId, deduction_rate: Rate },
		/// Payout deduction currency updated.
		ClaimRewardDeductionCurrencyUpdated { pool: PoolId, currency: Option<CurrencyId> },
	}

	/// Mapping from pool to its fixed incentive amounts of multi currencies per period.
	///
	/// IncentiveRewardAmounts: double_map Pool, RewardCurrencyId => RewardAmountPerPeriod
	#[pallet::storage]
	#[pallet::getter(fn incentive_reward_amounts)]
	pub type IncentiveRewardAmounts<T: Config> =
		StorageDoubleMap<_, Twox64Concat, PoolId, Twox64Concat, CurrencyId, Balance, ValueQuery>;

	/// Mapping from pool to its claim reward deduction rate.
	///
	/// ClaimRewardDeductionRates: map Pool => DeductionRate
	#[pallet::storage]
	pub type ClaimRewardDeductionRates<T: Config> = StorageMap<_, Twox64Concat, PoolId, FractionalRate, ValueQuery>;

	/// If specified, ClaimRewardDeductionRates only apply to this currency.
	///
	/// ClaimRewardDeductionCurrency: map Pool => Option<RewardCurrencyId>
	#[pallet::storage]
	pub type ClaimRewardDeductionCurrency<T: Config> = StorageMap<_, Twox64Concat, PoolId, CurrencyId, OptionQuery>;

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
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(now: BlockNumberFor<T>) -> Weight {
			// accumulate reward periodically
			if now % T::AccumulatePeriod::get() == Zero::zero() {
				let mut count: u32 = 0;
				let shutdown = T::EmergencyShutdown::is_shutdown();

				for (pool_id, pool_info) in orml_rewards::PoolInfos::<T>::iter() {
					if !pool_info.total_shares.is_zero() {
						match pool_id {
							// do not accumulate incentives for PoolId::Loans after shutdown
							PoolId::Loans(_) if shutdown => {
								log::debug!(
									target: "incentives",
									"on_initialize: skip accumulate incentives for pool {:?} after shutdown",
									pool_id
								);
							}
							_ => {
								count += 1;
								Self::accumulate_incentives(pool_id);
							}
						}
					}
				}

				T::WeightInfo::on_initialize(count)
			} else {
				Weight::zero()
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
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::deposit_dex_share())]
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
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_dex_share())]
		pub fn withdraw_dex_share(
			origin: OriginFor<T>,
			lp_currency_id: CurrencyId,
			#[pallet::compact] amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_withdraw_dex_share(&who, lp_currency_id, amount)?;
			Ok(())
		}

		/// Claim all available multi currencies rewards for specific PoolId.
		///
		/// The dispatch origin of this call must be `Signed` by the transactor.
		///
		/// - `pool_id`: pool type
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>, pool_id: PoolId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::do_claim_rewards(who, pool_id)
		}

		/// Update incentive reward amount for specific PoolId
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `updates`: Vec<(PoolId, Vec<(RewardCurrencyId, FixedAmountPerPeriod)>)>
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::update_incentive_rewards(
			updates.iter().fold(0, |count, x| count + x.1.len()) as u32
		))]
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

		/// Update claim rewards deduction rates for all rewards currencies of specific PoolId
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `updates`: Vec<(PoolId, DecutionRate>)>
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::update_claim_reward_deduction_rates(updates.len() as u32))]
		pub fn update_claim_reward_deduction_rates(
			origin: OriginFor<T>,
			updates: Vec<(PoolId, Rate)>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, deduction_rate) in updates {
				if let PoolId::Dex(currency_id) = pool_id {
					ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidPoolId);
				}
				ClaimRewardDeductionRates::<T>::mutate_exists(pool_id, |maybe_rate| -> DispatchResult {
					let mut v = maybe_rate.unwrap_or_default();
					if deduction_rate != *v.inner() {
						v.try_set(deduction_rate).map_err(|_| Error::<T>::InvalidRate)?;
						Self::deposit_event(Event::ClaimRewardDeductionRateUpdated {
							pool: pool_id,
							deduction_rate,
						});
					}

					if v.inner().is_zero() {
						*maybe_rate = None;
					} else {
						*maybe_rate = Some(v);
					}
					Ok(())
				})?;
			}
			Ok(())
		}

		/// Update claim rewards deduction rates currency
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::update_claim_reward_deduction_currency())]
		pub fn update_claim_reward_deduction_currency(
			origin: OriginFor<T>,
			pool_id: PoolId,
			currency_id: Option<CurrencyId>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			ClaimRewardDeductionCurrency::<T>::mutate_exists(pool_id, |c| *c = currency_id);
			Self::deposit_event(Event::ClaimRewardDeductionCurrencyUpdated {
				pool: pool_id,
				currency: currency_id,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub(crate) fn claim_reward_deduction_rates(pool_id: &PoolId) -> Rate {
		ClaimRewardDeductionRates::<T>::get(pool_id).into_inner()
	}

	// accumulate incentive rewards of multi currencies
	fn accumulate_incentives(pool_id: PoolId) {
		for (reward_currency_id, reward_amount) in IncentiveRewardAmounts::<T>::iter_prefix(pool_id) {
			if reward_amount.is_zero() {
				continue;
			}

			// ignore result so that failure will not block accumulate other type reward for the pool
			let _ =
				Self::transfer_rewards_and_update_records(pool_id, reward_currency_id, reward_amount).map_err(|e| {
					log::warn!(
						target: "incentives",
						"accumulate_incentives: failed to accumulate {:?} {:?} rewards for pool {:?} : {:?}",
						reward_amount, reward_currency_id, pool_id, e
					);
				});
		}
	}

	/// Ensure atomic
	#[transactional]
	fn transfer_rewards_and_update_records(
		pool_id: PoolId,
		reward_currency_id: CurrencyId,
		reward_amount: Balance,
	) -> DispatchResult {
		T::Currency::transfer(
			reward_currency_id,
			&T::RewardsSource::get(),
			&Self::account_id(),
			reward_amount,
		)?;
		<orml_rewards::Pallet<T>>::accumulate_reward(&pool_id, reward_currency_id, reward_amount)?;
		Ok(())
	}

	fn do_claim_rewards(who: T::AccountId, pool_id: PoolId) -> DispatchResult {
		// orml_rewards will claim rewards for all currencies rewards
		<orml_rewards::Pallet<T>>::claim_rewards(&who, &pool_id);

		PendingMultiRewards::<T>::mutate_exists(pool_id, &who, |maybe_pending_multi_rewards| {
			if let Some(pending_multi_rewards) = maybe_pending_multi_rewards {
				let deduction_rate = Self::claim_reward_deduction_rates(&pool_id);
				let deduction_currency = ClaimRewardDeductionCurrency::<T>::get(pool_id);

				for (currency_id, pending_reward) in pending_multi_rewards.iter_mut() {
					if pending_reward.is_zero() {
						continue;
					}

					let deduction_rate = if let Some(deduction_currency) = deduction_currency {
						// only apply deduction rate to specified currency
						if deduction_currency == *currency_id {
							deduction_rate
						} else {
							Zero::zero()
						}
					} else {
						// apply deduction rate to all currencies
						deduction_rate
					};

					let (payout_amount, deduction_amount) = {
						let should_deduction_amount = deduction_rate.saturating_mul_int(*pending_reward);
						(
							pending_reward.saturating_sub(should_deduction_amount),
							should_deduction_amount,
						)
					};

					// payout reward to claimer and re-accumuated reward.
					match Self::payout_reward_and_reaccumulate_reward(
						pool_id,
						&who,
						*currency_id,
						payout_amount,
						deduction_amount,
					) {
						Ok(_) => {
							// update state
							*pending_reward = Zero::zero();

							Self::deposit_event(Event::ClaimRewards {
								who: who.clone(),
								pool: pool_id,
								reward_currency_id: *currency_id,
								actual_amount: payout_amount,
								deduction_amount,
							});
						}
						Err(e) => {
							log::error!(
								target: "incentives",
								"payout_reward_and_reaccumulate_reward: failed to payout {:?} to {:?} and re-accumulate {:?} {:?} to pool {:?}: {:?}",
								payout_amount, who, deduction_amount, currency_id, pool_id, e
							);
						}
					};
				}

				// clear zero value item of BTreeMap
				pending_multi_rewards.retain(|_, v| *v != 0);

				// if pending_multi_rewards is default, clear the storage
				if pending_multi_rewards.is_empty() {
					*maybe_pending_multi_rewards = None;
				}
			}
		});

		Ok(())
	}

	/// Ensure atomic
	#[transactional]
	fn payout_reward_and_reaccumulate_reward(
		pool_id: PoolId,
		who: &T::AccountId,
		reward_currency_id: CurrencyId,
		payout_amount: Balance,
		reaccumulate_amount: Balance,
	) -> DispatchResult {
		if !reaccumulate_amount.is_zero() {
			<orml_rewards::Pallet<T>>::accumulate_reward(&pool_id, reward_currency_id, reaccumulate_amount)?;
		}
		T::Currency::transfer(reward_currency_id, &Self::account_id(), who, payout_amount)?;
		Ok(())
	}
}

impl<T: Config> DEXIncentives<T::AccountId, CurrencyId, Balance> for Pallet<T> {
	fn do_deposit_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);

		T::Currency::transfer(lp_currency_id, who, &Self::account_id(), amount)?;
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Dex(lp_currency_id), amount.unique_saturated_into())?;

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
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Dex(lp_currency_id), amount.unique_saturated_into())?;

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
		Self::claim_reward_deduction_rates(&pool_id)
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
impl<T: Config> Handler<(T::AccountId, CurrencyId, Amount, Balance)> for OnUpdateLoan<T> {
	fn handle(info: &(T::AccountId, CurrencyId, Amount, Balance)) -> DispatchResult {
		let (who, currency_id, adjustment, _previous_amount) = info;
		let adjustment_abs = TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();

		if adjustment.is_positive() {
			<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Loans(*currency_id), adjustment_abs)
		} else {
			<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Loans(*currency_id), adjustment_abs)
		}
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
impl<T: Config> Handler<(T::AccountId, Balance)> for OnEarningBonded<T> {
	fn handle((who, amount): &(T::AccountId, Balance)) -> DispatchResult {
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::Earning(T::NativeCurrencyId::get()), *amount)
	}
}

pub struct OnEarningUnbonded<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Handler<(T::AccountId, Balance)> for OnEarningUnbonded<T> {
	fn handle((who, amount): &(T::AccountId, Balance)) -> DispatchResult {
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::Earning(T::NativeCurrencyId::get()), *amount)
	}
}

pub struct OnNomineesElectionBonded<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Handler<(T::AccountId, Balance)> for OnNomineesElectionBonded<T> {
	fn handle((who, amount): &(T::AccountId, Balance)) -> DispatchResult {
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::NomineesElection, *amount)
	}
}

pub struct OnNomineesElectionUnbonded<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Handler<(T::AccountId, Balance)> for OnNomineesElectionUnbonded<T> {
	fn handle((who, amount): &(T::AccountId, Balance)) -> DispatchResult {
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::NomineesElection, *amount)
	}
}
