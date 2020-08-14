//! # DEX Module
//!
//! ## Overview
//!
//! Built-in decentralized exchange modules in Acala network, the core currency
//! type of trading pairs is stable currency (aUSD), the trading mechanism
//! refers to the design of Uniswap. In addition to being used for trading, DEX
//! also participates in CDP liquidation, which is faster than liquidation by
//! auction when the liquidity is sufficient. And providing market making
//! liquidity for DEX will also receive stable currency as additional reward for
//! its participation in the CDP liquidation.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get},
	weights::{constants::WEIGHT_PER_MICROS, DispatchClass, Weight},
	Parameter,
};
use frame_system::{self as system, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32Bit, CheckedAdd, CheckedMul, CheckedSub, MaybeSerializeDeserialize, Member, One,
		Saturating, UniqueSaturatedInto, Zero,
	},
	DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand, ModuleId,
};
use sp_std::prelude::Vec;
use support::{CDPTreasury, DEXManager, EmergencyShutdown, Price, Rate, Ratio};

mod benchmarking;
mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Associate type for measuring liquidity contribution of specific trading
	/// pairs
	type Share: Parameter + Member + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize + FixedPointOperand;

	/// The origin which may update parameters of dex. Root can always do this.
	type UpdateOrigin: EnsureOrigin<Self::Origin>;

	/// Currency for transfer currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// CDP treasury for depositing additional liquidity reward to DEX
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Allowed trading currency type list, each currency type forms a trading
	/// pair with the base currency
	type EnabledCurrencyIds: Get<Vec<CurrencyId>>;

	/// The base currency as the core currency in all trading pairs
	type GetBaseCurrencyId: Get<CurrencyId>;

	/// Trading fee rate
	type GetExchangeFee: Get<Rate>;

	/// The DEX's module id, keep all assets in DEX.
	type ModuleId: Get<ModuleId>;

	/// Emergency shutdown.
	type EmergencyShutdown: EmergencyShutdown;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Share,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Add liquidity success. [who, currency_type, added_currency_amount, added_base_currency_amount, increment_share_amount]
		AddLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		/// Withdraw liquidity from the trading pool success. [who, currency_type, withdrawn_currency_amount, withdrawn_base_currency_amount, burned_share_amount]
		WithdrawLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		/// Use supply currency to swap target currency. [trader, supply_currency_type, supply_currency_amount, target_currency_type, target_currency_amount]
		Swap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
		/// Incentive reward rate updated. [currency_type, new_rate]
		LiquidityIncentiveRateUpdated(CurrencyId, Rate),
		/// Incentive interest claimed. [who, currency_type, amount]
		IncentiveInterestClaimed(AccountId, CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for dex module.
	pub enum Error for Module<T: Trait> {
		/// Not the tradable currency type
		CurrencyIdNotAllowed,
		/// Share amount is not enough
		ShareNotEnough,
		/// Share amount overflow
		SharesOverflow,
		/// The actual transaction price will be lower than the acceptable price
		UnacceptablePrice,
		/// The increment of liquidity is invalid
		InvalidLiquidityIncrement,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		/// Liquidity pool, which is the trading pair for specific currency type to base currency type.
		/// CurrencyType -> (OtherCurrencyAmount, BaseCurrencyAmount)
		LiquidityPool get(fn liquidity_pool): map hasher(twox_64_concat) CurrencyId => (Balance, Balance);

		/// Total shares amount of liquidity pool specified by currency type
		/// CurrencyType -> TotalSharesAmount
		TotalShares get(fn total_shares): map hasher(twox_64_concat) CurrencyId => T::Share;

		/// Shares records indexed by currency type and account id
		/// CurrencyType -> Owner -> ShareAmount
		Shares get(fn shares): double_map hasher(twox_64_concat) CurrencyId, hasher(twox_64_concat) T::AccountId => T::Share;

		/// Incentive reward rate for different currency type
		/// CurrencyType -> IncentiveRate
		LiquidityIncentiveRate get(fn liquidity_incentive_rate): map hasher(twox_64_concat) CurrencyId => Rate;

		/// Total interest(include total withdrawn) and total withdrawn interest for different currency type
		/// CurrencyType -> (TotalInterest, TotalWithdrawnInterest)
		TotalInterest get(fn total_interest): map hasher(twox_64_concat) CurrencyId => (Balance, Balance);

		/// Withdrawn interest indexed by currency type and account id
		/// CurrencyType -> Owner -> WithdrawnInterest
		WithdrawnInterest get(fn withdrawn_interest): double_map hasher(twox_64_concat) CurrencyId, hasher(twox_64_concat) T::AccountId => Balance;
	}

	add_extra_genesis {
		config(liquidity_incentive_rate): Vec<(CurrencyId, Rate)>;
		build(|config: &GenesisConfig| {
			config.liquidity_incentive_rate.iter().for_each(| (currency_id, liquidity_incentive_rate) | {
				LiquidityIncentiveRate::insert(currency_id, liquidity_incentive_rate);
			});
		});
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Tradable currency type list
		const EnabledCurrencyIds: Vec<CurrencyId> = T::EnabledCurrencyIds::get();

		/// Base currency type id
		const GetBaseCurrencyId: CurrencyId = T::GetBaseCurrencyId::get();

		/// Trading fee rate
		const GetExchangeFee: Rate = T::GetExchangeFee::get();

		/// The DEX's module id, keep all assets in DEX.
		const ModuleId: ModuleId = T::ModuleId::get();

		/// Update liquidity incentive rate of specific liquidity pool
		///
		/// The dispatch origin of this call must be `UpdateOrigin`.
		///
		/// - `currency_id`: currency type to determine the type of liquidity pool.
		/// - `liquidity_incentive_rate`: liquidity incentive rate.
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: 0
		/// - Db writes: 1
		/// -------------------
		/// Base Weight: 24.92 µs
		/// # </weight>
		#[weight = (25 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(0, 1), DispatchClass::Operational)]
		pub fn set_liquidity_incentive_rate(
			origin,
			currency_id: CurrencyId,
			liquidity_incentive_rate: Rate,
		) {
			with_transaction_result(|| {
				T::UpdateOrigin::ensure_origin(origin)?;
				LiquidityIncentiveRate::insert(currency_id, liquidity_incentive_rate);
				Self::deposit_event(RawEvent::LiquidityIncentiveRateUpdated(currency_id, liquidity_incentive_rate));
				Ok(())
			})?;
		}

		/// Just withdraw liquidity incentive interest as the additional reward for liquidity contribution
		///
		/// - `currency_id`: currency type to determine the type of liquidity pool.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		///		- T::CDPTreasury is module_cdp_treasury
		/// - Complexity: `O(1)`
		/// - Db reads: 8
		/// - Db writes: 4
		/// -------------------
		/// Base Weight: 143.4 µs
		/// # </weight>
		#[weight = 143 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(8, 4)]
		pub fn withdraw_incentive_interest(origin, currency_id: CurrencyId) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				Self::claim_interest(currency_id, &who)?;
				Ok(())
			})?;
		}

		/// Trading with DEX, swap supply currency to target currency
		///
		/// - `supply_currency_id`: supply currency type.
		/// - `supply_amount`: supply currency amount.
		/// - `target_currency_id`: target currency type.
		/// - `acceptable_target_amount`: acceptable target amount, if actual amount is under it, swap will not happen
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		/// - Complexity: `O(1)`
		/// - Db reads:
		///		- swap base to other: 8
		///		- swap other to base: 8
		///		- swap other to other: 9
		/// - Db writes:
		///		- swap base to other: 5
		///		- swap other to base: 5
		///		- swap other to other: 6
		/// -------------------
		/// Base Weight:
		///		- swap base to other: 192.1 µs
		///		- swap other to base: 175.8 µs
		///		- swap other to other: 199.7 µs
		/// # </weight>
		#[weight = 200 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(9, 6)]
		pub fn swap_currency(
			origin,
			supply_currency_id: CurrencyId,
			#[compact] supply_amount: Balance,
			target_currency_id: CurrencyId,
			#[compact] acceptable_target_amount: Balance,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				Self::do_exchange(&who, supply_currency_id, supply_amount, target_currency_id, acceptable_target_amount)?;
				Ok(())
			})?;
		}

		/// Injecting liquidity to specific liquidity pool in the form of depositing currencies in trading pairs
		/// into liquidity pool, and issue shares in proportion to the caller. Shares are temporarily not
		/// allowed to transfer and trade, it represents the proportion of assets in liquidity pool.
		///
		/// - `other_currency_id`: currency type to determine the type of liquidity pool.
		/// - `max_other_currency_amount`: maximum currency amount allowed to inject to liquidity pool.
		/// - `max_base_currency_amount`: maximum base currency(stable currency) amount allowed to inject to liquidity pool.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		/// - Complexity: `O(1)`
		/// - Db reads:
		///		- best case: 9
		///		- worst case: 10
		/// - Db writes:
		///		- best case: 7
		///		- worst case: 9
		/// -------------------
		/// Base Weight:
		///		- best case: 177.6 µs
		///		- worst case: 205.7 µs
		/// # </weight>
		#[weight = 206 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(10, 9)]
		pub fn add_liquidity(
			origin,
			other_currency_id: CurrencyId,
			#[compact] max_other_currency_amount: Balance,
			#[compact] max_base_currency_amount: Balance,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let base_currency_id = T::GetBaseCurrencyId::get();
				ensure!(
					T::EnabledCurrencyIds::get().contains(&other_currency_id),
					Error::<T>::CurrencyIdNotAllowed,
				);

				let total_shares = Self::total_shares(other_currency_id);
				let (other_currency_increment, base_currency_increment, share_increment): (Balance, Balance, T::Share) =
				if total_shares.is_zero() {
					// initialize this liquidity pool, the initial share is equal to the max value between base currency amount and other currency amount
					let initial_share: T::Share = sp_std::cmp::max(max_other_currency_amount, max_base_currency_amount).unique_saturated_into();

					(max_other_currency_amount, max_base_currency_amount, initial_share)
				} else {
					let (other_currency_pool, base_currency_pool): (Balance, Balance) = Self::liquidity_pool(other_currency_id);
					let other_base_price = Price::checked_from_rational(base_currency_pool, other_currency_pool).unwrap_or_default();
					let input_other_base_price = Price::checked_from_rational(max_base_currency_amount, max_other_currency_amount).unwrap_or_default();

					if input_other_base_price <= other_base_price {
						// max_other_currency_amount may be too much, calculate the actual other currency amount
						let base_other_price = Price::checked_from_rational(other_currency_pool, base_currency_pool).unwrap_or_default();
						let other_currency_amount = base_other_price.saturating_mul_int(max_base_currency_amount);
						let share = Ratio::checked_from_rational(other_currency_amount, other_currency_pool)
							.and_then(|n| n.checked_mul_int(total_shares))
							.unwrap_or_default();
						(other_currency_amount, max_base_currency_amount, share)
					} else {
						// max_base_currency_amount is too much, calculate the actual base currency amount
						let base_currency_amount = other_base_price.saturating_mul_int(max_other_currency_amount);
						let share = Ratio::checked_from_rational(base_currency_amount, base_currency_pool)
							.and_then(|n| n.checked_mul_int(total_shares))
							.unwrap_or_default();
						(max_other_currency_amount, base_currency_amount, share)
					}
				};

				ensure!(
					!share_increment.is_zero() && !other_currency_increment.is_zero() && !base_currency_increment.is_zero(),
					Error::<T>::InvalidLiquidityIncrement,
				);

				T::Currency::transfer(other_currency_id, &who, &Self::account_id(), other_currency_increment)?;
				T::Currency::transfer(base_currency_id, &who, &Self::account_id(), base_currency_increment)?;
				Self::deposit_calculate_interest(other_currency_id, &who, share_increment);
				<TotalShares<T>>::try_mutate(other_currency_id, |total_shares| -> DispatchResult {
					*total_shares = total_shares.checked_add(&share_increment).ok_or(Error::<T>::SharesOverflow)?;
					Ok(())
				})?;
				<Shares<T>>::mutate(other_currency_id, &who, |share|
					*share = share.checked_add(&share_increment).expect("share cannot overflow if `total_shares` doesn't; qed")
				);
				LiquidityPool::mutate(other_currency_id, |(other, base)| {
					*other = other.saturating_add(other_currency_increment);
					*base = base.saturating_add(base_currency_increment);
				});

				Self::deposit_event(RawEvent::AddLiquidity(
					who,
					other_currency_id,
					other_currency_increment,
					base_currency_increment,
					share_increment,
				));
				Ok(())
			})?;
		}

		/// Withdraw liquidity from specific liquidity pool in the form of burning shares, and withdrawing currencies in trading pairs
		/// from liquidity pool in proportion, and withdraw liquidity incentive interest.
		///
		/// - `currency_id`: currency type to determine the type of liquidity pool.
		/// - `share_amount`: share amount to burn.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		/// - Complexity: `O(1)`
		/// - Db reads: 11
		/// - Db writes: 9
		/// -------------------
		/// Base Weight:
		///		- best case: 240.1 µs
		///		- worst case: 248.2 µs
		/// # </weight>
		#[weight = 248 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(11, 9)]
		pub fn withdraw_liquidity(origin, currency_id: CurrencyId, #[compact] share_amount: T::Share) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				if share_amount.is_zero() { return Ok(()); }
				ensure!(
					T::EnabledCurrencyIds::get().contains(&currency_id),
					Error::<T>::CurrencyIdNotAllowed,
				);
				let (other_currency_pool, base_currency_pool): (Balance, Balance) = Self::liquidity_pool(currency_id);
				let proportion = Ratio::checked_from_rational(share_amount, Self::total_shares(currency_id)).unwrap_or_default();
				let withdraw_other_currency_amount = proportion.saturating_mul_int(other_currency_pool);
				let withdraw_base_currency_amount = proportion.saturating_mul_int(base_currency_pool);
				T::Currency::transfer(currency_id, &Self::account_id(), &who, withdraw_other_currency_amount)?;
				T::Currency::transfer(T::GetBaseCurrencyId::get(), &Self::account_id(), &who, withdraw_base_currency_amount)?;
				Self::withdraw_calculate_interest(currency_id, &who, share_amount)?;
				<Shares<T>>::try_mutate(currency_id, &who, |share| -> DispatchResult {
					*share = share.checked_sub(&share_amount).ok_or(Error::<T>::ShareNotEnough)?;
					Ok(())
				})?;
				<TotalShares<T>>::mutate(currency_id, |share|
					*share = share.checked_sub(&share_amount).expect("total share cannot underflow if share doesn't; qed")
				);
				LiquidityPool::mutate(currency_id, |(other, base)| {
					*other = other.saturating_sub(withdraw_other_currency_amount);
					*base = base.saturating_sub(withdraw_base_currency_amount);
				});

				Self::deposit_event(RawEvent::WithdrawLiquidity(
					who,
					currency_id,
					withdraw_other_currency_amount,
					withdraw_base_currency_amount,
					share_amount,
				));
				Ok(())
			})?;
		}

		/// Accumulate liquidity incentive interest to respective reward pool when block end
		///
		/// # <weight>
		/// - Complexity: `O(N)` where `N` is the number of currency_ids
		/// - Db reads: `IsShutdown`, `TotalInterest`, 2 items in cdp_treasury
		///	- Db writes: `TotalInterest`, 2 items in cdp_treasury
		/// - Db reads per currency_id: , `LiquidityPool`, `LiquidityIncentiveRate`
		/// -------------------
		/// Base Weight: 79.58 * N µs
		/// # </weight>
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			let mut consumed_weight = 0;
			let mut add_weight = |reads, writes, weight| {
				consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
				consumed_weight += weight;
			};

			if !T::EmergencyShutdown::is_shutdown() {
				add_weight(4, 3, 0);
				let mut accumulated_interest: Balance = Zero::zero();

				// accumulate interest
				for currency_id in T::EnabledCurrencyIds::get() {
					let interest_to_issue = Self::accumulate_interest(currency_id);
					accumulated_interest = accumulated_interest.saturating_add(interest_to_issue);
					add_weight(2, 0, 80_000_000);
				}

				// issue aUSD as interest, ignore result
				let _ = T::CDPTreasury::issue_debit(&Self::account_id(), accumulated_interest, false);
			}

			consumed_weight
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	fn calculate_swap_target_amount(
		supply_pool: Balance,
		target_pool: Balance,
		supply_amount: Balance,
		fee_rate: Rate,
	) -> Balance {
		if supply_amount.is_zero() {
			Zero::zero()
		} else {
			// new_target_pool = supply_pool * target_pool / (supply_amount + supply_pool)
			let new_target_pool = supply_pool
				.checked_add(supply_amount)
				.and_then(|n| Ratio::checked_from_rational(supply_pool, n))
				.and_then(|n| n.checked_mul_int(target_pool))
				.unwrap_or_default();

			if new_target_pool.is_zero() {
				Zero::zero()
			} else {
				// target_amount = (target_pool - new_target_pool) * (1 - fee_rate)
				target_pool
					.checked_sub(new_target_pool)
					.and_then(|n| Rate::one().saturating_sub(fee_rate).checked_mul_int(n))
					.unwrap_or_default()
			}
		}
	}

	/// Calculate how much supply token needed for swap specific target amount.
	fn calculate_swap_supply_amount(
		supply_pool: Balance,
		target_pool: Balance,
		target_amount: Balance,
		fee_rate: Rate,
	) -> Balance {
		if target_amount.is_zero() {
			Zero::zero()
		} else {
			// new_target_pool = target_pool - target_amount / (1 - fee_rate)
			let new_target_pool = Rate::one()
				.saturating_sub(fee_rate)
				.reciprocal()
				.and_then(|n| n.checked_add(&Ratio::from_inner(1))) // add 1 to result in order to correct the possible losses caused by remainder discarding in internal
				// division calculation
				.and_then(|n| n.checked_mul_int(target_amount))
				// add 1 to result in order to correct the possible losses caused by remainder discarding in internal
				// division calculation
				.and_then(|n| n.checked_add(Balance::one()))
				.and_then(|n| target_pool.checked_sub(n))
				.unwrap_or_default();

			if new_target_pool.is_zero() {
				Zero::zero()
			} else {
				// supply_amount = target_pool * supply_pool / new_target_pool - supply_pool
				Ratio::checked_from_rational(target_pool, new_target_pool)
					.and_then(|n| n.checked_add(&Ratio::from_inner(1))) // add 1 to result in order to correct the possible losses caused by remainder discarding in
					// internal division calculation
					.and_then(|n| n.checked_mul_int(supply_pool))
					.and_then(|n| n.checked_add(Balance::one())) // add 1 to result in order to correct the possible losses caused by remainder discarding in
					// internal division calculation
					.and_then(|n| n.checked_sub(supply_pool))
					.unwrap_or_default()
			}
		}
	}

	// use other currency to swap base currency
	fn swap_other_to_base(
		who: &T::AccountId,
		other_currency_id: CurrencyId,
		other_currency_amount: Balance,
		acceptable_base_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		// calculate the base currency amount can get
		let base_currency_id = T::GetBaseCurrencyId::get();
		let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(other_currency_id);
		let base_currency_amount = Self::calculate_swap_target_amount(
			other_currency_pool,
			base_currency_pool,
			other_currency_amount,
			T::GetExchangeFee::get(),
		);

		// ensure the amount can get is not 0 and >= minium acceptable
		ensure!(
			!base_currency_amount.is_zero() && base_currency_amount >= acceptable_base_currency_amount,
			Error::<T>::UnacceptablePrice,
		);

		// transfer token between account and dex and update liquidity pool
		T::Currency::transfer(other_currency_id, who, &Self::account_id(), other_currency_amount)?;
		T::Currency::transfer(base_currency_id, &Self::account_id(), who, base_currency_amount)?;

		LiquidityPool::mutate(other_currency_id, |(other, base)| {
			*other = other.saturating_add(other_currency_amount);
			*base = base.saturating_sub(base_currency_amount);
		});

		Ok(base_currency_amount)
	}

	// use base currency to swap other currency
	fn swap_base_to_other(
		who: &T::AccountId,
		other_currency_id: CurrencyId,
		base_currency_amount: Balance,
		acceptable_other_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(other_currency_id);
		let other_currency_amount = Self::calculate_swap_target_amount(
			base_currency_pool,
			other_currency_pool,
			base_currency_amount,
			T::GetExchangeFee::get(),
		);
		ensure!(
			!other_currency_amount.is_zero() && other_currency_amount >= acceptable_other_currency_amount,
			Error::<T>::UnacceptablePrice,
		);

		T::Currency::transfer(base_currency_id, who, &Self::account_id(), base_currency_amount)?;
		T::Currency::transfer(other_currency_id, &Self::account_id(), who, other_currency_amount)?;
		LiquidityPool::mutate(other_currency_id, |(other, base)| {
			*other = other.saturating_sub(other_currency_amount);
			*base = base.saturating_add(base_currency_amount);
		});

		Ok(other_currency_amount)
	}

	// use other currency to swap another other currency
	fn swap_other_to_other(
		who: &T::AccountId,
		supply_other_currency_id: CurrencyId,
		supply_other_currency_amount: Balance,
		target_other_currency_id: CurrencyId,
		acceptable_target_other_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		let fee_rate = T::GetExchangeFee::get();
		let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_other_currency_id);
		let intermediate_base_currency_amount = Self::calculate_swap_target_amount(
			supply_other_currency_pool,
			supply_base_currency_pool,
			supply_other_currency_amount,
			fee_rate,
		);
		let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_other_currency_id);
		let target_other_currency_amount = Self::calculate_swap_target_amount(
			target_base_currency_pool,
			target_other_currency_pool,
			intermediate_base_currency_amount,
			fee_rate,
		);
		ensure!(
			!target_other_currency_amount.is_zero()
				&& target_other_currency_amount >= acceptable_target_other_currency_amount,
			Error::<T>::UnacceptablePrice,
		);

		T::Currency::transfer(
			supply_other_currency_id,
			who,
			&Self::account_id(),
			supply_other_currency_amount,
		)?;
		T::Currency::transfer(
			target_other_currency_id,
			&Self::account_id(),
			who,
			target_other_currency_amount,
		)?;

		LiquidityPool::mutate(supply_other_currency_id, |(other, base)| {
			*other = other.saturating_add(supply_other_currency_amount);
			*base = base.saturating_sub(intermediate_base_currency_amount);
		});
		LiquidityPool::mutate(target_other_currency_id, |(other, base)| {
			*other = other.saturating_sub(target_other_currency_amount);
			*base = base.saturating_add(intermediate_base_currency_amount);
		});

		Ok(target_other_currency_amount)
	}

	fn do_exchange(
		who: &T::AccountId,
		supply_currency_id: CurrencyId,
		supply_amount: Balance,
		target_currency_id: CurrencyId,
		acceptable_target_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let allowed_currency_ids = T::EnabledCurrencyIds::get();

		let target_turnover =
			if target_currency_id == base_currency_id && allowed_currency_ids.contains(&supply_currency_id) {
				Self::swap_other_to_base(who, supply_currency_id, supply_amount, acceptable_target_amount)
			} else if supply_currency_id == base_currency_id && allowed_currency_ids.contains(&target_currency_id) {
				Self::swap_base_to_other(who, target_currency_id, supply_amount, acceptable_target_amount)
			} else if supply_currency_id != target_currency_id
				&& allowed_currency_ids.contains(&supply_currency_id)
				&& allowed_currency_ids.contains(&target_currency_id)
			{
				Self::swap_other_to_other(
					who,
					supply_currency_id,
					supply_amount,
					target_currency_id,
					acceptable_target_amount,
				)
			} else {
				Err(Error::<T>::CurrencyIdNotAllowed.into())
			}?;

		Self::deposit_event(RawEvent::Swap(
			who.clone(),
			supply_currency_id,
			supply_amount,
			target_currency_id,
			target_turnover,
		));

		Ok(target_turnover)
	}

	// get the minimum amount of supply currency needed for the target currency
	// amount return 0 means cannot exchange
	pub fn get_supply_amount_needed(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let fee_rate = T::GetExchangeFee::get();
		if supply_currency_id == target_currency_id {
			Zero::zero()
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_supply_amount(
				other_currency_pool,
				base_currency_pool,
				target_currency_amount,
				fee_rate,
			)
		} else if supply_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_supply_amount(
				base_currency_pool,
				other_currency_pool,
				target_currency_amount,
				fee_rate,
			)
		} else {
			let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);
			let intermediate_base_currency_amount = Self::calculate_swap_supply_amount(
				target_base_currency_pool,
				target_other_currency_pool,
				target_currency_amount,
				fee_rate,
			);
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_supply_amount(
				supply_other_currency_pool,
				supply_base_currency_pool,
				intermediate_base_currency_amount,
				fee_rate,
			)
		}
	}

	// get the maximum amount of target currency you can get for the supply currency
	// amount return 0 means cannot exchange
	pub fn get_target_amount_available(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Balance {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let fee_rate = T::GetExchangeFee::get();
		if supply_currency_id == target_currency_id {
			Zero::zero()
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_target_amount(
				other_currency_pool,
				base_currency_pool,
				supply_currency_amount,
				fee_rate,
			)
		} else if supply_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_target_amount(
				base_currency_pool,
				other_currency_pool,
				supply_currency_amount,
				fee_rate,
			)
		} else {
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			let intermediate_base_currency_amount = Self::calculate_swap_target_amount(
				supply_other_currency_pool,
				supply_base_currency_pool,
				supply_currency_amount,
				fee_rate,
			);
			let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_target_amount(
				target_base_currency_pool,
				target_other_currency_pool,
				intermediate_base_currency_amount,
				fee_rate,
			)
		}
	}

	fn deposit_calculate_interest(currency_id: CurrencyId, who: &T::AccountId, share_amount: T::Share) {
		let total_shares = Self::total_shares(currency_id);
		let (total_interest, _) = Self::total_interest(currency_id);
		if total_shares.is_zero() || total_interest.is_zero() {
			return;
		}

		let proportion = Ratio::checked_from_rational(share_amount, total_shares).unwrap_or_default();
		let interest_to_expand = proportion.saturating_mul_int(total_interest);
		<WithdrawnInterest<T>>::mutate(currency_id, who, |val| {
			*val = val.saturating_add(interest_to_expand);
		});
		TotalInterest::mutate(currency_id, |(total_interest, total_withdrawn)| {
			*total_interest = total_interest.saturating_add(interest_to_expand);
			*total_withdrawn = total_withdrawn.saturating_add(interest_to_expand);
		});
	}

	fn withdraw_calculate_interest(
		currency_id: CurrencyId,
		who: &T::AccountId,
		share_amount: T::Share,
	) -> DispatchResult {
		// claim interest first
		Self::claim_interest(currency_id, who)?;

		let proportion =
			Ratio::checked_from_rational(share_amount, Self::total_shares(currency_id)).unwrap_or_default();
		let withdrawn_interest_to_remove = Ratio::checked_from_rational(share_amount, Self::shares(currency_id, who))
			.unwrap_or_default()
			.saturating_mul_int(Self::withdrawn_interest(currency_id, who));

		<WithdrawnInterest<T>>::mutate(currency_id, who, |val| {
			*val = val.saturating_sub(withdrawn_interest_to_remove);
		});
		TotalInterest::mutate(currency_id, |(total_interest, total_withdrawn)| {
			*total_interest = total_interest.saturating_sub(proportion.saturating_mul_int(*total_interest));
			*total_withdrawn = total_withdrawn.saturating_sub(withdrawn_interest_to_remove);
		});

		Ok(())
	}

	fn claim_interest(currency_id: CurrencyId, who: &T::AccountId) -> DispatchResult {
		let proportion = Ratio::checked_from_rational(Self::shares(currency_id, who), Self::total_shares(currency_id))
			.unwrap_or_default();
		let interest_to_withdraw = proportion
			.saturating_mul_int(Self::total_interest(currency_id).0)
			.saturating_sub(Self::withdrawn_interest(currency_id, who));

		if !interest_to_withdraw.is_zero() {
			// withdraw interest to share holder
			T::Currency::transfer(
				T::GetBaseCurrencyId::get(),
				&Self::account_id(),
				&who,
				interest_to_withdraw,
			)?;
			<WithdrawnInterest<T>>::mutate(currency_id, who, |val| {
				*val = val.saturating_add(interest_to_withdraw);
			});
			TotalInterest::mutate(currency_id, |(_, total_withdrawn)| {
				*total_withdrawn = total_withdrawn.saturating_add(interest_to_withdraw);
			});

			Self::deposit_event(RawEvent::IncentiveInterestClaimed(
				who.clone(),
				currency_id,
				interest_to_withdraw,
			));
		}

		Ok(())
	}

	fn accumulate_interest(currency_id: CurrencyId) -> Balance {
		let (_, base_currency_pool) = Self::liquidity_pool(currency_id);
		let interest_to_increase = Self::liquidity_incentive_rate(currency_id).saturating_mul_int(base_currency_pool);

		if !interest_to_increase.is_zero() {
			TotalInterest::mutate(currency_id, |(total_interest, _)| {
				*total_interest = total_interest.saturating_add(interest_to_increase);
			});
		}

		interest_to_increase
	}
}

