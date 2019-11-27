#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use orml_traits::{arithmetic::Signed, MultiCurrency, MultiCurrencyExtended, PriceProvider};
use orml_utilities::FixedU128;
use rstd::{convert::TryInto, marker, result};
use sr_primitives::traits::{Bounded, CheckedAdd, CheckedSub, Convert};
use support::{AuctionManager, ExchangeRate, Price, Rate, Ratio, RiskManager};

mod debit_exchange_rate_convertor;
pub use debit_exchange_rate_convertor::DebitExchangeRateConvertor;

mod mock;
mod tests;

type BalanceOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type CurrencyIdOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type DebitBalanceOf<T> =
	<<T as vaults::Trait>::DebitCurrency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as vaults::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;
type DebitAmountOf<T> =
	<<T as vaults::Trait>::DebitCurrency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

pub trait Trait: system::Trait + vaults::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
	type AuctionManagerHandler: AuctionManager<
		Self::AccountId,
		CurrencyId = CurrencyIdOf<Self>,
		Balance = BalanceOf<Self>,
		Amount = AmountOf<Self>,
	>;
	type Currency: MultiCurrencyExtended<Self::AccountId>;
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, FixedU128>;
	type CollateralCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type GlobalStabilityFee: Get<Rate>;
	type DefaultLiquidationRatio: Get<Ratio>;
	type DefaulDebitExchangeRate: Get<ExchangeRate>;
	type MinimumDebitValue: Get<BalanceOf<Self>>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
}

decl_event!(
	pub enum Event<T>
	where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		LiquidateUnsafeCdp(CurrencyId, AccountId, Balance, Balance),
	}
);

decl_error! {
	/// Error for cdp engine module.
	pub enum Error {
		ExceedDebitValueHardCap,
		DebitAmountConvertFailed,
		AmountConvertFailed,
		BelowRequiredCollateralRatio,
		BelowLiquidationRatio,
		CollateralRatioStillSafe,
		UpdatePositionFailed,
		NotValidCurrencyId,
		RemainDebitValueTooSmall,
		GrabCollateralAndDebitFailed,
		BalanceOverflow,
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
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		fn on_finalize(_now: T::BlockNumber) {
			let global_stability_fee = T::GlobalStabilityFee::get();
			// handle all kinds of collateral type
			for currency_id in T::CollateralCurrencyIds::get() {
				let debit_exchange_rate = Self::debit_exchange_rate(currency_id).unwrap_or(T::DefaulDebitExchangeRate::get());
				let stability_fee_rate = Self::stability_fee(currency_id).unwrap_or(Rate::from_parts(0)).checked_add(&global_stability_fee).unwrap_or(Rate::max_value());
				let debit_exchange_rate_increment = debit_exchange_rate.checked_mul(&stability_fee_rate).unwrap_or(ExchangeRate::max_value());
				if debit_exchange_rate_increment > ExchangeRate::from_parts(0) {
					// update exchange rate
					let new_debit_exchange_rate = debit_exchange_rate.checked_add(&debit_exchange_rate_increment).unwrap_or(ExchangeRate::max_value());
					<DebitExchangeRate<T>>::insert(currency_id, new_debit_exchange_rate);

					// issue stablecoin to surplus pool
					let total_debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, <vaults::Module<T>>::total_debits(currency_id)));
					let issued_stable_coin_balance = debit_exchange_rate_increment.checked_mul_int(&total_debit_value).unwrap_or(BalanceOf::<T>::max_value());
					T::AuctionManagerHandler::increase_surplus(issued_stable_coin_balance);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn set_collateral_params(
		currency_id: CurrencyIdOf<T>,
		stability_fee: Option<Option<Rate>>,
		liquidation_ratio: Option<Option<Ratio>>,
		liquidation_penalty: Option<Option<Rate>>,
		required_collateral_ratio: Option<Option<Ratio>>,
		maximum_total_debit_value: Option<BalanceOf<T>>,
	) {
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

	pub fn calculate_collateral_ratio(
		currency_id: CurrencyIdOf<T>,
		collateral_balance: BalanceOf<T>,
		debit_balance: DebitBalanceOf<T>,
	) -> Ratio {
		let price = <T as Trait>::PriceSource::get_price(T::GetStableCurrencyId::get(), currency_id)
			.unwrap_or(Price::from_parts(0));
		let locked_collateral_value = TryInto::<u128>::try_into(
			price
				.checked_mul_int(&collateral_balance)
				.unwrap_or(BalanceOf::<T>::max_value()),
		)
		.unwrap_or(u128::max_value());
		let debit_value =
			TryInto::<u128>::try_into(DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance)))
				.unwrap_or(u128::max_value());

		Ratio::from_rational(locked_collateral_value, debit_value)
	}

	pub fn exceed_debit_value_cap(currency_id: CurrencyIdOf<T>, debit_balance: DebitBalanceOf<T>) -> bool {
		let hard_cap = Self::maximum_total_debit_value(currency_id);
		let issue = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		issue > hard_cap
	}

	pub fn update_position(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		ensure!(
			T::CollateralCurrencyIds::get().contains(&currency_id),
			Error::NotValidCurrencyId,
		);
		<vaults::Module<T>>::update_position(who, currency_id, collateral_adjustment, debit_adjustment)
			.map_err(|_| Error::UpdatePositionFailed)?;

		Ok(())
	}

	// TODO: how to trigger cdp liquidation
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> result::Result<(), Error> {
		let debit_balance = <vaults::Module<T>>::debits(&who, currency_id);
		let collateral_balance: BalanceOf<T> = <vaults::Module<T>>::collaterals(&who, currency_id);

		// ensure the cdp is unsafe
		let collateral_ratio = Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance);
		let liquidation_ratio = if let Some(ratio) = Self::liquidation_ratio(currency_id) {
			ratio
		} else {
			T::DefaultLiquidationRatio::get()
		};
		ensure!(collateral_ratio < liquidation_ratio, Error::CollateralRatioStillSafe);

		// grab collaterals and debits from unsafe cdp
		let grab_amount =
			TryInto::<AmountOf<T>>::try_into(collateral_balance).map_err(|_| Error::AmountConvertFailed)?;
		let grab_debit_amount =
			TryInto::<DebitAmountOf<T>>::try_into(debit_balance).map_err(|_| Error::AmountConvertFailed)?;
		<vaults::Module<T>>::update_collaterals_and_debits(who.clone(), currency_id, -grab_amount, -grab_debit_amount)
			.map_err(|_| Error::GrabCollateralAndDebitFailed)?;

		// create collateral auction
		let bad_debt = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let mut target = bad_debt;
		if let Some(penalty_ratio) = Self::liquidation_penalty(currency_id) {
			target = target
				.checked_add(
					&penalty_ratio
						.checked_mul_int(&target)
						.unwrap_or(BalanceOf::<T>::max_value()),
				)
				.unwrap_or(BalanceOf::<T>::max_value());
		}
		T::AuctionManagerHandler::new_collateral_auction(
			who.clone(),
			currency_id,
			collateral_balance,
			target,
			bad_debt,
		);
		Self::deposit_event(RawEvent::LiquidateUnsafeCdp(
			currency_id,
			who,
			collateral_balance,
			bad_debt,
		));

		Ok(())
	}
}

