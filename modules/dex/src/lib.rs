#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, Parameter};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::FixedU128;
use primitives::U256;
use rstd::{convert::TryInto, result};
use sp_runtime::{
	traits::{
		AccountIdConversion, Bounded, CheckedAdd, CheckedSub, MaybeSerializeDeserialize, Member, SimpleArithmetic,
	},
	ModuleId,
};
use support::DexManager;
use system::{self as system, ensure_signed};

mod mock;
mod tests;

const MODULE_ID: ModuleId = ModuleId(*b"aca/dexm");

type BalanceOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;

pub trait Trait: system::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type Share: Parameter + Member + SimpleArithmetic + Default + Copy + MaybeSerializeDeserialize;
	type GetBaseCurrencyId: Get<CurrencyIdOf<Self>>;
	type GetExchangeFee: Get<FixedU128>;
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		<T as Trait>::Share,
		Balance = BalanceOf<T>,
		CurrencyId = CurrencyIdOf<T>,
	{
		InjectLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		ExtractLiquidity(AccountId, CurrencyId, Balance, Balance, Share),
		Swap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
		OtherToBaseSwap(AccountId, CurrencyId, Balance, Balance),
		BaseToOtherSwap(AccountId, CurrencyId, Balance, Balance),
		OtherToOtherSwap(AccountId, CurrencyId, Balance, CurrencyId, Balance),
	}
);

