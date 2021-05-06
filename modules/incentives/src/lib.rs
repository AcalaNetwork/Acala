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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use frame_support::{log, pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::{Happened, MultiCurrency, RewardHandler};
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, MaybeDisplay, UniqueSaturatedInto, Zero},
	DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::{fmt::Debug, vec::Vec};
use support::{CDPTreasury, DEXIncentives, DEXManager, EmergencyShutdown, Rate};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

/// PoolId for various rewards pools
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PoolId<AccountId> {
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

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ orml_rewards::Config<Share = Balance, Balance = Balance, PoolId = PoolId<Self::RelaychainAccountId>>
	{
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The type of validator account id on relaychain.
		type RelaychainAccountId: Parameter + Member + MaybeSerializeDeserialize + Debug + MaybeDisplay + Ord + Default;

		/// The period to accumulate rewards
		#[pallet::constant]
		type AccumulatePeriod: Get<Self::BlockNumber>;

		/// The reward type for incentive.
		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// The reward type for dex saving.
		#[pallet::constant]
		type StableCurrencyId: Get<CurrencyId>;

		/// The reward type for homa validator insurance
		#[pallet::constant]
		type LiquidCurrencyId: Get<CurrencyId>;

		/// The source account for native token rewards.
		#[pallet::constant]
		type NativeRewardsSource: Get<Self::AccountId>;

		/// The vault account to keep rewards.
		#[pallet::constant]
		type RewardsVaultAccountId: Get<Self::AccountId>;

		/// The origin which may update incentive related params
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// CDP treasury to issue rewards in stable token
		type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

		/// Currency for transfer/issue assets
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// DEX to supply liquidity info
		type DEX: DEXManager<Self::AccountId, CurrencyId, Balance>;

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
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Deposit DEX share. \[who, dex_share_type, deposit_amount\]
		DepositDexShare(T::AccountId, CurrencyId, Balance),
		/// Withdraw DEX share. \[who, dex_share_type, withdraw_amount\]
		WithdrawDexShare(T::AccountId, CurrencyId, Balance),
		/// Claim rewards. \[who, pool_id\]
		ClaimRewards(T::AccountId, PoolId<T::RelaychainAccountId>),
	}

	/// Mapping from pool to its fixed reward amount per period.
	///
	/// IncentiveRewardAmount: map PoolId => Balance
	#[pallet::storage]
	#[pallet::getter(fn incentive_reward_amount)]
	pub type IncentiveRewardAmount<T: Config> =
		StorageMap<_, Twox64Concat, PoolId<T::RelaychainAccountId>, Balance, ValueQuery>;

	/// Mapping from pool to its fixed reward rate per period.
	///
	/// DexSavingRewardRate: map PoolId => Rate
	#[pallet::storage]
	#[pallet::getter(fn dex_saving_reward_rate)]
	pub type DexSavingRewardRate<T: Config> =
		StorageMap<_, Twox64Concat, PoolId<T::RelaychainAccountId>, Rate, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			// accumulate reward periodically
			if !T::EmergencyShutdown::is_shutdown() && now % T::AccumulatePeriod::get() == Zero::zero() {
				let mut count: u32 = 0;
				let native_currency_id = T::NativeCurrencyId::get();
				let stable_currency_id = T::StableCurrencyId::get();

				for (pool_id, pool_info) in orml_rewards::Pools::<T>::iter() {
					if !pool_info.total_shares.is_zero() {
						match pool_id {
							PoolId::LoansIncentive(_) | PoolId::DexIncentive(_) | PoolId::HomaIncentive => {
								count += 1;
								let incentive_reward_amount = Self::incentive_reward_amount(pool_id.clone());

								if !incentive_reward_amount.is_zero() {
									let res = T::Currency::transfer(
										native_currency_id,
										&T::NativeRewardsSource::get(),
										&T::RewardsVaultAccountId::get(),
										incentive_reward_amount,
									);
									match res {
										Ok(_) => {
											<orml_rewards::Pallet<T>>::accumulate_reward(
												&pool_id,
												incentive_reward_amount,
											);
										}
										Err(e) => {
											log::warn!(
												target: "incentives",
												"transfer: failed to transfer {:?} {:?} from {:?} to {:?}: {:?}. \
												This is unexpected but should be safe",
												incentive_reward_amount, native_currency_id, T::NativeRewardsSource::get(), T::RewardsVaultAccountId::get(), e
											);
										}
									}
								}
							}

							PoolId::DexSaving(lp_currency_id) => {
								count += 1;
								let dex_saving_reward_rate = Self::dex_saving_reward_rate(pool_id.clone());

								if !dex_saving_reward_rate.is_zero() {
									if let Some((currency_id_a, currency_id_b)) =
										lp_currency_id.split_dex_share_currency_id()
									{
										// accumulate saving reward only for liquidity pool of stable currency id
										let dex_saving_reward_base = if currency_id_a == stable_currency_id {
											T::DEX::get_liquidity_pool(stable_currency_id, currency_id_b).0
										} else if currency_id_b == stable_currency_id {
											T::DEX::get_liquidity_pool(stable_currency_id, currency_id_a).0
										} else {
											Zero::zero()
										};
										let dex_saving_reward_amount =
											dex_saving_reward_rate.saturating_mul_int(dex_saving_reward_base);

										// issue stable coin without backing.
										if !dex_saving_reward_amount.is_zero() {
											let res = T::CDPTreasury::issue_debit(
												&T::RewardsVaultAccountId::get(),
												dex_saving_reward_amount,
												false,
											);
											match res {
												Ok(_) => {
													<orml_rewards::Pallet<T>>::accumulate_reward(
														&pool_id,
														dex_saving_reward_amount,
													);
												}
												Err(e) => {
													log::warn!(
														target: "incentives",
														"issue_debit: failed to issue {:?} unbacked stable to {:?}: {:?}. \
														This is unexpected but should be safe",
														dex_saving_reward_amount, T::RewardsVaultAccountId::get(), e
													);
												}
											}
										}
									}
								}
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
		#[pallet::weight(<T as Config>::WeightInfo::deposit_dex_share())]
		#[transactional]
		pub fn deposit_dex_share(
			origin: OriginFor<T>,
			lp_currency_id: CurrencyId,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_deposit_dex_share(&who, lp_currency_id, amount)?;
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::withdraw_dex_share())]
		#[transactional]
		pub fn withdraw_dex_share(
			origin: OriginFor<T>,
			lp_currency_id: CurrencyId,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			Self::do_withdraw_dex_share(&who, lp_currency_id, amount)?;
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		#[transactional]
		pub fn claim_rewards(
			origin: OriginFor<T>,
			pool_id: PoolId<T::RelaychainAccountId>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			<orml_rewards::Pallet<T>>::claim_rewards(&who, &pool_id);
			Self::deposit_event(Event::ClaimRewards(who, pool_id));
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::update_incentive_rewards(updates.len() as u32))]
		#[transactional]
		pub fn update_incentive_rewards(
			origin: OriginFor<T>,
			updates: Vec<(PoolId<T::RelaychainAccountId>, Balance)>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, amount) in updates {
				match pool_id {
					PoolId::DexIncentive(currency_id) => {
						ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
					}
					PoolId::LoansIncentive(_) | PoolId::HomaIncentive => {}
					_ => {
						return Err(Error::<T>::InvalidPoolId.into());
					}
				}

				IncentiveRewardAmount::<T>::insert(pool_id, amount);
			}
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::update_dex_saving_rewards(updates.len() as u32))]
		#[transactional]
		pub fn update_dex_saving_rewards(
			origin: OriginFor<T>,
			updates: Vec<(PoolId<T::RelaychainAccountId>, Rate)>,
		) -> DispatchResultWithPostInfo {
			T::UpdateOrigin::ensure_origin(origin)?;
			for (pool_id, rate) in updates {
				match pool_id {
					PoolId::DexSaving(currency_id) => {
						ensure!(currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
					}
					_ => {
						return Err(Error::<T>::InvalidPoolId.into());
					}
				}
				DexSavingRewardRate::<T>::insert(pool_id, rate);
			}
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::add_allowance())]
		#[transactional]
		pub fn add_allowance(
			origin: OriginFor<T>,
			pool_id: PoolId<T::RelaychainAccountId>,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			match pool_id {
				PoolId::HomaValidatorAllowance(_) => {
					T::Currency::transfer(
						T::LiquidCurrencyId::get(),
						&who,
						&T::RewardsVaultAccountId::get(),
						amount,
					)?;
					<orml_rewards::Pallet<T>>::accumulate_reward(&pool_id, amount);
				}
				_ => {
					return Err(Error::<T>::InvalidPoolId.into());
				}
			}

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}
}

impl<T: Config> DEXIncentives<T::AccountId, CurrencyId, Balance> for Pallet<T> {
	fn do_deposit_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);

		T::Currency::transfer(lp_currency_id, who, &Self::account_id(), amount)?;
		<orml_rewards::Pallet<T>>::add_share(
			who,
			&PoolId::DexIncentive(lp_currency_id),
			amount.unique_saturated_into(),
		);
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::DexSaving(lp_currency_id), amount);

		Self::deposit_event(Event::DepositDexShare(who.clone(), lp_currency_id, amount));
		Ok(())
	}

	fn do_withdraw_dex_share(who: &T::AccountId, lp_currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		ensure!(lp_currency_id.is_dex_share_currency_id(), Error::<T>::InvalidCurrencyId);
		ensure!(
			<orml_rewards::Pallet<T>>::share_and_withdrawn_reward(&PoolId::DexIncentive(lp_currency_id), &who).0
				>= amount && <orml_rewards::Pallet<T>>::share_and_withdrawn_reward(
				&PoolId::DexSaving(lp_currency_id),
				&who
			)
			.0 >= amount,
			Error::<T>::NotEnough,
		);

		T::Currency::transfer(lp_currency_id, &Self::account_id(), &who, amount)?;
		<orml_rewards::Pallet<T>>::remove_share(
			who,
			&PoolId::DexIncentive(lp_currency_id),
			amount.unique_saturated_into(),
		);
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::DexSaving(lp_currency_id), amount);

		Self::deposit_event(Event::WithdrawDexShare(who.clone(), lp_currency_id, amount));
		Ok(())
	}
}

