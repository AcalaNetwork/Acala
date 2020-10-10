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

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, weights::Weight};
use frame_system::{self as system, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, CheckedMul, CheckedSub, One, Saturating, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId,
};
use sp_std::prelude::Vec;
use support::{CDPTreasury, DEXManager, Price, Rate, Ratio};

mod benchmarking;
mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn add_liquidity() -> Weight;
	fn withdraw_liquidity() -> Weight;
	fn swap_currency() -> Weight;
}

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

	/// Currency for transfer currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// CDP treasury for depositing additional liquidity reward to DEX
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = Balance, CurrencyId = CurrencyId>;

	/// Allowed trading pair list, must check the list is correct before
	/// configure.
	type EnabledTradingPairs: Get<Vec<(CurrencyId, CurrencyId)>>;

	/// Trading fee rate
	/// The first item of the tuple is the numerator of the fee rate, second
	/// item is the denominator, fee_rate = numerator / denominator
	type GetExchangeFee: Get<(u128, u128)>;

	/// The DEX's module id, keep all assets in DEX.
	type ModuleId: Get<ModuleId>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Add liquidity success. \[who, currency_id_0, pool_0_increment, currency_id_1, pool_1_increment, share_increment\]
		AddLiquidity(AccountId, CurrencyId, Balance, CurrencyId, Balance, Balance),
		/// Remove liquidity from the trading pool success. \[who, currency_id_0, pool_0_decrement, currency_id_1, pool_1_decrement, share_decrement\]
		RemoveLiquidity(AccountId, CurrencyId, Balance, CurrencyId, Balance, Balance),
		/// Use supply currency to swap target currency. \[trader, supply_currency_type, supply_currency_amount, target_currency_type, target_currency_amount\]
		Swap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for dex module.
	pub enum Error for Module<T: Trait> {
		/// Not the enable trading pair
		TradingPairNotAllowed,
		/// The actual transaction price will be lower than the acceptable price
		UnacceptablePrice,
		/// The increment of liquidity is invalid
		InvalidLiquidityIncrement,
		/// Invalid currency id
		InvalidCurrencyId,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		/// Liquidity pool for specific pair(a tuple consisting of two sorted CurrencyIds).
		/// (CurrencyId_0, CurrencyId_1) -> (Amount_0, Amount_1)
		LiquidityPool get(fn liquidity_pool): map hasher(twox_64_concat) (CurrencyId, CurrencyId) => (Balance, Balance);
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Allowed trading pair list
		const EnabledTradingPairs: Vec<(CurrencyId, CurrencyId)> = T::EnabledTradingPairs::get();

		/// Trading fee rate
		const GetExchangeFee: (u128, u128) = T::GetExchangeFee::get();

		/// The DEX's module id, keep all assets in DEX.
		const ModuleId: ModuleId = T::ModuleId::get();

		/// Trading with DEX, swap supply currency to target currency
		///
		/// - `supply_currency_id`: supply currency type.
		/// - `supply_amount`: supply currency amount.
		/// - `target_currency_id`: target currency type.
		/// - `acceptable_target_amount`: acceptable target amount, if actual amount is under it, swap will not happen
		#[weight = <T as Trait>::WeightInfo::swap_currency()]
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
		/// - `currency_id_a`: currency id A.
		/// - `currency_id_b`: currency id B.
		/// - `max_amount_a`: maximum currency A amount allowed to inject to liquidity pool.
		/// - `max_amount_b`: maximum currency A amount allowed to inject to liquidity pool.
		#[weight = T::WeightInfo::add_liquidity()]
		pub fn add_liquidity(
			origin,
			currency_id_a: CurrencyId,
			currency_id_b: CurrencyId,
			#[compact] max_amount_a: Balance,
			#[compact] max_amount_b: Balance,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;

				let (currency_id_0, currency_id_1) = Self::sort_currency_id(currency_id_a, currency_id_b);
				let lp_share_currency_id = match (currency_id_0, currency_id_1) {
					(CurrencyId::Token(token_symbol_0), CurrencyId::Token(token_symbol_1)) => CurrencyId::DEXShare(token_symbol_0, token_symbol_1),
					_ => return Err(Error::<T>::InvalidCurrencyId.into()),
				};
				ensure!(
					T::EnabledTradingPairs::get().contains(&(currency_id_0, currency_id_1)),
					Error::<T>::TradingPairNotAllowed,
				);

				let (max_amount_0, max_amount_1) = if currency_id_a == currency_id_0 {
					(max_amount_a, max_amount_b)
				} else {
					(max_amount_b, max_amount_a)
				};

				LiquidityPool::try_mutate((currency_id_0, currency_id_1), |(pool_0, pool_1)| -> DispatchResult {
					let total_shares = T::Currency::total_issuance(lp_share_currency_id);
					let (pool_0_increment, pool_1_increment, share_increment): (Balance, Balance, Balance) =
						if total_shares.is_zero() {
							// initialize this liquidity pool, the initial share is equal to the max value between base currency amount and other currency amount
							let initial_share = sp_std::cmp::max(max_amount_0, max_amount_1);
							(max_amount_0, max_amount_1, initial_share)
						} else {
							let 0_1_price = Price::checked_from_rational(*pool_1, *pool_0).unwrap_or_default();
							let input_0_1_price = Price::checked_from_rational(max_amount_1, max_amount_0).unwrap_or_default();

							if input_0_1_price <= 0_1_price {
								// max_amount_0 may be too much, calculate the actual amount_0
								let 1_0_price = Price::checked_from_rational(*pool_0, *pool_1).unwrap_or_default();
								let amount_0 = 1_0_price.saturating_mul_int(max_amount_1);
								let share_increment = Ratio::checked_from_rational(amount_0, *pool_0)
									.and_then(|n| n.checked_mul_int(total_shares))
									.unwrap_or_default();
								(amount_0, max_amount_1, share_increment)
							} else {
								// max_amount_1 is too much, calculate the actual amount_1
								let amount_1 = 0_1_price.saturating_mul_int(max_amount_0);
								let share_increment = Ratio::checked_from_rational(amount_1, *pool_1)
									.and_then(|n| n.checked_mul_int(total_shares))
									.unwrap_or_default();
								(max_amount_0, amount_1, share_increment)
							}
						};

					ensure!(
						!share_increment.is_zero() && !pool_0_increment.is_zero() && !pool_1_increment.is_zero(),
						Error::<T>::InvalidLiquidityIncrement,
					);

					let module_account_id = Self::account_id();
					T::Currency::transfer(currency_id_0, &who, &module_account_id, pool_0_increment)?;
					T::Currency::transfer(currency_id_1, &who, &module_account_id, pool_1_increment)?;
					T::Currency::deposit(lp_share_currency_id, &who, share_increment)?;

					*pool_0 = pool_0.saturating_add(pool_0_increment);
					*pool_1 = pool_1.saturating_add(pool_1_increment);

					Self::deposit_event(RawEvent::AddLiquidity(
						who,
						currency_id_0,
						pool_0_increment,
						currency_id_1,
						pool_1_increment,
						share_increment,
					));
					Ok(())
				})
			})?;
		}

		/// Remove liquidity from specific liquidity pool in the form of burning shares, and withdrawing currencies in trading pairs
		/// from liquidity pool in proportion, and withdraw liquidity incentive interest.
		///
		/// - `currency_id_a`: currency id A.
		/// - `currency_id_b`: currency id B.
		/// - `remove_share`: liquidity amount to remove.
		#[weight = T::WeightInfo::withdraw_liquidity()]
		pub fn withdraw_liquidity(origin, currency_id_a: CurrencyId, currency_id_b: CurrencyId, #[compact] remove_share: Balance) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				if remove_share.is_zero() { return Ok(()); }

				let (currency_id_0, currency_id_1) = Self::sort_currency_id(currency_id_a, currency_id_b);
				let lp_share_currency_id = match (currency_id_0, currency_id_1) {
					(CurrencyId::Token(token_symbol_0), CurrencyId::Token(token_symbol_1)) => CurrencyId::DEXShare(token_symbol_0, token_symbol_1),
					_ => return Err(Error::<T>::InvalidCurrencyId.into()),
				};

				LiquidityPool::try_mutate((currency_id_0, currency_id_1), |(pool_0, pool_1)| -> DispatchResult {
					let total_shares = T::Currency::total_issuance(lp_share_currency_id);
					let proportion = Ratio::checked_from_rational(remove_share, total_shares).unwrap_or_default();
					let pool_0_decrement = proportion.saturating_mul_int(*pool_0);
					let pool_1_decrement = proportion.saturating_mul_int(*pool_1);

					T::Currency::withdraw(lp_share_currency_id, &who, remove_share)?;

					let module_account_id = Self::account_id();
					T::Currency::transfer(currency_id_0, &module_account_id, &who, pool_0_decrement)?;
					T::Currency::transfer(currency_id_1, &module_account_id, &who, pool_1_decrement)?;

					*pool_0 = pool_0.saturating_sub(pool_0_decrement);
					*pool_1 = pool_1.saturating_sub(pool_1_decrement);

					Self::deposit_event(RawEvent::RemoveLiquidity(
						who,
						currency_id_0,
						pool_0_decrement,
						currency_id_1,
						pool_1_decrement,
						remove_share,
					));
					Ok(())
				})
			})?;
		}
	}
}