decl_error! {
	/// Error for cdp dex module.
	pub enum Error {
		BaseCurrencyIdNotAllowed,
		TokenNotEnough,
		ShareNotEnough,
		InvalidBalance,
		CanNotSwapItself,
		CanNotSwapAny,
		InvalidInject,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as Dex {
		LiquidityPool get(fn liquidity_pool): double_map CurrencyIdOf<T>, blake2_256(CurrencyIdOf<T>) => (BalanceOf<T>, BalanceOf<T>);
		TotalShares get(fn total_shares): map CurrencyIdOf<T> => T::Share;
		Shares get(fn shares): double_map T::AccountId, blake2_256(CurrencyIdOf<T>) => T::Share;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn swap_tokens(origin, supply: (CurrencyIdOf<T>, BalanceOf<T>), target: (CurrencyIdOf<T>, BalanceOf<T>)) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();

			// check swap pairs is valid
			ensure!(
				target.0 != supply.0,
				Error::CanNotSwapItself.into(),
			);

			if target.0 == base_currency_id {
				// use other token to swap base token
				Self::swap_other_to_base(who, supply.0, supply.1, target.1)?;
			} else if supply.0 == base_currency_id {
				// use base token to swap other token
				Self::swap_base_to_other(who, target.0, supply.1, target.1)?;
			} else {
				// other swap token to other token
				Self::swap_other_to_other(who, supply.0, target.0, supply.1, target.1)?;
			}
		}

		fn inject_liquidity(origin, tokens: (CurrencyIdOf<T>, BalanceOf<T>), base_currency_amount: BalanceOf<T>) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();

			ensure!(
				tokens.0 != base_currency_id,
				Error::BaseCurrencyIdNotAllowed.into(),
			);

			// check balance
			ensure!(
				tokens.1 != 0.into() && base_currency_amount != 0.into(),
				Error::InvalidBalance.into(),
			);
			ensure!(
				T::Currency::balance(base_currency_id, &who) >= base_currency_amount
				&&
				T::Currency::balance(tokens.0, &who) >= tokens.1,
				Error::TokenNotEnough.into(),
			);

			let total_shares = Self::total_shares(tokens.0);
			let (inject_token, inject_base, share_increment): (BalanceOf<T>, BalanceOf<T>, T::Share) =
			if total_shares == 0.into() {
				// initialize this liquidity pool, the initial share is equal to the min value between base currency amount and tokens amount
				let initial_share = TryInto::<T::Share>::try_into(
					TryInto::<u128>::try_into(
						rstd::cmp::max(tokens.1, base_currency_amount)
					).unwrap_or(u128::max_value())
				).unwrap_or(T::Share::max_value());

				(tokens.1, base_currency_amount, initial_share)
			} else {
				let (token_pool, base_pool): (BalanceOf<T>, BalanceOf<T>) = Self::liquidity_pool(tokens.0, base_currency_id);

				let token_to_base_rate = FixedU128::from_rational(
					TryInto::<u128>::try_into(base_pool).unwrap_or(u128::max_value()),
					TryInto::<u128>::try_into(token_pool).unwrap_or(u128::max_value()),
				);

				let input_rate = FixedU128::from_rational(
					TryInto::<u128>::try_into(base_currency_amount).unwrap_or(u128::max_value()),
					TryInto::<u128>::try_into(tokens.1).unwrap_or(u128::max_value()),
				);

				if input_rate <= token_to_base_rate {
					// input token amount is enough
					let base_to_token_rate = FixedU128::from_rational(
						TryInto::<u128>::try_into(token_pool).unwrap_or(u128::max_value()),
						TryInto::<u128>::try_into(base_pool).unwrap_or(u128::max_value()),
					);
					let token_amount = base_to_token_rate.checked_mul_int(&base_currency_amount).unwrap_or(BalanceOf::<T>::max_value());
					let share = FixedU128::from_rational(
						TryInto::<u128>::try_into(token_amount).unwrap_or(u128::max_value()),
						TryInto::<u128>::try_into(token_pool).unwrap_or(u128::max_value()),
					).checked_mul_int(&total_shares).unwrap_or(0.into());
					(token_amount, base_currency_amount, share)
				} else {
					//input base amount is enough
					let base_amount = token_to_base_rate.checked_mul_int(&tokens.1).unwrap_or(BalanceOf::<T>::max_value());
					let share = FixedU128::from_rational(
						TryInto::<u128>::try_into(base_amount).unwrap_or(u128::max_value()),
						TryInto::<u128>::try_into(base_pool).unwrap_or(u128::max_value()),
					).checked_mul_int(&total_shares).unwrap_or(0.into());
					(tokens.1, base_amount, share)
				}
			};

			ensure!(
				share_increment > 0.into() && inject_token > 0.into() && inject_base > 0.into(),
				Error::InvalidInject.into(),
			);

			T::Currency::transfer(tokens.0, &who, &Self::account_id(), inject_token)
			.expect("never failed because after checks");
			T::Currency::transfer(base_currency_id, &who, &Self::account_id(), inject_base)
			.expect("never failed because after checks");
			<TotalShares<T>>::mutate(tokens.0, |share| *share += share_increment);
			<Shares<T>>::mutate(&who, tokens.0, |share| *share += share_increment);
			<LiquidityPool<T>>::mutate(tokens.0, base_currency_id, |pool| {
				let newpool = (pool.0 + inject_token, pool.1 + inject_base);
				*pool = newpool;
			});
			Self::deposit_event(RawEvent::InjectLiquidity(
				who,
				tokens.0,
				inject_token,
				inject_base,
				share_increment,
			));
		}

		fn extract_liquidity(origin, currency_id: CurrencyIdOf<T>, extract_amount: T::Share) {
			let who = ensure_signed(origin)?;
			let base_currency_id = T::GetBaseCurrencyId::get();

			ensure!(
				currency_id != base_currency_id,
				Error::BaseCurrencyIdNotAllowed.into(),
			);
			ensure!(
				Self::shares(&who, currency_id) >= extract_amount,
				Error::ShareNotEnough.into(),
			);
			let (token_pool, base_pool): (BalanceOf<T>, BalanceOf<T>) = Self::liquidity_pool(currency_id, base_currency_id);
			let proportion = FixedU128::from_rational(
				TryInto::<u128>::try_into(extract_amount).unwrap_or(u128::max_value()),
				TryInto::<u128>::try_into(Self::total_shares(currency_id)).unwrap_or(u128::max_value()),
			);
			let extract_token_amount = proportion.checked_mul_int(&token_pool).unwrap_or(BalanceOf::<T>::max_value());
			let extract_base_amount = proportion.checked_mul_int(&base_pool).unwrap_or(BalanceOf::<T>::max_value());
			if extract_token_amount > 0.into() {
				T::Currency::transfer(currency_id, &Self::account_id(), &who, extract_token_amount)
				.expect("never failed because after checks");
			}
			if extract_base_amount > 0.into() {
				T::Currency::transfer(base_currency_id, &Self::account_id(), &who, extract_base_amount)
				.expect("never failed because after checks");
			}
			<TotalShares<T>>::mutate(currency_id, |share| *share -= extract_amount);
			<Shares<T>>::mutate(&who, currency_id, |share| *share -= extract_amount);
			<LiquidityPool<T>>::mutate(currency_id, base_currency_id, |pool| {
				let newpool = (pool.0 - extract_token_amount, pool.1 - extract_base_amount);
				*pool = newpool;
			});

			Self::deposit_event(RawEvent::ExtractLiquidity(
				who,
				currency_id,
				extract_token_amount,
				extract_base_amount,
				extract_amount,
			));
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
		let new_target_pool: BalanceOf<T> =
			U256::from(TryInto::<u128>::try_into(supply_pool).unwrap_or(u128::max_value()))
				.checked_mul(U256::from(
					TryInto::<u128>::try_into(target_pool).unwrap_or(u128::max_value()),
				))
				.and_then(|n| {
					n.checked_div(U256::from(
						TryInto::<u128>::try_into(supply_pool.checked_add(&supply_amount).unwrap_or(0.into()))
							.unwrap_or(u128::max_value()),
					))
				})
				.and_then(|n| TryInto::<u128>::try_into(n).ok())
				.and_then(|n| TryInto::<BalanceOf<T>>::try_into(n).ok())
				.unwrap_or(0.into());

		if new_target_pool != 0.into() {
			target_pool
				.checked_sub(&new_target_pool)
				.and_then(|n| {
					n.checked_sub(
						&T::GetExchangeFee::get()
							.checked_mul_int(&n)
							.unwrap_or(BalanceOf::<T>::max_value()),
					)
				})
				.unwrap_or(0.into())
		} else {
			0.into()
		}
	}

	pub fn calculate_swap_supply_amount(
		supply_pool: BalanceOf<T>,
		target_pool: BalanceOf<T>,
		target_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		U256::from(TryInto::<u128>::try_into(supply_pool).unwrap_or(u128::max_value()))
			.checked_mul(U256::from(
				TryInto::<u128>::try_into(target_pool).unwrap_or(u128::max_value()),
			))
			.and_then(|n| {
				n.checked_div(U256::from(
					TryInto::<u128>::try_into(
						FixedU128::from_natural(1)
							.checked_sub(&T::GetExchangeFee::get())
							.and_then(|n| FixedU128::from_natural(1).checked_div(&n))
							.and_then(|n| n.checked_mul_int(&target_amount))
							.and_then(|n| target_pool.checked_sub(&n))
							.unwrap_or(0.into()),
					)
					.unwrap_or(u128::max_value()),
				))
			})
			.and_then(|n| TryInto::<u128>::try_into(n).ok())
			.and_then(|n| TryInto::<BalanceOf<T>>::try_into(n).ok())
			.and_then(|n| n.checked_sub(&supply_pool))
			.unwrap_or(0.into())
	}

	pub fn swap_other_to_base(
		who: T::AccountId,
		other_currency_id: CurrencyIdOf<T>,
		other_amount: BalanceOf<T>,
		min_base_amount: BalanceOf<T>,
	) -> result::Result<(), Error> {
		ensure!(
			T::Currency::balance(other_currency_id, &who) >= other_amount,
			Error::TokenNotEnough,
		);
		let base_currency_id = T::GetBaseCurrencyId::get();
		let (token_pool, base_pool) = Self::liquidity_pool(other_currency_id, base_currency_id);
		let base_amount = Self::calculate_swap_target_amount(token_pool, base_pool, other_amount);
		ensure!(
			base_amount != 0.into() && base_amount >= min_base_amount,
			Error::CanNotSwapAny,
		);

		T::Currency::transfer(other_currency_id, &who, &Self::account_id(), other_amount)
			.expect("never failed because after checks");
		T::Currency::transfer(base_currency_id, &Self::account_id(), &who, base_amount)
			.expect("never failed because after checks");
		<LiquidityPool<T>>::mutate(other_currency_id, base_currency_id, |pool| {
			let newpool = (pool.0 + other_amount, pool.1 - base_amount);
			*pool = newpool;
		});
		Self::deposit_event(RawEvent::OtherToBaseSwap(
			who,
			other_currency_id,
			other_amount,
			base_amount,
		));
		Ok(())
	}

	pub fn swap_base_to_other(
		who: T::AccountId,
		other_currency_id: CurrencyIdOf<T>,
		base_amount: BalanceOf<T>,
		min_other_amount: BalanceOf<T>,
	) -> result::Result<(), Error> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		ensure!(
			T::Currency::balance(base_currency_id, &who) >= base_amount,
			Error::TokenNotEnough,
		);
		let (token_pool, base_pool) = Self::liquidity_pool(other_currency_id, base_currency_id);
		let other_amount = Self::calculate_swap_target_amount(base_pool, token_pool, base_amount);
		ensure!(
			other_amount != 0.into() && other_amount >= min_other_amount,
			Error::CanNotSwapAny,
		);

		T::Currency::transfer(base_currency_id, &who, &Self::account_id(), base_amount)
			.expect("never failed because after checks");
		T::Currency::transfer(other_currency_id, &Self::account_id(), &who, other_amount)
			.expect("never failed because after checks");
		<LiquidityPool<T>>::mutate(other_currency_id, base_currency_id, |pool| {
			let newpool = (pool.0 - other_amount, pool.1 + base_amount);
			*pool = newpool;
		});
		Self::deposit_event(RawEvent::BaseToOtherSwap(
			who,
			other_currency_id,
			base_amount,
			other_amount,
		));
		Ok(())
	}

	pub fn swap_other_to_other(
		who: T::AccountId,
		supply_other_currency_id: CurrencyIdOf<T>,
		target_other_currency_id: CurrencyIdOf<T>,
		supply_other_amount: BalanceOf<T>,
		min_target_other_amount: BalanceOf<T>,
	) -> result::Result<(), Error> {
		ensure!(
			T::Currency::balance(supply_other_currency_id, &who) >= supply_other_amount,
			Error::TokenNotEnough,
		);
		let base_currency_id = T::GetBaseCurrencyId::get();
		let (supply_token_pool, supply_base_pool) = Self::liquidity_pool(supply_other_currency_id, base_currency_id);
		let intermediate_base_amount =
			Self::calculate_swap_target_amount(supply_token_pool, supply_base_pool, supply_other_amount);
		let (target_token_pool, target_base_pool) = Self::liquidity_pool(target_other_currency_id, base_currency_id);
		let target_token_amount =
			Self::calculate_swap_target_amount(target_base_pool, target_token_pool, intermediate_base_amount);
		ensure!(
			target_token_amount != 0.into() && target_token_amount >= min_target_other_amount,
			Error::CanNotSwapAny,
		);

		T::Currency::transfer(supply_other_currency_id, &who, &Self::account_id(), supply_other_amount)
			.expect("never failed because after checks");
		T::Currency::transfer(target_other_currency_id, &Self::account_id(), &who, target_token_amount)
			.expect("never failed because after checks");

		<LiquidityPool<T>>::mutate(supply_other_currency_id, base_currency_id, |pool| {
			let newpool = (pool.0 + supply_other_amount, pool.1 - intermediate_base_amount);
			*pool = newpool;
		});
		<LiquidityPool<T>>::mutate(target_other_currency_id, base_currency_id, |pool| {
			let newpool = (pool.0 - target_token_amount, pool.1 + intermediate_base_amount);
			*pool = newpool;
		});
		Self::deposit_event(RawEvent::OtherToOtherSwap(
			who,
			supply_other_currency_id,
			supply_other_amount,
			target_other_currency_id,
			target_token_amount,
		));
		Ok(())
	}
}

