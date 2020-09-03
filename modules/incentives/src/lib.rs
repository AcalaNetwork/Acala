#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
	decl_module, decl_storage,
	traits::{EnsureOrigin, Get, Happened},
	IterableStorageMap,
};
use orml_traits::{Change, MultiCurrency, RewardHandler};
use orml_utilities::with_transaction_result;
use primitives::{Amount, Balance, CurrencyId, Share};
use sp_runtime::{
	traits::{Saturating, UniqueSaturatedInto, Zero},
	DispatchResult, FixedPointNumber, RuntimeDebug,
};
use sp_std::prelude::*;
use support::{CDPTreasury, Rate};

/// PoolId for various rewards pools
#[derive(Encode, Decode, Copy, Clone, PartialEq, Eq, RuntimeDebug)]
pub enum PoolId {
	/// Loans rewards pool for users who open CDP
	Loans(CurrencyId),
	/// Rewards pool(ACA) for market makers who provide dex liquidity
	DexIncentive(CurrencyId),
	/// Rewards pool(ACA) for liquidators who provide dex liquidity to
	/// participate automatic liquidation
	DexSaving(CurrencyId),
	/// Rewards pool(LDOT) for users who staking by Homa protocol
	LDOT,
}

/// Incentive params
#[derive(Encode, Decode, Clone, RuntimeDebug, PartialEq, Eq, Default)]
pub struct IncentiveParam<BlockNumber, Balance> {
	pub start_block: Option<BlockNumber>,
	pub end_block: Option<BlockNumber>,
	pub liner_adjust_start_block: Option<BlockNumber>,
	pub reward_amount_per_block: Balance,
}

// typedef to help polkadot.js disambiguate Change with different generic
// parameters
type ChangeOptionBlockNumber<T> = Change<Option<<T as frame_system::Trait>::BlockNumber>>;
type ChangeBalance = Change<Balance>;

pub trait Trait: frame_system::Trait + orml_rewards::Trait<Share = Share, Balance = Balance, PoolId = PoolId> {
	/// The vault account to keep rewards for type Loans PoolId
	type LoansIncentivePool: Get<Self::AccountId>;

	/// The vault account to keep rewards for type DexIncentive and DexSaving
	/// PoolId
	type DexIncentivePool: Get<Self::AccountId>;

	/// The vault account to keep rewards for type LDOT PoolId
	type LDOTIncentivePool: Get<Self::AccountId>;

	/// The period to accumulate rewards
	type AccumulatePeriod: Get<Self::BlockNumber>;

	/// The incentive currency type (should be ACA)
	type IncentiveCurrencyId: Get<CurrencyId>;

	/// The saving reward currency type (should be AUSD)
	type SavingCurrencyId: Get<CurrencyId>;

	/// The origin which may update incentives rate
	type UpdateOrigin: EnsureOrigin<Self::Origin>;

	/// CDP treasury to issue rewards in AUSD
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Currency for transfer/issue rewards in other tokens except AUSD
	type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
}

decl_storage! {
	trait Store for Module<T: Trait> as CDPEngine {
		/// Mapping from reward pool to its incentive params
		pub IncentiveParams get(fn incentive_params): map hasher(twox_64_concat) PoolId => IncentiveParam<T::BlockNumber, Balance>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		/// The vault account to keep rewards for type Loans PoolId
		const LoansIncentivePool: T::AccountId = T::LoansIncentivePool::get();

		/// The vault account to keep rewards for type DexIncentive and DexSaving PoolId
		const DexIncentivePool: T::AccountId = T::DexIncentivePool::get();

		/// The vault account to keep rewards for type LDOT PoolId
		const LDOTIncentivePool: T::AccountId = T::LDOTIncentivePool::get();

		/// The period to accumulate rewards
		const AccumulatePeriod: T::BlockNumber = T::AccumulatePeriod::get();

		/// The incentive currency type (should be ACA)
		const IncentiveCurrencyId: CurrencyId = T::IncentiveCurrencyId::get();

		/// The saving reward currency type (should be AUSD)
		const SavingCurrencyId: CurrencyId = T::SavingCurrencyId::get();

		#[weight = 10_000]
		pub fn set_incentive_param(
			origin,
			pool_id: PoolId,
			start_block: ChangeOptionBlockNumber<T>,
			end_block: ChangeOptionBlockNumber<T>,
			liner_adjust_start_block: ChangeOptionBlockNumber<T>,
			reward_amount_per_block: ChangeBalance,
		) {
			with_transaction_result(|| {
				T::UpdateOrigin::ensure_origin(origin)?;
				IncentiveParams::<T>::try_mutate(pool_id, |param| -> DispatchResult {
					if let Change::NewValue(update) = start_block {
						param.start_block = update;
					}
					if let Change::NewValue(update) = end_block {
						param.end_block = update;
					}
					if let Change::NewValue(update) = liner_adjust_start_block {
						param.liner_adjust_start_block = update;
					}
					if let Change::NewValue(update) = reward_amount_per_block {
						param.reward_amount_per_block = update;
					}
					Ok(())
				})
			})?;
		}
	}
}