impl<T: Trait> Module<T> {
	/// Sort currency id by ascending order.
	fn sort_currency_id(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (CurrencyId, CurrencyId) {
		if currency_id_a > currency_id_b {
			(currency_id_b, currency_id_a)
		} else {
			(currency_id_a, currency_id_b)
		}
	}

	fn account_id() -> T::AccountId {
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

		let module_account_id = Self::account_id();
		// transfer token between account and dex and update liquidity pool
		T::Currency::transfer(other_currency_id, who, &module_account_id, other_currency_amount)?;
		T::Currency::transfer(base_currency_id, &module_account_id, who, base_currency_amount)?;

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

		let module_account_id = Self::account_id();
		T::Currency::transfer(base_currency_id, who, &module_account_id, base_currency_amount)?;
		T::Currency::transfer(other_currency_id, &module_account_id, who, other_currency_amount)?;
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

		let module_account_id = Self::account_id();
		T::Currency::transfer(
			supply_other_currency_id,
			who,
			&module_account_id,
			supply_other_currency_amount,
		)?;
		T::Currency::transfer(
			target_other_currency_id,
			&module_account_id,
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

	/// get the minimum amount of supply currency needed for the target currency
	/// amount return None means cannot exchange
	pub fn get_supply_amount_needed(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Option<Balance> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let fee_rate = T::GetExchangeFee::get();
		let val = if supply_currency_id == target_currency_id {
			return None;
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
		};
		Some(val)
	}

	/// get the maximum amount of target currency you can get for the supply
	/// currency amount return None means cannot exchange
	pub fn get_target_amount_available(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Option<Balance> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		let fee_rate = T::GetExchangeFee::get();
		let val = if supply_currency_id == target_currency_id {
			return None;
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
		};
		Some(val)
	}
}

impl<T: Trait> DEXManager<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn get_target_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_currency_amount: Balance,
	) -> Option<Balance> {
		Self::get_target_amount_available(supply_currency_id, target_currency_id, supply_currency_amount)
	}

	fn get_supply_amount(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		target_currency_amount: Balance,
	) -> Option<Balance> {
		Self::get_supply_amount_needed(supply_currency_id, target_currency_id, target_currency_amount)
	}

	fn exchange_currency(
		who: T::AccountId,
		supply_currency_id: CurrencyId,
		supply_amount: Balance,
		target_currency_id: CurrencyId,
		acceptable_target_amount: Balance,
	) -> sp_std::result::Result<Balance, DispatchError> {
		with_transaction_result(|| {
			Self::do_exchange(
				&who,
				supply_currency_id,
				supply_amount,
				target_currency_id,
				acceptable_target_amount,
			)
		})
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

	fn get_liquidity_pool(currency_id: CurrencyId) -> (Balance, Balance) {
		Self::liquidity_pool(currency_id)
	}
}
