//! # DEX Module
//!
//! ## Overview
//!
//! Built-in decentralized exchange modules in Acala network, the trading
//! mechanism refers to the design of Uniswap V2. In addition to being used for
//! trading, DEX also participates in CDP liquidation, which is faster than
//! liquidation by auction when the liquidity is sufficient. And providing
//! market making liquidity for DEX will also receive stable currency as
//! additional reward for its participation in the CDP liquidation.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, weights::Weight};
use frame_system::{self as system, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, One, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId,
};
use sp_std::{prelude::*, vec};
use support::{DEXManager, Price, Ratio};

mod benchmarking;
mod default_weight;
mod mock;
// mod tests;

pub trait WeightInfo {
	fn add_liquidity() -> Weight;
	fn withdraw_liquidity() -> Weight;
	fn swap_with_exact_supply() -> Weight;
	fn swap_with_exact_target() -> Weight;
}

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

	/// Currency for transfer currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// Allowed trading pair list.
	type EnabledTradingPairs: Get<Vec<(CurrencyId, CurrencyId)>>;

	/// Trading fee rate
	/// The first item of the tuple is the numerator of the fee rate, second
	/// item is the denominator, fee_rate = numerator / denominator
	type GetExchangeFee: Get<(u32, u32)>;

	/// The limit for length of trading path
	type TradingPathLimit: Get<usize>;

	/// The DEX's module id, keep all assets in DEX.
	type ModuleId: Get<ModuleId>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_event!(
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		Balance = Balance,
		CurrencyId = CurrencyId,
	{
		/// Add liquidity success. \[who, currency_id_0, pool_0_increment, currency_id_1, pool_1_increment, share_increment\]
		AddLiquidity(AccountId, CurrencyId, Balance, CurrencyId, Balance, Balance),
		/// Remove liquidity from the trading pool success. \[who, currency_id_0, pool_0_decrement, currency_id_1, pool_1_decrement, share_decrement\]
		RemoveLiquidity(AccountId, CurrencyId, Balance, CurrencyId, Balance, Balance),
		/// Use supply currency to swap target currency. \[trader, trading_path, supply_currency_amount, target_currency_amount\]
		Swap(AccountId, Vec<CurrencyId>, Balance, Balance),
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
		/// Invalid trading path length
		InvalidTradingPathLength,
		/// Target amount is less to min_target_amount
		InsufficientTargetAmount,
		/// Supply amount is more than max_supply_amount
		ExcessiveSupplyAmount,
		/// The swap will cause unacceptable price impact
		ExceedPriceImpactLimit,
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
		const GetExchangeFee: (u32, u32) = T::GetExchangeFee::get();

		/// The limit for length of trading path
		const TradingPathLimit: u32 = T::TradingPathLimit::get() as u32;

		/// The DEX's module id, keep all assets in DEX.
		const ModuleId: ModuleId = T::ModuleId::get();

		/// Trading with DEX, swap with exact supply amount
		///
		/// - `path`: trading path.
		/// - `supply_amount`: exact supply amount.
		/// - `min_target_amount`: acceptable minimum target amount.
		#[weight = <T as Trait>::WeightInfo::swap_with_exact_supply()]
		pub fn swap_with_exact_supply(
			origin,
			path: Vec<CurrencyId>,
			#[compact] supply_amount: Balance,
			#[compact] min_target_amount: Balance,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let _ = Self::do_swap_with_exact_supply(&who, path, supply_amount, min_target_amount, None)?;
				Ok(())
			})?;
		}

		/// Trading with DEX, swap with exact target amount
		///
		/// - `path`: trading path.
		/// - `target_amount`: exact target amount.
		/// - `max_supply_amount`: acceptable maxmum supply amount.
		#[weight = <T as Trait>::WeightInfo::swap_with_exact_target()]
		pub fn swap_with_exact_target(
			origin,
			path: Vec<CurrencyId>,
			#[compact] target_amount: Balance,
			#[compact] max_supply_amount: Balance,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				let _ = Self::do_swap_with_exact_target(&who, path, target_amount, max_supply_amount, None)?;
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

				ensure!(
					T::EnabledTradingPairs::get().contains(&(currency_id_a, currency_id_b))
					|| T::EnabledTradingPairs::get().contains(&(currency_id_b, currency_id_a)),
					Error::<T>::TradingPairNotAllowed,
				);

				let (currency_id_0, currency_id_1) = Self::sort_currency_id(currency_id_a, currency_id_b);
				let lp_share_currency_id = match (currency_id_0, currency_id_1) {
					(CurrencyId::Token(token_symbol_0), CurrencyId::Token(token_symbol_1)) => CurrencyId::DEXShare(token_symbol_0, token_symbol_1),
					_ => return Err(Error::<T>::InvalidCurrencyId.into()),
				};
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
							let price_0_1 = Price::checked_from_rational(*pool_1, *pool_0).unwrap_or_default();
							let input_price_0_1 = Price::checked_from_rational(max_amount_1, max_amount_0).unwrap_or_default();

							if input_price_0_1 <= price_0_1 {
								// max_amount_0 may be too much, calculate the actual amount_0
								let price_1_0 = Price::checked_from_rational(*pool_0, *pool_1).unwrap_or_default();
								let amount_0 = price_1_0.saturating_mul_int(max_amount_1);
								let share_increment = Ratio::checked_from_rational(amount_0, *pool_0)
									.and_then(|n| n.checked_mul_int(total_shares))
									.unwrap_or_default();
								(amount_0, max_amount_1, share_increment)
							} else {
								// max_amount_1 is too much, calculate the actual amount_1
								let amount_1 = price_0_1.saturating_mul_int(max_amount_0);
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
	fn account_id() -> T::AccountId {
		T::ModuleId::get().into_account()
	}

	/// Sort currency id by ascending order.
	fn sort_currency_id(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (CurrencyId, CurrencyId) {
		if currency_id_a > currency_id_b {
			(currency_id_b, currency_id_a)
		} else {
			(currency_id_a, currency_id_b)
		}
	}

	fn get_liquidity(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		let (currency_id_0, currency_id_1) = Self::sort_currency_id(currency_id_a, currency_id_b);
		let (pool_0, pool_1) = Self::liquidity_pool((currency_id_0, currency_id_1));

		if currency_id_a == currency_id_0 {
			(pool_0, pool_1)
		} else {
			(pool_1, pool_0)
		}
	}

	/// Get how much target amount will be got for specific supply amount and
	/// price impact
	fn get_target_amount(supply_pool: Balance, target_pool: Balance, supply_amount: Balance) -> Balance {
		if supply_amount.is_zero() || supply_pool.is_zero() || target_pool.is_zero() {
			Zero::zero()
		} else {
			let (fee_numerator, fee_denominator) = T::GetExchangeFee::get();
			let supply_amount_with_fee =
				supply_amount.saturating_mul(fee_denominator.saturating_sub(fee_numerator).unique_saturated_into());
			let numerator = supply_amount_with_fee.saturating_mul(target_pool);
			let denominator = supply_pool
				.saturating_mul(fee_denominator.unique_saturated_into())
				.saturating_add(supply_amount_with_fee);

			numerator.checked_div(denominator).unwrap_or_else(|| Zero::zero())
		}
	}

	/// Get how much supply amount will be paid for specific target amount.
	fn get_supply_amount(supply_pool: Balance, target_pool: Balance, target_amount: Balance) -> Balance {
		if target_amount.is_zero() || supply_pool.is_zero() || target_pool.is_zero() {
			Zero::zero()
		} else {
			let (fee_numerator, fee_denominator) = T::GetExchangeFee::get();
			let numerator = supply_pool
				.saturating_mul(target_amount)
				.saturating_mul(fee_denominator.unique_saturated_into());
			let denominator = target_pool
				.saturating_sub(target_amount)
				.saturating_mul(fee_denominator.saturating_sub(fee_numerator).unique_saturated_into());

			numerator
				.checked_div(denominator)
				.and_then(|r| r.checked_add(One::one()))
				.unwrap_or_else(|| Zero::zero()) // add 1 to result so that correct the possible
			                     // losses
			                     // caused by remainder discarding in
		}
	}

	fn get_target_amounts(
		path: Vec<CurrencyId>,
		supply_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Vec<Balance>, DispatchError> {
		let path_length = path.len();
		ensure!(
			path_length >= 2 && path_length <= T::TradingPathLimit::get(),
			Error::<T>::InvalidTradingPathLength
		);
		let mut target_amounts: Vec<Balance> = vec![Zero::zero(); path_length];
		target_amounts[0] = supply_amount;

		let mut i: usize = 0;
		while i + 1 < path_length {
			let (supply_pool, target_pool) = Self::get_liquidity(path[i], path[i + 1]);
			let target_amount = Self::get_target_amount(supply_pool, target_pool, target_amounts[i]);

			// check price impact if limit exists
			if let Some(limit) = price_impact_limit {
				let price_impact =
					Ratio::checked_from_rational(target_amount, target_pool).unwrap_or_else(|| Ratio::zero());
				ensure!(price_impact <= limit, Error::<T>::ExceedPriceImpactLimit);
			}

			target_amounts[i + 1] = target_amount;
			i += 1;
		}

		Ok(target_amounts)
	}

	fn get_supply_amounts(
		path: Vec<CurrencyId>,
		target_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Vec<Balance>, DispatchError> {
		let path_length = path.len();
		ensure!(
			path_length >= 2 && path_length <= T::TradingPathLimit::get(),
			Error::<T>::InvalidTradingPathLength
		);
		let mut supply_amounts: Vec<Balance> = vec![Zero::zero(); path_length];
		supply_amounts[path_length - 1] = target_amount;

		let mut i: usize = path_length - 1;
		while i > 0 {
			let (supply_pool, target_pool) = Self::get_liquidity(path[i - 1], path[i]);

			// check price impact if limit exists
			if let Some(limit) = price_impact_limit {
				let price_impact =
					Ratio::checked_from_rational(supply_amounts[i], target_pool).unwrap_or_else(|| Ratio::zero());
				ensure!(price_impact <= limit, Error::<T>::ExceedPriceImpactLimit);
			};

			let supply_amount = Self::get_supply_amount(supply_pool, target_pool, supply_amounts[i]);
			supply_amounts[i - 1] = supply_amount;
			i -= 1;
		}

		Ok(supply_amounts)
	}

	fn _swap(
		supply_currency_id: CurrencyId,
		target_currency_id: CurrencyId,
		supply_increment: Balance,
		target_decrement: Balance,
	) {
		let (currency_id_0, currency_id_1) = Self::sort_currency_id(supply_currency_id, target_currency_id);
		LiquidityPool::mutate((currency_id_0, currency_id_1), |(pool_0, pool_1)| {
			if supply_currency_id == currency_id_0 {
				*pool_0 = pool_0.saturating_add(supply_increment);
				*pool_1 = pool_1.saturating_sub(target_decrement);
			} else {
				*pool_0 = pool_0.saturating_sub(target_decrement);
				*pool_1 = pool_1.saturating_add(supply_increment);
			}
		});
	}

	fn _swap_by_path(path: Vec<CurrencyId>, amounts: Vec<Balance>) {
		let mut i: usize = 0;
		while i + 1 < path.len() {
			let (supply_currency_id, target_currency_id) = (path[i], path[i + 1]);
			let (supply_increment, target_decrement) = (amounts[i], amounts[i + 1]);
			Self::_swap(
				supply_currency_id,
				target_currency_id,
				supply_increment,
				target_decrement,
			);
			i += 1;
		}
	}

	fn do_swap_with_exact_supply(
		who: &T::AccountId,
		path: Vec<CurrencyId>,
		supply_amount: Balance,
		min_target_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		with_transaction_result(|| {
			let amounts = Self::get_target_amounts(path.clone(), supply_amount, price_impact_limit)?;
			ensure!(
				amounts[amounts.len() - 1] >= min_target_amount,
				Error::<T>::InsufficientTargetAmount
			);
			let module_account_id = Self::account_id();
			let actual_target_amount = amounts[amounts.len() - 1];

			T::Currency::transfer(path[0], who, &module_account_id, supply_amount)?;
			Self::_swap_by_path(path.clone(), amounts);
			T::Currency::transfer(path[path.len() - 1], &module_account_id, who, actual_target_amount)?;

			Self::deposit_event(RawEvent::Swap(who.clone(), path, supply_amount, actual_target_amount));
			Ok(actual_target_amount)
		})
	}

	fn do_swap_with_exact_target(
		who: &T::AccountId,
		path: Vec<CurrencyId>,
		target_amount: Balance,
		max_supply_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		with_transaction_result(|| {
			let amounts = Self::get_supply_amounts(path.clone(), target_amount, price_impact_limit)?;
			ensure!(amounts[0] <= max_supply_amount, Error::<T>::ExcessiveSupplyAmount);
			let module_account_id = Self::account_id();
			let actual_supply_amount = amounts[0];

			T::Currency::transfer(path[0], who, &module_account_id, actual_supply_amount)?;
			Self::_swap_by_path(path.clone(), amounts);
			T::Currency::transfer(path[path.len() - 1], &module_account_id, who, target_amount)?;

			Self::deposit_event(RawEvent::Swap(who.clone(), path, actual_supply_amount, target_amount));
			Ok(actual_supply_amount)
		})
	}
}

impl<T: Trait> DEXManager<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		Self::get_liquidity(currency_id_a, currency_id_b)
	}

	fn get_swap_target_amount(path: Vec<CurrencyId>, supply_amount: Balance) -> Option<Balance> {
		Self::get_target_amounts(path, supply_amount, None)
			.ok()
			.map(|amounts| amounts[amounts.len() - 1])
	}

	fn get_swap_supply_amount(path: Vec<CurrencyId>, target_amount: Balance) -> Option<Balance> {
		Self::get_supply_amounts(path, target_amount, None)
			.ok()
			.map(|amounts| amounts[0])
	}

	fn swap_with_exact_supply(
		who: &T::AccountId,
		path: Vec<CurrencyId>,
		supply_amount: Balance,
		min_target_amount: Balance,
		gas_price_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Self::do_swap_with_exact_supply(who, path, supply_amount, min_target_amount, gas_price_limit)
	}

	fn swap_with_exact_target(
		who: &T::AccountId,
		path: Vec<CurrencyId>,
		target_amount: Balance,
		max_supply_amount: Balance,
		gas_price_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Self::do_swap_with_exact_target(who, path, target_amount, max_supply_amount, gas_price_limit)
	}
}