impl<T: Trait> DEXManager<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn get_target_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Balance {
		Self::get_target_amount_available(supply_currency_id, target_currency_id, supply_currency_amount)
	}

	fn get_supply_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance {
		Self::get_supply_amount_needed(supply_currency_id, target_currency_id, target_currency_amount)
	}

	fn exchange_currency(
		who: T::AccountId,
		supply_currency_id: CurrencyId,
		supply_amount: Balance,
		target_currency_id: CurrencyId,
		acceptable_target_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Self::do_exchange(
			&who,
			supply_currency_id,
			supply_amount,
			target_currency_id,
			acceptable_target_amount,
		)
	}

	// do not consider the fee rate
	fn get_exchange_slippage(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_amount: Balance,
	) -> Option<Ratio> {
		let base_currency_id = T::GetBaseCurrencyId::get();

		if supply_currency_id == target_currency_id {
			None
		} else if supply_currency_id == base_currency_id {
			let (_, base_currency_pool) = Self::liquidity_pool(target_currency_id);

			// supply_amount / (supply_amount + base_currency_pool)
			supply_amount
				.checked_add(base_currency_pool)
				.and_then(|n| Ratio::checked_from_rational(supply_amount, n))
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, _) = Self::liquidity_pool(supply_currency_id);

			// supply_amount / (supply_amount + other_currency_pool)
			supply_amount
				.checked_add(other_currency_pool)
				.and_then(|n| Ratio::checked_from_rational(supply_amount, n))
		} else {
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			let (_, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);

			// first slippage in swap supply other currency to base currency:
			// first_slippage = supply_amount / (supply_amount + supply_other_currency_pool)
			let supply_to_base_slippage = supply_amount
				.checked_add(supply_other_currency_pool)
				.and_then(|n| Ratio::checked_from_rational(supply_amount, n))?;

			// second slippage in swap base currency to target other currency:
			// base_amount = first_slippage * supply_base_currency_pool
			// second_slippage = base_amount / (base_amount + target_base_currency_pool)
			let base_amount = supply_to_base_slippage.saturating_mul_int(supply_base_currency_pool);
			let base_to_target_slippage = base_amount
				.checked_add(target_base_currency_pool)
				.and_then(|n| Ratio::checked_from_rational(base_amount, n))?;

			// final_slippage = first_slippage + (1 - first_slippage) * second_slippage
			Ratio::one()
				.checked_sub(&supply_to_base_slippage)
				.and_then(|n| n.checked_mul(&base_to_target_slippage))
				.and_then(|n| n.checked_add(&supply_to_base_slippage))
		}
	}
}
