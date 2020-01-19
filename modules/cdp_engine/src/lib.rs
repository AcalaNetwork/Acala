#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::{arithmetic::Signed, MultiCurrency, MultiCurrencyExtended};
use rstd::{convert::TryInto, marker, prelude::*};
use sp_runtime::{
	traits::{CheckedAdd, CheckedSub, Convert, EnsureOrigin, Saturating, UniqueSaturatedInto, Zero},
	DispatchResult,
};
use support::{
	AuctionManager, CDPTreasury, EmergencyShutdown, ExchangeRate, Price, PriceProvider, Rate, Ratio, RiskManager,
};
use system::ensure_root;

mod debit_exchange_rate_convertor;
pub use debit_exchange_rate_convertor::DebitExchangeRateConvertor;

mod mock;
mod tests;

type CurrencyIdOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as vaults::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait + vaults::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type AuctionManagerHandler: AuctionManager<
		Self::AccountId,
		Balance = BalanceOf<Self>,
		CurrencyId = CurrencyIdOf<Self>,
	>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, Price>;
	type CollateralCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type GlobalStabilityFee: Get<Rate>;
	type DefaultLiquidationRatio: Get<Ratio>;
	type DefaulDebitExchangeRate: Get<ExchangeRate>;
	type MinimumDebitValue: Get<BalanceOf<Self>>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
	type Treasury: CDPTreasury<Self::AccountId, Balance = BalanceOf<Self>, CurrencyId = CurrencyIdOf<Self>>;
	type UpdateOrigin: EnsureOrigin<Self::Origin>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		LiquidateUnsafeCdp(CurrencyId, AccountId, Balance, Balance),
		SettleCdpInDebit(CurrencyId, AccountId),
	}
);

decl_error! {
	/// Error for cdp engine module.
	pub enum Error for Module<T: Trait> {
		ExceedDebitValueHardCap,
		UpdatePositionFailed,
		DebitAmountConvertFailed,
		AmountConvertFailed,
		BelowRequiredCollateralRatio,
		BelowLiquidationRatio,
		CollateralRatioStillSafe,
		NotValidCurrencyId,
		RemainDebitValueTooSmall,
		GrabCollateralAndDebitFailed,
		BalanceOverflow,
		InvalidFeedPrice,
		AlreadyNoDebit,
	}
}