pub struct OnUpdateLoan<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, CurrencyId, Amount, Balance)> for OnUpdateLoan<T> {
	fn happened(info: &(T::AccountId, CurrencyId, Amount, Balance)) {
		let (who, currency_id, adjustment, previous_amount) = info;
		let adjustment_abs =
			sp_std::convert::TryInto::<Balance>::try_into(adjustment.saturating_abs()).unwrap_or_default();

		if !adjustment_abs.is_zero() {
			let new_share_amount = if adjustment.is_positive() {
				previous_amount.saturating_add(adjustment_abs)
			} else {
				previous_amount.saturating_sub(adjustment_abs)
			};

			<orml_rewards::Pallet<T>>::set_share(who, &PoolId::LoansIncentive(*currency_id), new_share_amount);
		}
	}
}

pub struct OnIncreaseGuarantee<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, T::RelaychainAccountId, Balance)> for OnIncreaseGuarantee<T> {
	fn happened(info: &(T::AccountId, T::RelaychainAccountId, Balance)) {
		let (who, validator, increment) = info;
		<orml_rewards::Pallet<T>>::add_share(who, &PoolId::HomaValidatorAllowance(validator.clone()), *increment);
	}
}

pub struct OnDecreaseGuarantee<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Happened<(T::AccountId, T::RelaychainAccountId, Balance)> for OnDecreaseGuarantee<T> {
	fn happened(info: &(T::AccountId, T::RelaychainAccountId, Balance)) {
		let (who, validator, decrement) = info;
		<orml_rewards::Pallet<T>>::remove_share(who, &PoolId::HomaValidatorAllowance(validator.clone()), *decrement);
	}
}

impl<T: Config> RewardHandler<T::AccountId> for Pallet<T> {
	type Balance = Balance;
	type PoolId = PoolId<T::RelaychainAccountId>;

	fn payout(who: &T::AccountId, pool_id: &Self::PoolId, amount: Self::Balance) {
		let currency_id = match pool_id {
			PoolId::LoansIncentive(_) | PoolId::DexIncentive(_) | PoolId::HomaIncentive => T::NativeCurrencyId::get(),
			PoolId::DexSaving(_) => T::StableCurrencyId::get(),
			PoolId::HomaValidatorAllowance(_) => T::LiquidCurrencyId::get(),
		};

		// payout the reward to user from the pool. it should not affect the
		// process, ignore the result to continue. if it fails, just the user will not
		// be rewarded, there will not increase user balance.
		let res = T::Currency::transfer(currency_id, &T::RewardsVaultAccountId::get(), &who, amount);
		if let Err(e) = res {
			log::warn!(
				target: "incentives",
				"transfer: failed to transfer {:?} {:?} from {:?} to {:?}: {:?}. \
				This is unexpected but should be safe",
				amount, currency_id, T::RewardsVaultAccountId::get(), who, e
			);
			debug_assert!(false);
		}
	}
}