impl<T: Trait> DexManager<T::AccountId, CurrencyIdOf<T>, BalanceOf<T>> for Module<T> {
	type Error = Error;

	fn get_supply_amount(
		supply_currency_id: CurrencyIdOf<T>,
		target_currency_id: CurrencyIdOf<T>,
		target_amount: BalanceOf<T>,
	) -> BalanceOf<T> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		if supply_currency_id == target_currency_id {
			0.into()
		} else if target_currency_id == base_currency_id {
			let (token_pool, base_pool) = Self::liquidity_pool(supply_currency_id, base_currency_id);
			Self::calculate_swap_supply_amount(token_pool, base_pool, target_amount)
		} else if supply_currency_id == base_currency_id {
			let (token_pool, base_pool) = Self::liquidity_pool(target_currency_id, base_currency_id);
			Self::calculate_swap_supply_amount(base_pool, token_pool, target_amount)
		} else {
			let (target_token_pool, target_base_pool) = Self::liquidity_pool(target_currency_id, base_currency_id);
			let intermediate_base_amount =
				Self::calculate_swap_supply_amount(target_base_pool, target_token_pool, target_amount);
			let (supply_token_pool, supply_base_pool) = Self::liquidity_pool(supply_currency_id, base_currency_id);
			Self::calculate_swap_supply_amount(supply_token_pool, supply_base_pool, intermediate_base_amount)
		}
	}

	fn exchange_token(
		who: T::AccountId,
		supply: (CurrencyIdOf<T>, BalanceOf<T>),
		target: (CurrencyIdOf<T>, BalanceOf<T>),
	) -> Result<(), Self::Error> {
		let base_currency_id = T::GetBaseCurrencyId::get();
		ensure!(target.0 != supply.0, Error::CanNotSwapItself.into(),);
		if target.0 == base_currency_id {
			Self::swap_other_to_base(who, supply.0, supply.1, target.1)
		} else if supply.0 == base_currency_id {
			Self::swap_base_to_other(who, target.0, supply.1, target.1)
		} else {
			Self::swap_other_to_other(who, supply.0, target.0, supply.1, target.1)
		}
	}
}
