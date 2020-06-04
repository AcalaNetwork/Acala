//! # DEX Module
//!
//! ## Overview
//!
//! Built-in decentralized exchange modules in Acala network, the core currency type of trading pairs is stable coin (aUSD),
//! the trading mechanism refers to the design of Uniswap. In addition to being used for trading, DEX also participates
//! in CDP liquidation, which is faster than liquidation by auction when the liquidity is sufficient. And providing market
//! making liquidity for DEX will also receive stable coin as additional reward for its participation in the CDP liquidation.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::{EnsureOrigin, Get},
	weights::Weight,
	Parameter,
};
use frame_system::{self as system, ensure_root, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::fixed_u128::{FixedPointOperand, FixedUnsignedNumber};
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32Bit, CheckedAdd, CheckedDiv, CheckedSub, MaybeSerializeDeserialize, Member, One,
		Saturating, UniqueSaturatedInto, Zero,
	},
	DispatchError, DispatchResult, ModuleId,
};
use sp_std::prelude::Vec;
use support::{CDPTreasury, DEXManager, OnEmergencyShutdown, Price, Rate, Ratio};

mod benchmarking;
mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/dexm");

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Associate type for measuring liquidity contribution of specific trading pairs
	type Share: Parameter + Member + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize + FixedPointOperand;

	/// The origin which may update parameters of dex. Root can always do this.
	type UpdateOrigin: EnsureOrigin<Self::Origin>;

	/// Currency for transfer currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// CDP treasury for depositing additional liquidity reward to DEX
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Allowed trading currency type list, each currency type forms a trading pair with the base currency
	type EnabledCurrencyIds: Get<Vec<CurrencyId>>;

	/// The base currency as the core currency in all trading pairs
	type GetBaseCurrencyId: Get<CurrencyId>;

	/// Trading fee rate
	type GetExchangeFee: Get<Rate>;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Share,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Add liquidity success (who, currency_type, added_currency_amount, added_base_currency_amount, increment_share_amount)
		AddLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		/// Withdraw liquidity from the trading pool success (who, currency_type, withdrawn_currency_amount, withdrawn_base_currency_amount, burned_share_amount)
		WithdrawLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		/// Use supply currency to swap target currency (trader, supply_currency_type, supply_currency_amount, target_currency_type, target_currency_amount)
		Swap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for dex module.
	pub enum Error for Module<T: Trait> {
		/// Not the tradable currency type
		CurrencyIdNotAllowed,
		/// Currency amount is not enough
		AmountNotEnough,
		/// Share amount is not enough
		ShareNotEnough,
		/// Currency amount is invalid
		InvalidAmount,
		/// Can not trading with self currency type
		CanNotSwapItself,
		/// The actual transaction price will be lower than the acceptable price
		InacceptablePrice,
		/// The increament of liquidity is invalid
		InvalidLiquidityIncrement,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		/// Liquidity pool, which is the trading pair for specific currency type to base currency type.
		/// CurrencyType -> (CurrencyAmount, BaseCurrencyAmount)
		LiquidityPool get(fn liquidity_pool): map hasher(twox_64_concat) CurrencyId => (Balance, Balance);

		/// Total shares amount of liquidity pool specificed by currency type
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

		/// System shutdown flag
		IsShutdown get(fn is_shutdown): bool;
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

		/// Update liquidity incentive rate of specific liquidity pool
		///
		/// The dispatch origin of this call must be `UpdateOrigin` or _Root_.
		///
		/// - `currency_id`: currency type to determine the type of liquidity pool.
		/// - `liquidity_incentive_rate`: liquidity incentive rate.
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads:
		/// - Db writes: LiquidityIncentiveRate
		/// -------------------
		/// Base Weight: 3.591 µs
		/// # </weight>
		#[weight = 4_000_000 + T::DbWeight::get().reads_writes(0, 1)]
		pub fn set_liquidity_incentive_rate(
			origin,
			currency_id: CurrencyId,
			liquidity_incentive_rate: Rate,
		) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;

			LiquidityIncentiveRate::insert(currency_id, liquidity_incentive_rate);
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
		/// - Db reads: `WithdrawnInterest`, `TotalWithdrawnInterest`, 2 items of orml_currencies
		/// - Db writes: `WithdrawnInterest`, `TotalWithdrawnInterest`, 2 items of orml_currencies
		/// -------------------
		/// Base Weight: 38.4 µs
		/// # </weight>
		#[weight = 39_000_000 + T::DbWeight::get().reads_writes(4, 4)]
		pub fn withdraw_incentive_interest(origin, currency_id: CurrencyId) {
			let who = ensure_signed(origin)?;
			Self::claim_interest(currency_id, &who)?;
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
		///		- swap other to base: 1 * `LiquidityPool`, 4 items of orml_currencies
		///		- swap base to other: 1 * `LiquidityPool`, 4 items of orml_currencies
		///		- swap other to other: 2 * `LiquidityPool`, 4 items of orml_currencies
		/// - Db writes:
		///		- swap other to base: 1 * `LiquidityPool`, 4 items of orml_currencies
		///		- swap base to other: 1 * `LiquidityPool`, 4 items of orml_currencies
		///		- swap other to other: 2 * `LiquidityPool`, 4 items of orml_currencies
		/// -------------------
		/// Base Weight:
		///		- swap base to other: 47.81 µs
		///		- swap other to base: 42.57 µs
		///		- swap other to other: 54.77 µs
		/// # </weight>
		#[weight = 55_000_000 + T::DbWeight::get().reads_writes(6, 6)]
		pub fn swap_currency(
			origin,
			supply_currency_id: CurrencyId,
			#[compact] supply_amount: Balance,
			target_currency_id: CurrencyId,
			#[compact] acceptable_target_amount: Balance,
		) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();
			ensure!(
				supply_currency_id != target_currency_id,
				Error::<T>::CanNotSwapItself,
			);

			if target_currency_id == base_currency_id {
				Self::swap_other_to_base(who, supply_currency_id, supply_amount, acceptable_target_amount)?;
			} else if supply_currency_id == base_currency_id {
				Self::swap_base_to_other(who, target_currency_id, supply_amount, acceptable_target_amount)?;
			} else {
				Self::swap_other_to_other(who, supply_currency_id, supply_amount, target_currency_id, acceptable_target_amount)?;
			}
		}

		/// Injecting liquidity to specific liquidity pool in the form of depositing currencies in trading pairs
		/// into liquidity pool, and issue shares in proportion to the caller. Shares are temporarily not
		/// allowed to transfer and trade, it represents the proportion of assets in liquidity pool.
		///
		/// - `other_currency_id`: currency type to determine the type of liquidity pool.
		/// - `max_other_currency_amount`: maximum currency amount allowed to inject to liquidity pool.
		/// - `max_base_currency_amount`: maximum base currency(stable coin) amount allowed to inject to liquidity pool.
		///
		/// # <weight>
		/// - Preconditions:
		/// 	- T::Currency is orml_currencies
		/// - Complexity: `O(1)`
		/// - Db reads:
		///		- best case: `TotalShares`, `LiquidityPool`, `Shares`, 4 items of orml_currencies
		///		- worst case: `TotalShares`, `LiquidityPool`, `Shares`, `WithdrawnInterest`, `TotalInterest`, 4 items of orml_currencies
		/// - Db writes:
		///		- best case: `TotalShares`, `LiquidityPool`, `Shares`, 4 items of orml_currencies
		///		- worst case: `TotalShares`, `LiquidityPool`, `Shares`, `WithdrawnInterest`, `TotalInterest`, 4 items of orml_currencies
		/// -------------------
		/// Base Weight:
		///		- best case: 49.04 µs
		///		- worst case: 57.72 µs
		/// # </weight>
		#[weight = 58_000_000 + T::DbWeight::get().reads_writes(8, 9)]
		pub fn add_liquidity(
			origin,
			other_currency_id: CurrencyId,
			#[compact] max_other_currency_amount: Balance,
			#[compact] max_base_currency_amount: Balance,
		) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();
			ensure!(
				T::EnabledCurrencyIds::get().contains(&other_currency_id),
				Error::<T>::CurrencyIdNotAllowed,
			);
			ensure!(
				!max_other_currency_amount.is_zero() && !max_base_currency_amount.is_zero(),
				Error::<T>::InvalidAmount,
			);

			let total_shares = Self::total_shares(other_currency_id);
			let (other_currency_increment, base_currency_increment, share_increment): (Balance, Balance, T::Share) =
			if total_shares.is_zero() {
				// initialize this liquidity pool, the initial share is equal to the max value between base currency amount and other currency amount
				let initial_share: u128 = sp_std::cmp::max(max_other_currency_amount, max_base_currency_amount).unique_saturated_into();
				let initial_share: T::Share = initial_share.unique_saturated_into();

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
			ensure!(
				T::Currency::ensure_can_withdraw(base_currency_id, &who, base_currency_increment).is_ok()
				&&
				T::Currency::ensure_can_withdraw(other_currency_id, &who, other_currency_increment).is_ok(),
				Error::<T>::AmountNotEnough,
			);
			T::Currency::transfer(other_currency_id, &who, &Self::account_id(), other_currency_increment)
			.expect("never failed because after checks");
			T::Currency::transfer(base_currency_id, &who, &Self::account_id(), base_currency_increment)
			.expect("never failed because after checks");

			Self::deposit_calculate_interest(other_currency_id, &who, share_increment);
			<TotalShares<T>>::mutate(other_currency_id, |share| *share = share.saturating_add(share_increment));
			<Shares<T>>::mutate(other_currency_id, &who, |share| *share = share.saturating_add(share_increment));
			LiquidityPool::mutate(other_currency_id, |pool| {
				*pool = (pool.0.saturating_add(other_currency_increment), pool.1.saturating_add(base_currency_increment));
			});
			Self::deposit_event(RawEvent::AddLiquidity(
				who,
				other_currency_id,
				other_currency_increment,
				base_currency_increment,
				share_increment,
			));
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
		/// - Db reads: `Shares`, `LiquidityPool`, `TotalShares`, `WithdrawnInterest`, `TotalInterest`, 4 items of orml_currencies
		/// - Db writes: `Shares`, `LiquidityPool`, `TotalShares`, `WithdrawnInterest`, `TotalInterest`, 4 items of orml_currencies
		/// -------------------
		/// Base Weight:
		///		- best case: 66.59 µs
		///		- worst case: 71.18 µs
		/// # </weight>
		#[weight = 72_000_000 + T::DbWeight::get().reads_writes(9, 9)]
		pub fn withdraw_liquidity(origin, currency_id: CurrencyId, #[compact] share_amount: T::Share) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();
			ensure!(
				T::EnabledCurrencyIds::get().contains(&currency_id),
				Error::<T>::CurrencyIdNotAllowed,
			);
			ensure!(
				Self::shares(currency_id, &who) >= share_amount && !share_amount.is_zero(),
				Error::<T>::ShareNotEnough,
			);

			let (other_currency_pool, base_currency_pool): (Balance, Balance) = Self::liquidity_pool(currency_id);
			let proportion = Ratio::checked_from_rational(share_amount, Self::total_shares(currency_id)).unwrap_or_default();
			let withdraw_other_currency_amount = proportion.saturating_mul_int(other_currency_pool);
			let withdraw_base_currency_amount = proportion.saturating_mul_int(base_currency_pool);
			if !withdraw_other_currency_amount.is_zero() {
				T::Currency::transfer(currency_id, &Self::account_id(), &who, withdraw_other_currency_amount)
				.expect("never failed because after checks");
			}
			if !withdraw_base_currency_amount.is_zero() {
				T::Currency::transfer(base_currency_id, &Self::account_id(), &who, withdraw_base_currency_amount)
				.expect("never failed because after checks");
			}

			Self::withdraw_calculate_interest(currency_id, &who, share_amount)?;
			<TotalShares<T>>::mutate(currency_id, |share| *share = share.saturating_sub(share_amount));
			<Shares<T>>::mutate(currency_id, &who, |share| *share = share.saturating_sub(share_amount));
			LiquidityPool::mutate(currency_id, |pool| {
				*pool = (pool.0.saturating_sub(withdraw_other_currency_amount), pool.1.saturating_sub(withdraw_base_currency_amount));
			});

			Self::deposit_event(RawEvent::WithdrawLiquidity(
				who,
				currency_id,
				withdraw_other_currency_amount,
				withdraw_base_currency_amount,
				share_amount,
			));
		}

		/// Accumalte liquidity incentive interest to respective reward pool when block end
		///
		/// # <weight>
		/// - Complexity: `O(N)` where `N` is the number of currency_ids
		/// - Db reads: `IsShutdown`, `TotalInterest`, 2 items in cdp_treasury
		///	- Db writes: `TotalInterest`, 2 items in cdp_treasury
		/// - Db reads per currency_id: , `LiquidityPool`, `LiquidityIncentiveRate`
		/// -------------------
		/// Base Weight: 35.45 * N µs
		/// # </weight>
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			let mut consumed_weight = 0;
			let mut add_weight = |reads, writes, weight| {
				consumed_weight += T::DbWeight::get().reads_writes(reads, writes);
				consumed_weight += weight;
			};

			if !Self::is_shutdown() {
				add_weight(4, 3, 0);
				for currency_id in T::EnabledCurrencyIds::get() {
					Self::accumulate_interest(currency_id);
					add_weight(2, 0, 36_000_000);
				}
			}
			consumed_weight
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	pub fn calculate_swap_target_amount(supply_pool: Balance, target_pool: Balance, supply_amount: Balance) -> Balance {
		// new_target_pool = supply_pool * target_pool / (supply_amount + supply_pool)
		let new_target_pool = supply_pool
			.checked_add(supply_amount)
			.and_then(|n| Ratio::checked_from_rational(supply_pool, n))
			.map(|n| n.saturating_mul_int(target_pool))
			.unwrap_or_default();

		// new_target_pool should be more then 0
		if !new_target_pool.is_zero() {
			// actual can get = (target_pool - new_target_pool) * (1 - GetExchangeFee)
			target_pool
				.checked_sub(new_target_pool)
				.and_then(|n| n.checked_sub(T::GetExchangeFee::get().saturating_mul_int(n)))
				.unwrap_or_default()
		} else {
			Zero::zero()
		}
	}

	pub fn calculate_swap_supply_amount(supply_pool: Balance, target_pool: Balance, target_amount: Balance) -> Balance {
		// formular:
		// new_target_pool = target_pool - target_amount / (1 - GetExchangeFee)
		// supply_amount = target_pool * supply_pool / new_target_pool - supply_pool

		// TODO : determine if there is a remainder before adding 1 in multiple calculation of FixedU128
		// that needs FixedU128 supporting new mul function
		if target_amount.is_zero() {
			Zero::zero()
		} else {
			Rate::from_natural(1)
				.checked_sub(&T::GetExchangeFee::get())
				.and_then(|n| Ratio::from_natural(1).checked_div(&n))
				.and_then(|n| Ratio::from_inner(1).checked_add(&n)) // add Ratio::from_inner(1) to correct the possible losses caused by discarding the remainder in inner division
				.and_then(|n| n.checked_mul_int(target_amount))
				.and_then(|n| n.checked_add(One::one())) // add 1 to correct the possible losses caused by discarding the remainder in division
				.and_then(|n| target_pool.checked_sub(n))
				.and_then(|n| Ratio::checked_from_rational(supply_pool, n))
				.and_then(|n| Ratio::from_inner(1).checked_add(&n)) // add Ratio::from_inner(1) to correct the possible losses caused by discarding the remainder in inner division
				.and_then(|n| n.checked_mul_int(target_pool))
				.and_then(|n| n.checked_add(One::one())) // add 1 to correct the possible losses caused by discarding the remainder in division
				.and_then(|n| n.checked_sub(supply_pool))
				.unwrap_or_default()
		}
	}

	// use other currency to swap base currency
	pub fn swap_other_to_base(
		who: T::AccountId,
		other_currency_id: CurrencyId,
		other_currency_amount: Balance,
		acceptable_base_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		// 1. ensure supply amount must > 0 and account has sufficient balance
		ensure!(
			!other_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(other_currency_id, &who, other_currency_amount).is_ok(),
			Error::<T>::AmountNotEnough,
		);

		// 2. calculate the base currency amount can get
		let base_currency_id = T::GetBaseCurrencyId::get();
		let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(other_currency_id);
		let base_currency_amount =
			Self::calculate_swap_target_amount(other_currency_pool, base_currency_pool, other_currency_amount);

		// 3. ensure the amount can get is not 0 and >= minium acceptable
		ensure!(
			!base_currency_amount.is_zero() && base_currency_amount >= acceptable_base_currency_amount,
			Error::<T>::InacceptablePrice,
		);

		// 4. transfer token between account and dex and update liquidity pool
		T::Currency::transfer(other_currency_id, &who, &Self::account_id(), other_currency_amount)
			.expect("never failed because after checks");
		T::Currency::transfer(base_currency_id, &Self::account_id(), &who, base_currency_amount)
			.expect("never failed because after checks");
		LiquidityPool::mutate(other_currency_id, |pool| {
			*pool = (
				pool.0.saturating_add(other_currency_amount),
				pool.1.saturating_sub(base_currency_amount),
			);
		});

		Self::deposit_event(RawEvent::Swap(
			who,
			other_currency_id,
			other_currency_amount,
			base_currency_id,
			base_currency_amount,
		));
		Ok(base_currency_amount)
	}

	// use base currency to swap other currency
	pub fn swap_base_to_other(
		who: T::AccountId,
		other_currency_id: CurrencyId,
		base_currency_amount: Balance,
		acceptable_other_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		ensure!(
			!base_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(base_currency_id, &who, base_currency_amount).is_ok(),
			Error::<T>::AmountNotEnough,
		);

		let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(other_currency_id);
		let other_currency_amount =
			Self::calculate_swap_target_amount(base_currency_pool, other_currency_pool, base_currency_amount);
		ensure!(
			!other_currency_amount.is_zero() && other_currency_amount >= acceptable_other_currency_amount,
			Error::<T>::InacceptablePrice,
		);

		T::Currency::transfer(base_currency_id, &who, &Self::account_id(), base_currency_amount)
			.expect("never failed because after checks");
		T::Currency::transfer(other_currency_id, &Self::account_id(), &who, other_currency_amount)
			.expect("never failed because after checks");
		LiquidityPool::mutate(other_currency_id, |pool| {
			*pool = (
				pool.0.saturating_sub(other_currency_amount),
				pool.1.saturating_add(base_currency_amount),
			);
		});

		Self::deposit_event(RawEvent::Swap(
			who,
			base_currency_id,
			base_currency_amount,
			other_currency_id,
			other_currency_amount,
		));
		Ok(other_currency_amount)
	}

	// use other currency to swap another other currency
	pub fn swap_other_to_other(
		who: T::AccountId,
		supply_other_currency_id: CurrencyId,
		supply_other_currency_amount: Balance,
		target_other_currency_id: CurrencyId,
		acceptable_target_other_currency_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		ensure!(
			!supply_other_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(supply_other_currency_id, &who, supply_other_currency_amount)
					.is_ok(),
			Error::<T>::AmountNotEnough,
		);

		let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_other_currency_id);
		let intermediate_base_currency_amount = Self::calculate_swap_target_amount(
			supply_other_currency_pool,
			supply_base_currency_pool,
			supply_other_currency_amount,
		);
		let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_other_currency_id);
		let target_other_currency_amount = Self::calculate_swap_target_amount(
			target_base_currency_pool,
			target_other_currency_pool,
			intermediate_base_currency_amount,
		);
		ensure!(
			!target_other_currency_amount.is_zero()
				&& target_other_currency_amount >= acceptable_target_other_currency_amount,
			Error::<T>::InacceptablePrice,
		);

		T::Currency::transfer(
			supply_other_currency_id,
			&who,
			&Self::account_id(),
			supply_other_currency_amount,
		)
		.expect("never failed because after checks");
		T::Currency::transfer(
			target_other_currency_id,
			&Self::account_id(),
			&who,
			target_other_currency_amount,
		)
		.expect("never failed because after checks");
		LiquidityPool::mutate(supply_other_currency_id, |pool| {
			*pool = (
				pool.0.saturating_add(supply_other_currency_amount),
				pool.1.saturating_sub(intermediate_base_currency_amount),
			);
		});
		LiquidityPool::mutate(target_other_currency_id, |pool| {
			*pool = (
				pool.0.saturating_sub(target_other_currency_amount),
				pool.1.saturating_add(intermediate_base_currency_amount),
			);
		});

		Self::deposit_event(RawEvent::Swap(
			who,
			supply_other_currency_id,
			supply_other_currency_amount,
			target_other_currency_id,
			target_other_currency_amount,
		));
		Ok(target_other_currency_amount)
	}

	// get the minimum amount of supply currency needed for the target currency amount
	// return 0 means cannot exchange
	pub fn get_supply_amount_needed(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Balance {
		let base_currency_id = T::GetBaseCurrencyId::get();
		if supply_currency_id == target_currency_id {
			Zero::zero()
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_supply_amount(other_currency_pool, base_currency_pool, target_currency_amount)
		} else if supply_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_supply_amount(base_currency_pool, other_currency_pool, target_currency_amount)
		} else {
			let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);
			let intermediate_base_currency_amount = Self::calculate_swap_supply_amount(
				target_base_currency_pool,
				target_other_currency_pool,
				target_currency_amount,
			);
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_supply_amount(
				supply_other_currency_pool,
				supply_base_currency_pool,
				intermediate_base_currency_amount,
			)
		}
	}

	// get the maximum amount of target currency you can get for the supply currency amount
	// return 0 means cannot exchange
	pub fn get_target_amount_available(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Balance {
		let base_currency_id = T::GetBaseCurrencyId::get();
		if supply_currency_id == target_currency_id {
			Zero::zero()
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			Self::calculate_swap_target_amount(other_currency_pool, base_currency_pool, supply_currency_amount)
		} else if supply_currency_id == base_currency_id {
			let (other_currency_pool, base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_target_amount(base_currency_pool, other_currency_pool, supply_currency_amount)
		} else {
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			let intermediate_base_currency_amount = Self::calculate_swap_target_amount(
				supply_other_currency_pool,
				supply_base_currency_pool,
				supply_currency_amount,
			);
			let (target_other_currency_pool, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);
			Self::calculate_swap_target_amount(
				target_base_currency_pool,
				target_other_currency_pool,
				intermediate_base_currency_amount,
			)
		}
	}

	pub fn deposit_calculate_interest(currency_id: CurrencyId, who: &T::AccountId, share_amount: T::Share) {
		let total_shares = Self::total_shares(currency_id);
		if total_shares.is_zero() {
			return;
		}
		let proportion = Ratio::checked_from_rational(share_amount, total_shares).unwrap_or_default();
		let (total_interest, _) = Self::total_interest(currency_id);
		if total_interest.is_zero() {
			return;
		}
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
		}

		Ok(())
	}

	fn accumulate_interest(currency_id: CurrencyId) {
		let (_, base_currency_pool) = Self::liquidity_pool(currency_id);
		let interest_to_increase = Self::liquidity_incentive_rate(currency_id).saturating_mul_int(base_currency_pool);

		if !interest_to_increase.is_zero() {
			// issue aUSD as interest
			if T::CDPTreasury::deposit_unbacked_debit_to(&Self::account_id(), interest_to_increase).is_ok() {
				TotalInterest::mutate(currency_id, |(total_interest, _)| {
					*total_interest = total_interest.saturating_add(interest_to_increase);
				});
			}
		}
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
		let base_currency_id = T::GetBaseCurrencyId::get();
		ensure!(target_currency_id != supply_currency_id, Error::<T>::CanNotSwapItself);
		if target_currency_id == base_currency_id {
			Self::swap_other_to_base(who, supply_currency_id, supply_amount, acceptable_target_amount)
		} else if supply_currency_id == base_currency_id {
			Self::swap_base_to_other(who, target_currency_id, supply_amount, acceptable_target_amount)
		} else {
			Self::swap_other_to_other(
				who,
				supply_currency_id,
				supply_amount,
				target_currency_id,
				acceptable_target_amount,
			)
		}
	}

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
			Ratio::checked_from_rational(supply_amount, supply_amount.saturating_add(base_currency_pool))
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, _) = Self::liquidity_pool(supply_currency_id);

			// supply_amount / (supply_amount + other_currency_pool)
			Ratio::checked_from_rational(supply_amount, supply_amount.saturating_add(other_currency_pool))
		} else {
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			let (_, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);

			// first slippage in swap supply other currency to base currency:
			// first_slippage = supply_amount / (supply_amount + supply_other_currency_pool)
			let supply_to_base_slippage: Ratio =
				Ratio::checked_from_rational(supply_amount, supply_amount.saturating_add(supply_other_currency_pool))?;

			// second slippage in swap base currency to target other currency:
			// base_amount = first_slippage * supply_base_currency_pool
			// second_slippage = base_amount / (base_amount + target_base_currency_pool)
			let base_to_target_slippage: Ratio = Ratio::checked_from_rational(
				supply_to_base_slippage.saturating_mul_int(supply_base_currency_pool),
				supply_to_base_slippage
					.saturating_mul_int(supply_base_currency_pool)
					.saturating_add(target_base_currency_pool),
			)?;

			// final_slippage = first_slippage + (1 - first_slippage) * second_slippage
			let final_slippage: Ratio = supply_to_base_slippage.saturating_add(
				Ratio::from_natural(1)
					.saturating_sub(supply_to_base_slippage)
					.saturating_mul(base_to_target_slippage),
			);

			Some(final_slippage)
		}
	}
}

impl<T: Trait> OnEmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}