impl<T: Trait> RiskManager<T::AccountId, CurrencyIdOf<T>, AmountOf<T>, DebitAmountOf<T>> for Module<T> {
	type Error = Error;

	fn check_position_adjustment(
		account_id: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_amount: AmountOf<T>,
		debit_amount: DebitAmountOf<T>,
	) -> Result<(), Self::Error> {
		let mut debit_balance = <vaults::Module<T>>::debits(account_id, currency_id);
		let mut collateral_balance = <vaults::Module<T>>::collaterals(account_id, currency_id);

		// calculate new debit balance and collateral balance after position adjustment
		let collateral_balance_adjustment =
			TryInto::<BalanceOf<T>>::try_into(collateral_amount.abs()).map_err(|_| Error::AmountConvertFailed)?;
		if collateral_amount.is_positive() {
			collateral_balance = collateral_balance
				.checked_add(&collateral_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		} else {
			collateral_balance = collateral_balance
				.checked_sub(&collateral_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		}

		let debit_balance_adjustment =
			TryInto::<DebitBalanceOf<T>>::try_into(debit_amount.abs()).map_err(|_| Error::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			debit_balance = debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		} else {
			debit_balance = debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		}

		let debit_value = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));

		if debit_value != 0.into() {
			// check the required collateral ratio
			let collateral_ratio = Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance);
			if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id) {
				ensure!(
					collateral_ratio >= required_collateral_ratio,
					Error::BelowRequiredCollateralRatio
				);
			}

			// check the liquidation ratio
			let liquidation_ratio = if let Some(ratio) = Self::liquidation_ratio(currency_id) {
				ratio
			} else {
				T::DefaultLiquidationRatio::get()
			};
			ensure!(collateral_ratio >= liquidation_ratio, Error::BelowLiquidationRatio);

			// check the minimum_debit_value
			ensure!(
				debit_value >= T::MinimumDebitValue::get(),
				Error::RemainDebitValueTooSmall,
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyIdOf<T>, debit_amount: DebitAmountOf<T>) -> Result<(), Self::Error> {
		let mut total_debit_balance = <vaults::Module<T>>::total_debits(currency_id);
		let debit_balance_adjustment =
			TryInto::<DebitBalanceOf<T>>::try_into(debit_amount.abs()).map_err(|_| Error::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			total_debit_balance = total_debit_balance
				.checked_add(&debit_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		} else {
			total_debit_balance = total_debit_balance
				.checked_sub(&debit_balance_adjustment)
				.ok_or(Error::BalanceOverflow)?;
		}
		ensure!(
			!Self::exceed_debit_value_cap(currency_id, total_debit_balance),
			Error::ExceedDebitValueHardCap
		);

		Ok(())
	}
}
