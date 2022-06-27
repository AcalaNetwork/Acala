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

//! # Network Fee Distribution & Incentive Pools Module

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	pallet_prelude::*,
	parameter_types,
	traits::{Currency, Imbalance, OnUnbalanced},
	transactional,
};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId, IncomeSource, PoolPercent};
use sp_runtime::{
	traits::{One, Saturating, Zero},
	FixedPointNumber, FixedU128,
};
use sp_std::vec::Vec;
use support::{DEXManager, OnFeeDeposit, SwapLimit};

mod mock;
mod tests;
pub mod weights;
use sp_runtime::traits::UniqueSaturatedInto;
pub use weights::WeightInfo;

pub type NegativeImbalanceOf<T> =
	<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;
pub type Incomes<T> = Vec<(IncomeSource, Vec<(<T as frame_system::Config>::AccountId, u32)>)>;
pub type Treasuries<T> = Vec<(
	<T as frame_system::Config>::AccountId,
	Balance,
	Vec<(<T as frame_system::Config>::AccountId, u32)>,
)>;

/// helper method to create `PoolPercent` list by tuple.
pub fn build_pool_percents<AccountId: Clone>(list: Vec<(AccountId, u32)>) -> Vec<PoolPercent<AccountId>> {
	list.iter()
		.map(|data| PoolPercent {
			pool: data.clone().0,
			rate: FixedU128::saturating_from_rational(data.1, 100),
		})
		.collect()
}

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	parameter_types! {
		pub const MaxPoolSize: u8 = 10;
		pub const MaxTokenSize: u8 = 100;
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		type Currency: Currency<Self::AccountId>;

		type Currencies: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		#[pallet::constant]
		type NativeCurrencyId: Get<CurrencyId>;

		/// Allocation period from treasury to incentive pools.
		#[pallet::constant]
		type AllocationPeriod: Get<Self::BlockNumber>;

		/// DEX to exchange currencies when allocation.
		type DEX: DEXManager<Self::AccountId, Balance, CurrencyId>;

		#[pallet::constant]
		type DexSwapJointList: Get<Vec<Vec<CurrencyId>>>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		InvalidParams,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		IncomeFeeSet {
			income: IncomeSource,
			pools: Vec<PoolPercent<T::AccountId>>,
		},
		TreasuryPoolSet {
			treasury: T::AccountId,
			pools: Vec<PoolPercent<T::AccountId>>,
		},
		IncentiveDistribution {
			treasury: T::AccountId,
			amount: Balance,
		},
	}

	/// Income fee source mapping to different treasury pools.
	///
	/// IncomeToTreasuries: map IncomeSource => Vec<PoolPercent>
	#[pallet::storage]
	#[pallet::getter(fn income_to_treasuries)]
	pub type IncomeToTreasuries<T: Config> =
		StorageMap<_, Twox64Concat, IncomeSource, BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize>, ValueQuery>;

	/// Treasury pool allocation mapping to different income pools.
	/// Only allocation token from treasury pool account to income pool accounts when native token
	/// of treasury pool account is large than threshold.
	///
	/// TreasuryToIncentives: map AccountId => (Balance, Vec<PoolPercent>)
	#[pallet::storage]
	#[pallet::getter(fn treasury_to_incentives)]
	pub type TreasuryToIncentives<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		(Balance, BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize>),
		ValueQuery,
	>;

	/// Treasury pool tokens list.
	///
	/// TreasuryTokens: map AccountId => Vec<CurrencyId>
	#[pallet::storage]
	#[pallet::getter(fn treasury_tokens)]
	pub type TreasuryTokens<T: Config> =
		StorageMap<_, Twox64Concat, T::AccountId, BoundedVec<CurrencyId, MaxTokenSize>, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub incomes: Incomes<T>,
		pub treasuries: Treasuries<T>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				incomes: Default::default(),
				treasuries: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			self.incomes.iter().for_each(|(income, pools)| {
				let pool_rates = build_pool_percents::<T::AccountId>(pools.clone());
				let _ = <Pallet<T>>::do_set_treasury_rate(*income, pool_rates);
			});
			self.treasuries.iter().for_each(|(treasury, threshold, pools)| {
				let pool_rates = build_pool_percents::<T::AccountId>(pools.clone());
				let _ = <Pallet<T>>::do_set_incentive_rate(treasury.clone(), *threshold, pool_rates);
			});
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(now: T::BlockNumber) -> Weight {
			if now % T::AllocationPeriod::get() == Zero::zero() {
				Self::distribute_incentives();
				<T as Config>::WeightInfo::force_transfer_to_incentive()
			} else {
				0
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set how much percentage of income fee go to different treasury pools
		#[pallet::weight(T::WeightInfo::set_income_fee())]
		#[transactional]
		pub fn set_income_fee(
			origin: OriginFor<T>,
			income_source: IncomeSource,
			treasury_pool_rates: Vec<PoolPercent<T::AccountId>>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			Self::do_set_treasury_rate(income_source, treasury_pool_rates)
		}

		/// Set how much percentage of treasury pool go to different incentive pools
		#[pallet::weight(T::WeightInfo::set_treasury_pool())]
		#[transactional]
		pub fn set_treasury_pool(
			origin: OriginFor<T>,
			treasury: T::AccountId,
			threshold: Balance,
			incentive_pools: Vec<PoolPercent<T::AccountId>>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			Self::do_set_incentive_rate(treasury, threshold, incentive_pools)
		}

		/// Force transfer token from treasury pool to incentive pool.
		#[pallet::weight(T::WeightInfo::force_transfer_to_incentive())]
		#[transactional]
		pub fn force_transfer_to_incentive(origin: OriginFor<T>, treasury: T::AccountId) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			Self::distribution_incentive(treasury)
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_set_treasury_rate(
		income: IncomeSource,
		treasury_pool_rates: Vec<PoolPercent<T::AccountId>>,
	) -> DispatchResult {
		ensure!(!treasury_pool_rates.is_empty(), Error::<T>::InvalidParams);
		Self::check_rates(&treasury_pool_rates)?;

		let pool_rates: BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize> = treasury_pool_rates
			.clone()
			.try_into()
			.map_err(|_| Error::<T>::InvalidParams)?;
		IncomeToTreasuries::<T>::try_mutate(income, |maybe_pool_rates| -> DispatchResult {
			*maybe_pool_rates = pool_rates;
			Ok(())
		})?;

		Self::deposit_event(Event::IncomeFeeSet {
			income,
			pools: treasury_pool_rates,
		});
		Ok(())
	}

	pub fn do_set_incentive_rate(
		treasury: T::AccountId,
		threshold: Balance,
		incentive_pool_rates: Vec<PoolPercent<T::AccountId>>,
	) -> DispatchResult {
		ensure!(!incentive_pool_rates.is_empty(), Error::<T>::InvalidParams);
		Self::check_rates(&incentive_pool_rates)?;

		let pool_rates: BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize> = incentive_pool_rates
			.clone()
			.try_into()
			.map_err(|_| Error::<T>::InvalidParams)?;
		TreasuryToIncentives::<T>::try_mutate(&treasury, |(maybe_threshold, maybe_pool_rates)| -> DispatchResult {
			*maybe_pool_rates = pool_rates;
			*maybe_threshold = threshold;
			Ok(())
		})?;

		Self::deposit_event(Event::TreasuryPoolSet {
			treasury,
			pools: incentive_pool_rates,
		});
		Ok(())
	}

	fn check_rates(pool_rates: &[PoolPercent<T::AccountId>]) -> DispatchResult {
		let mut sum = FixedU128::zero();
		pool_rates.iter().for_each(|pool_rate| {
			sum = sum.saturating_add(pool_rate.rate);
		});
		ensure!(One::is_one(&sum), Error::<T>::InvalidParams);
		Ok(())
	}

	/// Distribute/Deposit income to treasury pool account.
	fn distribution_fees(
		pool_rates: BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize>,
		currency_id: CurrencyId,
		amount: Balance,
	) -> DispatchResult {
		ensure!(!pool_rates.is_empty(), Error::<T>::InvalidParams);

		pool_rates.into_iter().for_each(|pool_rate| {
			let treasury_account = pool_rate.pool;
			let amount_to_pool = pool_rate.rate.saturating_mul_int(amount);

			let deposit = T::Currencies::deposit(currency_id, &treasury_account, amount_to_pool);
			if deposit.is_ok() {
				// record token type for treasury account, used when distribute to incentive pools.
				let _ = TreasuryTokens::<T>::try_mutate(&treasury_account, |maybe_tokens| -> DispatchResult {
					if !maybe_tokens.contains(&currency_id) {
						maybe_tokens
							.try_push(currency_id)
							.map_err(|_| Error::<T>::InvalidParams)?;
					}
					Ok(())
				});
			}
		});
		Ok(())
	}

	/// Transfer balance from treasury pool account to incentive pool account.
	fn distribution_incentive(treasury: T::AccountId) -> DispatchResult {
		let native_token = T::NativeCurrencyId::get();
		let tokens = TreasuryTokens::<T>::get(&treasury);
		let (threshold, pool_rates) = TreasuryToIncentives::<T>::get(&treasury);

		let mut total_native: Balance = 0;
		tokens.into_iter().for_each(|token| {
			if let Some(native_amount) = Self::get_native_account(&treasury, native_token, token) {
				total_native = total_native.saturating_add(native_amount);
			}
		});

		// If treasury pool account has small native token, do not distribute to incentive pools.
		if total_native < threshold {
			return Ok(());
		}

		pool_rates.into_iter().for_each(|pool_rate| {
			let treasury_account = pool_rate.pool;
			let amount_to_pool = pool_rate.rate.saturating_mul_int(total_native);

			let _ = T::Currencies::transfer(native_token, &treasury, &treasury_account, amount_to_pool);
		});

		Self::deposit_event(Event::IncentiveDistribution {
			treasury,
			amount: total_native,
		});
		Ok(())
	}

	/// Use dex swap foreign token to native token, then treasury pool account only has native
	/// token.
	fn get_native_account(treasury: &T::AccountId, native_token: CurrencyId, token: CurrencyId) -> Option<Balance> {
		if native_token == token {
			let amount = T::Currency::free_balance(treasury);
			Some(amount.unique_saturated_into())
		} else {
			let amount = T::Currencies::free_balance(token, treasury);
			let limit = SwapLimit::ExactSupply(amount, 0);
			let swap_path = T::DEX::get_best_price_swap_path(token, native_token, limit, T::DexSwapJointList::get());
			if let Some((swap_path, _, _)) = swap_path {
				if let Ok((_, native_amount)) = T::DEX::swap_with_specific_path(treasury, &swap_path, limit) {
					return Some(native_amount);
				}
			}
			None
		}
	}

	fn distribute_incentives() {
		TreasuryToIncentives::<T>::iter_keys().for_each(|treasury| {
			let _ = Self::distribution_incentive(treasury);
		});
	}
}

impl<T: Config + Send + Sync> OnFeeDeposit<T::AccountId, CurrencyId, Balance> for Pallet<T> {
	/// Parameters:
	/// - income: Income source, normally means existing modules.
	/// - account_id: If given account, then the whole fee amount directly deposit to it.
	/// - currency_id: currency type.
	/// - amount: fee amount.
	fn on_fee_deposit(income: IncomeSource, currency_id: CurrencyId, amount: Balance) -> DispatchResult {
		// use `IncomeSource` to distribution fee to different treasury pool based on percentage.
		let pool_rates: BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize> = IncomeToTreasuries::<T>::get(income);
		Pallet::<T>::distribution_fees(pool_rates, currency_id, amount)
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct DistributeTxFees<T: Config + Send + Sync>(PhantomData<T>);

/// Transaction payment fee distribution.
impl<T: Config + Send + Sync> OnUnbalanced<NegativeImbalanceOf<T>> for DistributeTxFees<T> {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalanceOf<T>>) {
		if let Some(mut fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut fees);
			}

			let pool_rates: BoundedVec<PoolPercent<T::AccountId>, MaxPoolSize> =
				IncomeToTreasuries::<T>::get(IncomeSource::TxFee);
			let pool_rates = pool_rates.into_iter().collect::<Vec<_>>();

			if let Some(pool) = pool_rates.get(0) {
				let pool_id: &T::AccountId = &pool.pool;
				let pool_rate: FixedU128 = pool.rate;
				let pool_amount = pool_rate.saturating_mul_int(100u32);
				let amount_other = 100u32.saturating_sub(pool_amount);
				let split = fees.ration(pool_amount, amount_other);
				<T as Config>::Currency::resolve_creating(pool_id, split.0);

				// Current only support at least two treasury pool account for tx fee.
				if let Some(pool) = pool_rates.get(1) {
					let pool_id: &T::AccountId = &pool.pool;
					<T as Config>::Currency::resolve_creating(pool_id, split.1);
				}
			}
		}
	}
}

#[derive(Encode, Decode, Clone, Eq, PartialEq, TypeInfo)]
#[scale_info(skip_type_params(T))]
pub struct DealTxFeesWithAccount<T: Config + Send + Sync, A>(PhantomData<(T, A)>);

/// All transaction fee distribute to treasury pool account.
impl<T: Config + Send + Sync, A: Get<T::AccountId>> OnUnbalanced<NegativeImbalanceOf<T>>
	for DealTxFeesWithAccount<T, A>
{
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalanceOf<T>>) {
		if let Some(mut fees) = fees_then_tips.next() {
			if let Some(tips) = fees_then_tips.next() {
				tips.merge_into(&mut fees);
			}

			// Must resolve into existing but better to be safe.
			T::Currency::resolve_creating(&A::get(), fees);
		}
	}
}