decl_storage! {
	trait Store for Module<T: Trait> as CdpEngine {
		pub StabilityFee get(fn stability_fee): map CurrencyIdOf<T> => Option<Rate>;
		pub LiquidationRatio get(fn liquidation_ratio): map CurrencyIdOf<T> => Option<Ratio>;
		pub LiquidationPenalty get(fn liquidation_penalty): map CurrencyIdOf<T> => Option<Rate>;
		pub RequiredCollateralRatio get(fn required_collateral_ratio): map CurrencyIdOf<T> => Option<Ratio>;
		pub MaximumTotalDebitValue get(fn maximum_total_debit_value): map CurrencyIdOf<T> => BalanceOf<T>;
		pub DebitExchangeRate get(fn debit_exchange_rate): map CurrencyIdOf<T> => Option<ExchangeRate>;
		pub MaximumCollateralAuctionSize get(fn maximum_collateral_auction_size): map CurrencyIdOf<T> => BalanceOf<T>;
		pub IsShutdown get(fn is_shutdown): bool;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn set_maximum_collateral_auction_size(origin, currency_id: CurrencyIdOf<T>, size: BalanceOf<T>) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			<MaximumCollateralAuctionSize<T>>::insert(currency_id, size);
		}

		pub fn set_collateral_params(
			origin,
			currency_id: CurrencyIdOf<T>,
			stability_fee: Option<Option<Rate>>,
			liquidation_ratio: Option<Option<Ratio>>,
			liquidation_penalty: Option<Option<Rate>>,
			required_collateral_ratio: Option<Option<Ratio>>,
			maximum_total_debit_value: Option<BalanceOf<T>>,
		) {
			T::UpdateOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;
			if let Some(update) = stability_fee {
				if let Some(val) = update {
					<StabilityFee<T>>::insert(currency_id, val);
				} else {
					<StabilityFee<T>>::remove(currency_id);
				}
			}
			if let Some(update) = liquidation_ratio {
				if let Some(val) = update {
					<LiquidationRatio<T>>::insert(currency_id, val);
				} else {
					<LiquidationRatio<T>>::remove(currency_id);
				}
			}
			if let Some(update) = liquidation_penalty {
				if let Some(val) = update {
					<LiquidationPenalty<T>>::insert(currency_id, val);
				} else {
					<LiquidationPenalty<T>>::remove(currency_id);
				}
			}
			if let Some(update) = required_collateral_ratio {
				if let Some(val) = update {
					<RequiredCollateralRatio<T>>::insert(currency_id, val);
				} else {
					<RequiredCollateralRatio<T>>::remove(currency_id);
				}
			}
			if let Some(val) = maximum_total_debit_value {
				<MaximumTotalDebitValue<T>>::insert(currency_id, val);
			}
		}

		fn on_finalize(_now: T::BlockNumber) {
			// collect stability fee for all types of collateral
			if !Self::is_shutdown() {
				let global_stability_fee = T::GlobalStabilityFee::get();

				for currency_id in T::CollateralCurrencyIds::get() {
					let debit_exchange_rate = Self::debit_exchange_rate(currency_id).unwrap_or_else(T::DefaulDebitExchangeRate::get);
					let stability_fee_rate = Self::stability_fee(currency_id)
						.unwrap_or_default()
						.saturating_add(global_stability_fee);
					let total_debits = <vaults::Module<T>>::total_debits(currency_id);
					if !stability_fee_rate.is_zero() && !total_debits.is_zero() {
						let debit_exchange_rate_increment = debit_exchange_rate.saturating_mul(stability_fee_rate);

						// update exchange rate
						let new_debit_exchange_rate = debit_exchange_rate.saturating_add(debit_exchange_rate_increment);
						<DebitExchangeRate<T>>::insert(currency_id, new_debit_exchange_rate);

						// issue stablecoin to surplus pool
						let total_debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, total_debits));
						let issued_stable_coin_balance = debit_exchange_rate_increment.saturating_mul_int(&total_debit_value);
						<T as Trait>::Treasury::on_system_surplus(issued_stable_coin_balance);
					}
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}

	pub fn calculate_collateral_ratio(
		currency_id: CurrencyIdOf<T>,
		collateral_balance: BalanceOf<T>,
		debit_balance: T::DebitBalance,
		price: Price,
	) -> Ratio {
		let locked_collateral_value = price.saturating_mul_int(&collateral_balance);
		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		Ratio::from_rational(locked_collateral_value, debit_value)
	}

	pub fn exceed_debit_value_cap(currency_id: CurrencyIdOf<T>, debit_balance: T::DebitBalance) -> bool {
		let hard_cap = Self::maximum_total_debit_value(currency_id);
		let issue = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		issue > hard_cap
	}

	pub fn update_position(
		who: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: T::DebitAmount,
	) -> DispatchResult {
		ensure!(
			T::CollateralCurrencyIds::get().contains(&currency_id),
			Error::<T>::NotValidCurrencyId,
		);
		<vaults::Module<T>>::update_position(who, currency_id, collateral_adjustment, debit_adjustment)
			.map_err(|_| Error::<T>::UpdatePositionFailed)?;

		Ok(())
	}

	// settle cdp has debit when emergency shutdown
	pub fn settle_cdp_has_debit(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <vaults::Module<T>>::debits(&who, currency_id);
		ensure!(!debit_balance.is_zero(), Error::<T>::AlreadyNoDebit);

		// confiscate collateral in cdp to cdp treasury
		// and decrease cdp's debit to zero
		let collateral_balance = <vaults::Module<T>>::collaterals(&who, currency_id);
		let settle_price: Price = T::PriceSource::get_price(currency_id, T::GetStableCurrencyId::get())
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let debt_in_stable_currency = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let confiscate_collateral_amount = rstd::cmp::min(
			settle_price.saturating_mul_int(&debt_in_stable_currency),
			collateral_balance,
		);
		let grab_collateral_amount = TryInto::<AmountOf<T>>::try_into(confiscate_collateral_amount)
			.map_err(|_| Error::<T>::AmountConvertFailed)?;
		let grab_debit_amount =
			TryInto::<T::DebitAmount>::try_into(debit_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		<vaults::Module<T>>::update_collaterals_and_debits(
			who.clone(),
			currency_id,
			-grab_collateral_amount,
			-grab_debit_amount,
		)
		.map_err(|_| Error::<T>::GrabCollateralAndDebitFailed)?;
		<T as Trait>::Treasury::deposit_system_collateral(currency_id, confiscate_collateral_amount)
			.expect("never failed because this amount can not cause overflow");
		<T as Trait>::Treasury::on_system_debit(debt_in_stable_currency);

		Self::deposit_event(RawEvent::SettleCdpInDebit(currency_id, who));
		Ok(())
	}

	// liquidate unsafe cdp
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		let debit_balance = <vaults::Module<T>>::debits(&who, currency_id);
		let collateral_balance = <vaults::Module<T>>::collaterals(&who, currency_id);

		// first: ensure the cdp is unsafe
		let feed_price = T::PriceSource::get_price(T::GetStableCurrencyId::get(), currency_id)
			.ok_or(Error::<T>::InvalidFeedPrice)?;
		let collateral_ratio =
			Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
		let liquidation_ratio = Self::liquidation_ratio(currency_id).unwrap_or_else(T::DefaultLiquidationRatio::get);
		ensure!(
			collateral_ratio < liquidation_ratio,
			Error::<T>::CollateralRatioStillSafe
		);

		// second: grab collaterals and debits from unsafe cdp
		let grab_amount =
			TryInto::<AmountOf<T>>::try_into(collateral_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		let grab_debit_amount =
			TryInto::<T::DebitAmount>::try_into(debit_balance).map_err(|_| Error::<T>::AmountConvertFailed)?;
		<vaults::Module<T>>::update_collaterals_and_debits(who.clone(), currency_id, -grab_amount, -grab_debit_amount)
			.map_err(|_| Error::<T>::GrabCollateralAndDebitFailed)?;

		// third: create collateral auction
		let bad_debt = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let mut target = bad_debt;
		if let Some(penalty_ratio) = Self::liquidation_penalty(currency_id) {
			target = target.saturating_add(penalty_ratio.saturating_mul_int(&target));
		}
		let maximum_collateral_auction_size = Self::maximum_collateral_auction_size(currency_id);
		let mut unhandled_collateral_amount = collateral_balance;
		let mut unhandled_target = target;
		let mut unhandled_bad_debt = bad_debt;

		while !unhandled_collateral_amount.is_zero() {
			let (lot_collateral_amount, lot_target, lot_bad_debt) = if unhandled_collateral_amount
				> maximum_collateral_auction_size
				&& !maximum_collateral_auction_size.is_zero()
			{
				let rate = Rate::from_rational(maximum_collateral_auction_size, collateral_balance);
				(
					maximum_collateral_auction_size,
					rate.saturating_mul_int(&target),
					rate.saturating_mul_int(&bad_debt),
				)
			} else {
				(unhandled_collateral_amount, unhandled_target, unhandled_bad_debt)
			};

			T::AuctionManagerHandler::new_collateral_auction(
				&who,
				currency_id,
				lot_collateral_amount,
				lot_target,
				lot_bad_debt,
			);

			unhandled_collateral_amount -= lot_collateral_amount;
			unhandled_target -= lot_target;
			unhandled_bad_debt -= lot_bad_debt;
		}

		Self::deposit_event(RawEvent::LiquidateUnsafeCdp(
			currency_id,
			who,
			collateral_balance,
			bad_debt,
		));

		Ok(())
	}
}

impl<T: Trait> RiskManager<T::AccountId, CurrencyIdOf<T>, AmountOf<T>, T::DebitAmount> for Module<T> {
	fn check_position_adjustment(
		account_id: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_amount: AmountOf<T>,
		debit_amount: T::DebitAmount,
	) -> DispatchResult {
		let mut debit_balance = <vaults::Module<T>>::debits(account_id, currency_id);
		let mut collateral_balance = <vaults::Module<T>>::collaterals(account_id, currency_id);

		// calculate new debit balance and collateral balance after position adjustment
		let collateral_balance_adjustment =
			TryInto::<BalanceOf<T>>::try_into(collateral_amount.abs()).map_err(|_| Error::<T>::AmountConvertFailed)?;
		if collateral_amount.is_positive() {
			collateral_balance = collateral_balance
				.checked_add(&collateral_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			collateral_balance = collateral_balance
				.checked_sub(&collateral_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}

		let debit_balance_adjustment = TryInto::<T::DebitBalance>::try_into(debit_amount.abs())
			.map_err(|_| Error::<T>::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			debit_balance = debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			debit_balance = debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}

		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		if !debit_value.is_zero() {
			// check the required collateral ratio
			let feed_price = <T as Trait>::PriceSource::get_price(T::GetStableCurrencyId::get(), currency_id)
				.ok_or(Error::<T>::InvalidFeedPrice)?;
			let collateral_ratio =
				Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance, feed_price);
			if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id) {
				ensure!(
					collateral_ratio >= required_collateral_ratio,
					Error::<T>::BelowRequiredCollateralRatio
				);
			}

			// check the liquidation ratio
			let liquidation_ratio = if let Some(ratio) = Self::liquidation_ratio(currency_id) {
				ratio
			} else {
				T::DefaultLiquidationRatio::get()
			};
			ensure!(collateral_ratio >= liquidation_ratio, Error::<T>::BelowLiquidationRatio);

			// check the minimum_debit_value
			ensure!(
				debit_value >= T::MinimumDebitValue::get(),
				Error::<T>::RemainDebitValueTooSmall,
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyIdOf<T>, debit_amount: T::DebitAmount) -> DispatchResult {
		let mut total_debit_balance = <vaults::Module<T>>::total_debits(currency_id);
		let debit_balance_adjustment = TryInto::<T::DebitBalance>::try_into(debit_amount.abs())
			.map_err(|_| Error::<T>::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			total_debit_balance = total_debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		} else {
			total_debit_balance = total_debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::<T>::BalanceOverflow)?;
		}
		ensure!(
			!Self::exceed_debit_value_cap(currency_id, total_debit_balance),
			Error::<T>::ExceedDebitValueHardCap
		);

		Ok(())
	}
}

impl<T: Trait> EmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}