pub struct OnAddLiquidity<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> Happened<(T::AccountId, CurrencyId, Share)> for OnAddLiquidity<T> {
	fn happened(info: &(T::AccountId, CurrencyId, Share)) {
		let (who, currency_id, increase_share) = info;
		<orml_rewards::Module<T>>::add_share(who, PoolId::DexIncentive(*currency_id), *increase_share);
		<orml_rewards::Module<T>>::add_share(who, PoolId::DexSaving(*currency_id), *increase_share);
	}
}

pub struct OnRemoveLiquidity<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> Happened<(T::AccountId, CurrencyId, Share)> for OnRemoveLiquidity<T> {
	fn happened(info: &(T::AccountId, CurrencyId, Share)) {
		let (who, currency_id, decrease_share) = info;
		<orml_rewards::Module<T>>::remove_share(who, PoolId::DexIncentive(*currency_id), *decrease_share);
		<orml_rewards::Module<T>>::remove_share(who, PoolId::DexSaving(*currency_id), *decrease_share);
	}
}

pub struct OnUpdateLoan<T>(sp_std::marker::PhantomData<T>);
impl<T: Trait> Happened<(T::AccountId, CurrencyId, Amount, Balance)> for OnUpdateLoan<T> {
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

			<orml_rewards::Module<T>>::set_share(who, PoolId::Loans(*currency_id), new_share_amount);
		}
	}
}

impl<T: Trait> Module<T>
where
	<T as frame_system::Trait>::BlockNumber: sp_runtime::FixedPointOperand,
{
	fn calculate_accumulate_reward_amount(pool_id: PoolId, now: T::BlockNumber) -> Balance {
		let accumulate_period = T::AccumulatePeriod::get();
		if now % accumulate_period != Zero::zero() {
			return Zero::zero();
		}

		let param = Self::incentive_params(pool_id);

		if let Some(start_block) = param.start_block {
			if now <= start_block {
				return Zero::zero();
			}
		}

		let mut reward_amount_per_block = param.reward_amount_per_block;

		if let Some(end_block) = param.end_block {
			if now > end_block {
				return Zero::zero();
			}

			if let Some(liner_adjust_start_block) = param.liner_adjust_start_block {
				let multiplier = Rate::saturating_from_integer(1).saturating_sub(
					Rate::checked_from_rational(
						now.saturating_sub(liner_adjust_start_block),
						end_block.saturating_sub(liner_adjust_start_block),
					)
					.unwrap_or_default(),
				);

				reward_amount_per_block = multiplier.saturating_mul_int(reward_amount_per_block);
			}
		}

		reward_amount_per_block.saturating_mul(accumulate_period.unique_saturated_into())
	}
}

impl<T: Trait> RewardHandler<T::AccountId, T::BlockNumber> for Module<T>
where
	<T as frame_system::Trait>::BlockNumber: sp_runtime::FixedPointOperand,
{
	type Share = Share;
	type Balance = Balance;
	type PoolId = PoolId;

	fn accumulate_reward(now: T::BlockNumber, callback: impl Fn(PoolId, Balance)) -> Balance {
		for (pool_id, _) in orml_rewards::Pools::<T>::iter() {
			let reward_amount = Self::calculate_accumulate_reward_amount(pool_id, now);

			if !reward_amount.is_zero() {
				if match pool_id {
					PoolId::Loans(_) => {
						// TODO: transfer from RESERVED TREASURY instead of issuing
						T::Currency::deposit(
							T::IncentiveCurrencyId::get(),
							&T::LoansIncentivePool::get(),
							reward_amount,
						)
					}
					PoolId::DexIncentive(_) => {
						// TODO: transfer from RESERVED TREASURY instead of issuing
						T::Currency::deposit(
							T::IncentiveCurrencyId::get(),
							&T::DexIncentivePool::get(),
							reward_amount,
						)
					}
					PoolId::DexSaving(_) => {
						T::CDPTreasury::issue_debit(&T::DexIncentivePool::get(), reward_amount, false)
					}
					PoolId::LDOT => {
						// TODO: transfer from RESERVED TREASURY instead of issuing
						T::Currency::deposit(
							T::IncentiveCurrencyId::get(),
							&T::LDOTIncentivePool::get(),
							reward_amount,
						)
					}
				}
				.is_ok()
				{
					callback(pool_id, reward_amount);
				}
			}
		}

		// TODO: accumulated rewards are deferent token types
		Zero::zero()
	}

	fn payout(who: &T::AccountId, pool_id: PoolId, amount: Balance) {
		let (pool_account, currency_id) = match pool_id {
			PoolId::Loans(_) => (T::LoansIncentivePool::get(), T::IncentiveCurrencyId::get()),
			PoolId::DexIncentive(_) => (T::DexIncentivePool::get(), T::IncentiveCurrencyId::get()),
			PoolId::DexSaving(_) => (T::DexIncentivePool::get(), T::SavingCurrencyId::get()),
			PoolId::LDOT => (T::LDOTIncentivePool::get(), T::IncentiveCurrencyId::get()),
		};

		// ignore result
		let _ = T::Currency::transfer(currency_id, &pool_account, &who, amount);
	}
}
