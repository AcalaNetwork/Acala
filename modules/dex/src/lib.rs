#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, Parameter};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use rstd::prelude::Vec;
use sp_runtime::{
	traits::{
		AccountIdConversion, AtLeast32Bit, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, Saturating,
		UniqueSaturatedInto, Zero,
	},
	DispatchError, DispatchResult, ModuleId,
};
use support::{CDPTreasury, DEXManager, Price, Rate, Ratio};
use system::{self as system, ensure_signed};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/dexm");

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type Share: Into<BalanceOf<Self>>
		+ From<BalanceOf<Self>>
		+ Parameter
		+ Member
		+ AtLeast32Bit
		+ Default
		+ Copy
		+ MaybeSerializeDeserialize;
	type EnabledCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type GetBaseCurrencyId: Get<CurrencyIdOf<Self>>;
	type GetExchangeFee: Get<Rate>;
	type CDPTreasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Share,
		Balance = BalanceOf<T>,
		CurrencyId = CurrencyIdOf<T>,
	{
		AddLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		WithdrawLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		Swap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for dex module.
	pub enum Error for Module<T: Trait> {
		CurrencyIdNotAllowed,
		TokenNotEnough,
		ShareNotEnough,
		InvalidBalance,
		CanNotSwapItself,
		InacceptablePrice,
		InvalidLiquidityIncrement,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		LiquidityIncentiveRate get(fn liquidity_incentive_rate) config(): Rate;
		LiquidityPool get(fn liquidity_pool): map hasher(twox_64_concat) CurrencyIdOf<T> => (BalanceOf<T>, BalanceOf<T>);
		TotalShares get(fn total_shares): map hasher(twox_64_concat) CurrencyIdOf<T> => T::Share;
		Shares get(fn shares): double_map hasher(twox_64_concat) CurrencyIdOf<T>, hasher(twox_64_concat) T::AccountId => T::Share;
		TotalDebits get(fn total_debits): map hasher(twox_64_concat) CurrencyIdOf<T> => T::Share;
		Debits get(fn debits): double_map hasher(twox_64_concat) CurrencyIdOf<T>, hasher(twox_64_concat) T::AccountId => T::Share;
		TotalInterest get(fn total_interest): map hasher(twox_64_concat) CurrencyIdOf<T> => T::Share;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		const EnabledCurrencyIds: Vec<CurrencyIdOf<T>> = T::EnabledCurrencyIds::get();
		const GetBaseCurrencyId: CurrencyIdOf<T> = T::GetBaseCurrencyId::get();
		const GetExchangeFee: Rate = T::GetExchangeFee::get();

		pub fn swap_currency(
			origin,
			supply_currency_id: CurrencyIdOf<T>,
			#[compact] supply_amount: BalanceOf<T>,
			target_currency_id: CurrencyIdOf<T>,
			#[compact] acceptable_target_amount: BalanceOf<T>,
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

		pub fn add_liquidity(
			origin,
			other_currency_id: CurrencyIdOf<T>,
			#[compact] max_other_currency_amount: BalanceOf<T>,
			#[compact] max_base_currency_amount: BalanceOf<T>
		) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();
			ensure!(
				T::EnabledCurrencyIds::get().contains(&other_currency_id),
				Error::<T>::CurrencyIdNotAllowed,
			);
			ensure!(
				!max_other_currency_amount.is_zero() && !max_base_currency_amount.is_zero(),
				Error::<T>::InvalidBalance,
			);

			let total_shares = Self::total_shares(other_currency_id);
			let (other_currency_increment, base_currency_increment, share_increment): (BalanceOf<T>, BalanceOf<T>, T::Share) =
			if total_shares.is_zero() {
				// initialize this liquidity pool, the initial share is equal to the max value between base currency amount and other currency amount
				let initial_share: u128 = rstd::cmp::max(max_other_currency_amount, max_base_currency_amount).unique_saturated_into();
				let initial_share: T::Share = initial_share.unique_saturated_into();

				(max_other_currency_amount, max_base_currency_amount, initial_share)
			} else {
				let (other_currency_pool, base_currency_pool): (BalanceOf<T>, BalanceOf<T>) = Self::liquidity_pool(other_currency_id);
				let other_base_price = Price::from_rational(base_currency_pool, other_currency_pool);
				let input_other_base_price = Price::from_rational(max_base_currency_amount, max_other_currency_amount);

				if input_other_base_price <= other_base_price {
					// max_other_currency_amount may be too much, calculate the actual other currency amount
					let base_other_price = Price::from_rational(other_currency_pool, base_currency_pool);
					let other_currency_amount = base_other_price.saturating_mul_int(&max_base_currency_amount);
					let share = Ratio::from_rational(other_currency_amount, other_currency_pool).checked_mul_int(&total_shares).unwrap_or_default();
					(other_currency_amount, max_base_currency_amount, share)
				} else {
					// max_base_currency_amount is too much, calculate the actual base currency amount
					let base_currency_amount = other_base_price.saturating_mul_int(&max_other_currency_amount);
					let share = Ratio::from_rational(base_currency_amount, base_currency_pool).checked_mul_int(&total_shares).unwrap_or_default();
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
				Error::<T>::TokenNotEnough,
			);
			T::Currency::transfer(other_currency_id, &who, &Self::account_id(), other_currency_increment)
			.expect("never failed because after checks");
			T::Currency::transfer(base_currency_id, &who, &Self::account_id(), base_currency_increment)
			.expect("never failed because after checks");

			Self::deposit_calculate_interest(other_currency_id, &who, share_increment);
			<TotalShares<T>>::mutate(other_currency_id, |share| *share = share.saturating_add(share_increment));
			<Shares<T>>::mutate(other_currency_id, &who, |share| *share = share.saturating_add(share_increment));
			<LiquidityPool<T>>::mutate(other_currency_id, |pool| {
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

		pub fn withdraw_liquidity(origin, currency_id: CurrencyIdOf<T>, #[compact] share_amount: T::Share) {
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

			let (other_currency_pool, base_currency_pool): (BalanceOf<T>, BalanceOf<T>) = Self::liquidity_pool(currency_id);
			let proportion = Ratio::from_rational(share_amount, Self::total_shares(currency_id));
			let withdraw_other_currency_amount = proportion.saturating_mul_int(&other_currency_pool);
			let withdraw_base_currency_amount = proportion.saturating_mul_int(&base_currency_pool);
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
			<LiquidityPool<T>>::mutate(currency_id, |pool| {
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

		fn on_initialize(_n: T::BlockNumber) {
			for currency_id in T::EnabledCurrencyIds::get() {
				Self::accumulate_interest(currency_id);
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn account_id() -> T::AccountId {
		MODULE_ID.into_account()
	}

	pub fn calculate_swap_target_amount(
		supply_pool: BalanceOf<T>,
		target_pool: BalanceOf<T>,
		supply_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		// new_target_pool = supply_pool * target_pool / (supply_amount + supply_pool)
		let new_target_pool = supply_pool
			.checked_add(&supply_amount)
			.and_then(|n| Some(Ratio::from_rational(supply_pool, n)))
			.and_then(|n| n.checked_mul_int(&target_pool))
			.unwrap_or_default();

		// new_target_pool should be more then 0
		if !new_target_pool.is_zero() {
			// actual can get = (target_pool - new_target_pool) * (1 - GetExchangeFee)
			target_pool
				.checked_sub(&new_target_pool)
				.and_then(|n| n.checked_sub(&T::GetExchangeFee::get().saturating_mul_int(&n)))
				.unwrap_or_default()
		} else {
			0.into()
		}
	}

	pub fn calculate_swap_supply_amount(
		supply_pool: BalanceOf<T>,
		target_pool: BalanceOf<T>,
		target_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
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
				.and_then(|n| Ratio::from_parts(1).checked_add(&n)) // add Ratio::from_parts(1) to correct the possible losses caused by discarding the remainder in inner division
				.and_then(|n| n.checked_mul_int(&target_amount))
				.and_then(|n| n.checked_add(&1.into())) // add 1 to correct the possible losses caused by discarding the remainder in division
				.and_then(|n| target_pool.checked_sub(&n))
				.and_then(|n| Some(Ratio::from_rational(supply_pool, n)))
				.and_then(|n| Ratio::from_parts(1).checked_add(&n)) // add Ratio::from_parts(1) to correct the possible losses caused by discarding the remainder in inner division
				.and_then(|n| n.checked_mul_int(&target_pool))
				.and_then(|n| n.checked_add(&1.into())) // add 1 to correct the possible losses caused by discarding the remainder in division
				.and_then(|n| n.checked_sub(&supply_pool))
				.unwrap_or_default()
		}
	}

	// use other currency to swap base currency
	pub fn swap_other_to_base(
		who: T::AccountId,
		other_currency_id: CurrencyIdOf<T>,
		other_currency_amount: BalanceOf<T>,
		acceptable_base_currency_amount: BalanceOf<T>,
	) -> rstd::result::Result<BalanceOf<T>, DispatchError> {
		// 1. ensure supply amount must > 0 and account has sufficient balance
		ensure!(
			!other_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(other_currency_id, &who, other_currency_amount).is_ok(),
			Error::<T>::TokenNotEnough,
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
		<LiquidityPool<T>>::mutate(other_currency_id, |pool| {
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
		other_currency_id: CurrencyIdOf<T>,
		base_currency_amount: BalanceOf<T>,
		acceptable_other_currency_amount: BalanceOf<T>,
	) -> rstd::result::Result<BalanceOf<T>, DispatchError> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		ensure!(
			!base_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(base_currency_id, &who, base_currency_amount).is_ok(),
			Error::<T>::TokenNotEnough,
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
		<LiquidityPool<T>>::mutate(other_currency_id, |pool| {
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
		supply_other_currency_id: CurrencyIdOf<T>,
		supply_other_currency_amount: BalanceOf<T>,
		target_other_currency_id: CurrencyIdOf<T>,
		acceptable_target_other_currency_amount: BalanceOf<T>,
	) -> rstd::result::Result<BalanceOf<T>, DispatchError> {
		ensure!(
			!supply_other_currency_amount.is_zero()
				&& T::Currency::ensure_can_withdraw(supply_other_currency_id, &who, supply_other_currency_amount)
					.is_ok(),
			Error::<T>::TokenNotEnough,
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
		<LiquidityPool<T>>::mutate(supply_other_currency_id, |pool| {
			*pool = (
				pool.0.saturating_add(supply_other_currency_amount),
				pool.1.saturating_sub(intermediate_base_currency_amount),
			);
		});
		<LiquidityPool<T>>::mutate(target_other_currency_id, |pool| {
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
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		target_currency_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		if supply_currency_id == target_currency_id {
			0.into()
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
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		supply_currency_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		if supply_currency_id == target_currency_id {
			0.into()
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

	pub fn deposit_calculate_interest(currency_id: CurrencyIdOf<T>, who: &T::AccountId, share_amount: T::Share) {
		let total_shares = Self::total_shares(currency_id);
		if total_shares.is_zero() {
			return;
		}
		let proportion = Ratio::from_rational(share_amount, total_shares);
		let total_interest = Self::total_interest(currency_id);
		if total_interest.is_zero() {
			return;
		}
		let new_debits = proportion.saturating_mul_int(&total_interest);
		<Debits<T>>::mutate(currency_id, who, |debits| {
			*debits = debits.saturating_add(new_debits);
		});
		<TotalDebits<T>>::mutate(currency_id, |total_debits| {
			*total_debits = total_debits.saturating_add(new_debits);
		});
		<TotalInterest<T>>::mutate(currency_id, |interest| {
			*interest = interest.saturating_add(new_debits);
		});
	}

	fn withdraw_calculate_interest(
		currency_id: CurrencyIdOf<T>,
		who: &T::AccountId,
		share_amount: T::Share,
	) -> DispatchResult {
		Self::claim_interest(currency_id, who)?;
		let shares = Self::shares(currency_id, who);
		let debits = Self::debits(currency_id, who);
		let proportion = Ratio::from_rational(share_amount, Self::total_shares(currency_id));
		let remove_debits = Ratio::from_rational(share_amount, shares).saturating_mul_int(&debits);

		<Debits<T>>::mutate(currency_id, who, |debits| {
			*debits = debits.saturating_sub(remove_debits);
		});
		<TotalDebits<T>>::mutate(currency_id, |total_debits| {
			*total_debits = total_debits.saturating_sub(remove_debits);
		});
		<TotalInterest<T>>::mutate(currency_id, |interest| {
			*interest = interest.saturating_sub(proportion.saturating_mul_int(interest));
		});
		Ok(())
	}

	fn claim_interest(currency_id: CurrencyIdOf<T>, who: &T::AccountId) -> DispatchResult {
		let shares = Self::shares(currency_id, who);
		let debits = Self::debits(currency_id, who);
		let proportion = Ratio::from_rational(shares, Self::total_shares(currency_id));
		let total_interest = Self::total_interest(currency_id);
		let withdrawn_interest = proportion.saturating_mul_int(&total_interest).saturating_sub(debits);
		<Debits<T>>::mutate(currency_id, who, |debits| {
			*debits = debits.saturating_add(withdrawn_interest);
		});
		<TotalDebits<T>>::mutate(currency_id, |debits| {
			*debits = debits.saturating_add(withdrawn_interest);
		});
		T::CDPTreasury::deposit_unbacked_debit(who, withdrawn_interest.into())
	}

	fn accumulate_interest(currency_id: CurrencyIdOf<T>) {
		let (_, base_currency_pool) = Self::liquidity_pool(currency_id);
		let total_debits = Self::total_debits(currency_id);
		let total_interest = Self::total_interest(currency_id);
		let total = base_currency_pool
			.unique_saturated_into()
			.saturating_add(total_interest.unique_saturated_into())
			.saturating_sub(total_debits.unique_saturated_into());

		let new_interest = Self::liquidity_incentive_rate().saturating_mul_int(&total);
		<TotalInterest<T>>::mutate(currency_id, |interest| {
			*interest = interest.saturating_add(new_interest.unique_saturated_into());
		});
	}
}

impl<T: Trait> DEXManager<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>> for Module<T> {
	fn get_target_amount(
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		supply_currency_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		Self::get_target_amount_available(supply_currency_id, target_currency_id, supply_currency_amount)
	}

	fn get_supply_amount(
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		target_currency_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		Self::get_supply_amount_needed(supply_currency_id, target_currency_id, target_currency_amount)
	}

	fn exchange_currency(
		who: T::AccountId,
		supply_currency_id: CurrencyIdOf<T>,
		supply_amount: BalanceOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		acceptable_target_amount: BalanceOf<T>,
	) -> rstd::result::Result<BalanceOf<T>, DispatchError> {
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
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		supply_amount: BalanceOf<T>,
	) -> Option<Ratio> {
		let base_currency_id = T::GetBaseCurrencyId::get();

		if supply_currency_id == target_currency_id {
			None
		} else if supply_currency_id == base_currency_id {
			let (_, base_currency_pool) = Self::liquidity_pool(target_currency_id);

			// supply_amount / (supply_amount + base_currency_pool)
			Some(Ratio::from_rational(
				supply_amount,
				supply_amount.saturating_add(base_currency_pool),
			))
		} else if target_currency_id == base_currency_id {
			let (other_currency_pool, _) = Self::liquidity_pool(supply_currency_id);

			// supply_amount / (supply_amount + other_currency_pool)
			Some(Ratio::from_rational(
				supply_amount,
				supply_amount.saturating_add(other_currency_pool),
			))
		} else {
			let (supply_other_currency_pool, supply_base_currency_pool) = Self::liquidity_pool(supply_currency_id);
			let (_, target_base_currency_pool) = Self::liquidity_pool(target_currency_id);

			// first slippage in swap supply other currency to base currency:
			// first_slippage = supply_amount / (supply_amount + supply_other_currency_pool)
			let supply_to_base_slippage: Ratio =
				Ratio::from_rational(supply_amount, supply_amount.saturating_add(supply_other_currency_pool));

			// second slippage in swap base currency to target other currency:
			// base_amount = first_slippage * supply_base_currency_pool
			// second_slippage = base_amount / (base_amount + target_base_currency_pool)
			let base_to_target_slippage: Ratio = Ratio::from_rational(
				supply_to_base_slippage.saturating_mul_int(&supply_base_currency_pool),
				supply_to_base_slippage
					.saturating_mul_int(&supply_base_currency_pool)
					.saturating_add(target_base_currency_pool),
			);

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
