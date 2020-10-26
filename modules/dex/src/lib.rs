//! # DEX Module
//!
//! ## Overview
//!
//! Built-in decentralized exchange modules in Acala network, the swap
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
use primitives::{Balance, CurrencyId, TradingPair};
use sp_core::U256;
use sp_runtime::{
	traits::{AccountIdConversion, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult, FixedPointNumber, ModuleId,
};
use sp_std::{convert::TryInto, prelude::*, vec};
use support::{DEXManager, Price, Ratio};

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn add_liquidity() -> Weight;
	fn remove_liquidity() -> Weight;
	fn swap_with_exact_supply() -> Weight;
	fn swap_with_exact_target() -> Weight;
}

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;

	/// Currency for transfer currencies
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

	/// Allowed trading pair list.
	type EnabledTradingPairs: Get<Vec<TradingPair>>;

	/// Trading fee rate
	/// The first item of the tuple is the numerator of the fee rate, second
	/// item is the denominator, fee_rate = numerator / denominator,
	/// use (u32, u32) over `Rate` type to minimize internal division operation.
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
		/// Liquidity is not enough
		InsufficientLiquidity,
		/// The supply amount is zero
		ZeroSupplyAmount,
		/// The target amount is zero
		ZeroTargetAmount,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		/// Liquidity pool for specific pair(a tuple consisting of two sorted CurrencyIds).
		/// (CurrencyId_0, CurrencyId_1) -> (Amount_0, Amount_1)
		LiquidityPool get(fn liquidity_pool): map hasher(twox_64_concat) TradingPair => (Balance, Balance);
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// Allowed trading pair list
		const EnabledTradingPairs: Vec<TradingPair> = T::EnabledTradingPairs::get();

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
				let _ = Self::do_swap_with_exact_supply(&who, &path, supply_amount, min_target_amount, None)?;
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
				let _ = Self::do_swap_with_exact_target(&who, &path, target_amount, max_supply_amount, None)?;
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
				let trading_pair = TradingPair::new(currency_id_a, currency_id_b);
				ensure!(T::EnabledTradingPairs::get().contains(&trading_pair), Error::<T>::TradingPairNotAllowed);

				LiquidityPool::try_mutate(trading_pair, |(pool_0, pool_1)| -> DispatchResult {
					let lp_share_currency_id = trading_pair.get_dex_share_currency_id().ok_or(Error::<T>::InvalidCurrencyId)?;
					let total_shares = T::Currency::total_issuance(lp_share_currency_id);
					let (max_amount_0, max_amount_1) = if currency_id_a == trading_pair.0 {
						(max_amount_a, max_amount_b)
					} else {
						(max_amount_b, max_amount_a)
					};
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
					T::Currency::transfer(trading_pair.0, &who, &module_account_id, pool_0_increment)?;
					T::Currency::transfer(trading_pair.1, &who, &module_account_id, pool_1_increment)?;
					T::Currency::deposit(lp_share_currency_id, &who, share_increment)?;

					*pool_0 = pool_0.saturating_add(pool_0_increment);
					*pool_1 = pool_1.saturating_add(pool_1_increment);

					Self::deposit_event(RawEvent::AddLiquidity(
						who,
						trading_pair.0,
						pool_0_increment,
						trading_pair.1,
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
		#[weight = T::WeightInfo::remove_liquidity()]
		pub fn remove_liquidity(origin, currency_id_a: CurrencyId, currency_id_b: CurrencyId, #[compact] remove_share: Balance) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;
				if remove_share.is_zero() { return Ok(()); }
				let trading_pair = TradingPair::new(currency_id_a, currency_id_b);

				LiquidityPool::try_mutate(trading_pair, |(pool_0, pool_1)| -> DispatchResult {
					let lp_share_currency_id = trading_pair.get_dex_share_currency_id().ok_or(Error::<T>::InvalidCurrencyId)?;
					let total_shares = T::Currency::total_issuance(lp_share_currency_id);
					let proportion = Ratio::checked_from_rational(remove_share, total_shares).unwrap_or_default();
					let pool_0_decrement = proportion.saturating_mul_int(*pool_0);
					let pool_1_decrement = proportion.saturating_mul_int(*pool_1);
					let module_account_id = Self::account_id();

					T::Currency::withdraw(lp_share_currency_id, &who, remove_share)?;
					T::Currency::transfer(trading_pair.0, &module_account_id, &who, pool_0_decrement)?;
					T::Currency::transfer(trading_pair.1, &module_account_id, &who, pool_1_decrement)?;

					*pool_0 = pool_0.saturating_sub(pool_0_decrement);
					*pool_1 = pool_1.saturating_sub(pool_1_decrement);

					Self::deposit_event(RawEvent::RemoveLiquidity(
						who,
						trading_pair.0,
						pool_0_decrement,
						trading_pair.1,
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

	fn get_liquidity(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		let trading_pair = TradingPair::new(currency_id_a, currency_id_b);
		let (pool_0, pool_1) = Self::liquidity_pool(trading_pair);
		if currency_id_a == trading_pair.0 {
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
			let numerator: U256 = U256::from(supply_amount_with_fee).saturating_mul(U256::from(target_pool));
			let denominator: U256 = U256::from(supply_pool)
				.saturating_mul(U256::from(fee_denominator))
				.saturating_add(U256::from(supply_amount_with_fee));

			numerator
				.checked_div(denominator)
				.and_then(|n| TryInto::<Balance>::try_into(n).ok())
				.unwrap_or_else(Zero::zero)
		}
	}

	/// Get how much supply amount will be paid for specific target amount.
	fn get_supply_amount(supply_pool: Balance, target_pool: Balance, target_amount: Balance) -> Balance {
		if target_amount.is_zero() || supply_pool.is_zero() || target_pool.is_zero() {
			Zero::zero()
		} else {
			let (fee_numerator, fee_denominator) = T::GetExchangeFee::get();
			let numerator: U256 = U256::from(supply_pool)
				.saturating_mul(U256::from(target_amount))
				.saturating_mul(U256::from(fee_denominator));
			let denominator: U256 = U256::from(target_pool)
				.saturating_sub(U256::from(target_amount))
				.saturating_mul(U256::from(fee_denominator.saturating_sub(fee_numerator)));

			numerator
				.checked_div(denominator)
				.and_then(|r| r.checked_add(U256::one())) // add 1 to result so that correct the possible losses caused by remainder discarding in
				.and_then(|n| TryInto::<Balance>::try_into(n).ok())
				.unwrap_or_else(Zero::zero)
		}
	}

	fn get_target_amounts(
		path: &[CurrencyId],
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
			ensure!(
				T::EnabledTradingPairs::get().contains(&TradingPair::new(path[i], path[i + 1])),
				Error::<T>::TradingPairNotAllowed
			);
			let (supply_pool, target_pool) = Self::get_liquidity(path[i], path[i + 1]);
			ensure!(
				!supply_pool.is_zero() && !target_pool.is_zero(),
				Error::<T>::InsufficientLiquidity
			);
			let target_amount = Self::get_target_amount(supply_pool, target_pool, target_amounts[i]);
			ensure!(!target_amount.is_zero(), Error::<T>::ZeroTargetAmount);

			// check price impact if limit exists
			if let Some(limit) = price_impact_limit {
				let price_impact = Ratio::checked_from_rational(target_amount, target_pool).unwrap_or_else(Ratio::zero);
				ensure!(price_impact <= limit, Error::<T>::ExceedPriceImpactLimit);
			}

			target_amounts[i + 1] = target_amount;
			i += 1;
		}

		Ok(target_amounts)
	}

	fn get_supply_amounts(
		path: &[CurrencyId],
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
			ensure!(
				T::EnabledTradingPairs::get().contains(&TradingPair::new(path[i - 1], path[i])),
				Error::<T>::TradingPairNotAllowed
			);
			let (supply_pool, target_pool) = Self::get_liquidity(path[i - 1], path[i]);
			ensure!(
				!supply_pool.is_zero() && !target_pool.is_zero(),
				Error::<T>::InsufficientLiquidity
			);
			let supply_amount = Self::get_supply_amount(supply_pool, target_pool, supply_amounts[i]);
			ensure!(!supply_amount.is_zero(), Error::<T>::ZeroSupplyAmount);

			// check price impact if limit exists
			if let Some(limit) = price_impact_limit {
				let price_impact =
					Ratio::checked_from_rational(supply_amounts[i], target_pool).unwrap_or_else(Ratio::zero);
				ensure!(price_impact <= limit, Error::<T>::ExceedPriceImpactLimit);
			};

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
		let trading_pair = TradingPair::new(supply_currency_id, target_currency_id);
		LiquidityPool::mutate(trading_pair, |(pool_0, pool_1)| {
			if supply_currency_id == trading_pair.0 {
				*pool_0 = pool_0.saturating_add(supply_increment);
				*pool_1 = pool_1.saturating_sub(target_decrement);
			} else {
				*pool_0 = pool_0.saturating_sub(target_decrement);
				*pool_1 = pool_1.saturating_add(supply_increment);
			}
		});
	}

	fn _swap_by_path(path: &[CurrencyId], amounts: &[Balance]) {
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
		path: &[CurrencyId],
		supply_amount: Balance,
		min_target_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		with_transaction_result(|| {
			let amounts = Self::get_target_amounts(&path, supply_amount, price_impact_limit)?;
			ensure!(
				amounts[amounts.len() - 1] >= min_target_amount,
				Error::<T>::InsufficientTargetAmount
			);
			let module_account_id = Self::account_id();
			let actual_target_amount = amounts[amounts.len() - 1];

			T::Currency::transfer(path[0], who, &module_account_id, supply_amount)?;
			Self::_swap_by_path(&path, &amounts);
			T::Currency::transfer(path[path.len() - 1], &module_account_id, who, actual_target_amount)?;

			Self::deposit_event(RawEvent::Swap(
				who.clone(),
				path.to_vec(),
				supply_amount,
				actual_target_amount,
			));
			Ok(actual_target_amount)
		})
	}

	fn do_swap_with_exact_target(
		who: &T::AccountId,
		path: &[CurrencyId],
		target_amount: Balance,
		max_supply_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		with_transaction_result(|| {
			let amounts = Self::get_supply_amounts(&path, target_amount, price_impact_limit)?;
			ensure!(amounts[0] <= max_supply_amount, Error::<T>::ExcessiveSupplyAmount);
			let module_account_id = Self::account_id();
			let actual_supply_amount = amounts[0];

			T::Currency::transfer(path[0], who, &module_account_id, actual_supply_amount)?;
			Self::_swap_by_path(&path, &amounts);
			T::Currency::transfer(path[path.len() - 1], &module_account_id, who, target_amount)?;

			Self::deposit_event(RawEvent::Swap(
				who.clone(),
				path.to_vec(),
				actual_supply_amount,
				target_amount,
			));
			Ok(actual_supply_amount)
		})
	}
}

impl<T: Trait> DEXManager<T::AccountId, CurrencyId, Balance> for Module<T> {
	fn get_liquidity_pool(currency_id_a: CurrencyId, currency_id_b: CurrencyId) -> (Balance, Balance) {
		Self::get_liquidity(currency_id_a, currency_id_b)
	}

	fn get_swap_target_amount(
		path: &[CurrencyId],
		supply_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> Option<Balance> {
		Self::get_target_amounts(&path, supply_amount, price_impact_limit)
			.ok()
			.map(|amounts| amounts[amounts.len() - 1])
	}

	fn get_swap_supply_amount(
		path: &[CurrencyId],
		target_amount: Balance,
		price_impact_limit: Option<Ratio>,
	) -> Option<Balance> {
		Self::get_supply_amounts(&path, target_amount, price_impact_limit)
			.ok()
			.map(|amounts| amounts[0])
	}

	fn swap_with_exact_supply(
		who: &T::AccountId,
		path: &[CurrencyId],
		supply_amount: Balance,
		min_target_amount: Balance,
		gas_price_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Self::do_swap_with_exact_supply(who, path, supply_amount, min_target_amount, gas_price_limit)
	}

	fn swap_with_exact_target(
		who: &T::AccountId,
		path: &[CurrencyId],
		target_amount: Balance,
		max_supply_amount: Balance,
		gas_price_limit: Option<Ratio>,
	) -> sp_std::result::Result<Balance, DispatchError> {
		Self::do_swap_with_exact_target(who, path, target_amount, max_supply_amount, gas_price_limit)
	}
}
