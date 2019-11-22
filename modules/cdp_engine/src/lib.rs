#![cfg_attr(not(feature = "std"), no_std)]

use palette_support::{decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get};
use rstd::{ convert::{TryFrom, TryInto}, marker, result};
use sr_primitives::{traits::{Convert, SaturatedConversion}, Fixed64, Permill, RuntimeDebug};
use support::{ExchangeRate, Price, Ratio, RiskManager, AuctionManager};
use orml_traits::{arithmetic::Signed, MultiCurrency, MultiCurrencyExtended, PriceProvider};

mod debit_exchange_rate_convertor;
pub use debit_exchange_rate_convertor::DebitExchangeRateConvertor;

pub const U128_BILLION: u128 = 1_000_000_000;
pub const U128_MILLION: u128 = 1_000_000;

pub type BalanceOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
pub type CurrencyIdOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
pub type DebitBalanceOf<T> =
	<<T as vaults::Trait>::DebitCurrency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
pub type AmountOf<T> =
	<<T as vaults::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;
pub type DebitAmountOf<T> =
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
	type PriceSource: PriceProvider<CurrencyIdOf<Self>, Fixed64>;
	type CollateralCurrencyIds: Get<Vec<CurrencyIdOf<Self>>>;
	type GlobalStabilityFee: Get<Permill>;
	type DefaultLiquidationRatio: Get<Ratio>;
	type DefaulDebitExchangeRate: Get<ExchangeRate>;
	type MinimumDebitValue: Get<BalanceOf<Self>>;
	type GetNativeCurrencyId: Get<CurrencyIdOf<Self>>;
	type GetStableCurrencyId: Get<CurrencyIdOf<Self>>;
}

decl_event!(
	pub enum Event<T>
	where
		CurrencyId = CurrencyIdOf<T>,
		Balance = BalanceOf<T>,
	{
		CollateralAuction(CurrencyId, Balance, Balance),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as CdpEngine {
		pub StabilityFee get(fn stability_fee): map CurrencyIdOf<T> => Option<Permill>;
		pub LiquidationRatio get(fn liquidation_ratio): map CurrencyIdOf<T> => Option<Ratio>;
		pub LiquidationPenalty get(fn liquidation_penalty): map CurrencyIdOf<T> => Option<Permill>;
		pub RequiredCollateralRatio get(fn required_collateral_ratio): map CurrencyIdOf<T> => Option<Ratio>;
		pub MaximumTotalDebitValue get(fn maximum_total_debit_value): map CurrencyIdOf<T> => BalanceOf<T>;
		pub DebitExchangeRate get(fn debit_exchange_rate): map CurrencyIdOf<T> => Option<ExchangeRate>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		// const StableCurrencyId: CurrencyIdOf<T> = T::GetStableCurrencyId::get();
		// const CollateralCurrencyIds: Vec<CurrencyIdOf<T>> = T::CollateralCurrencyIds::get();
		// const GlobalStabilityFee: Permill = T::GlobalStabilityFee::get();

		// TODO: drip stability fee
		fn on_finalize(now: T::BlockNumber) {
			let global_stability_fee = T::GlobalStabilityFee::get();
			for currency_id in T::CollateralCurrencyIds::get() {
				let debit_exchange_rate: u128 = TryInto::<u128>::try_into(Self::debit_exchange_rate(currency_id).unwrap_or(Fixed64::from_natural(1)).into_inner()).unwrap_or(0);
				let stability_fee: u128 = u128::from(Self::stability_fee(currency_id).unwrap_or(Permill::zero()).deconstruct()) + u128::from(global_stability_fee.deconstruct());

				// update exchange rate
				let debit_exchange_rate_increment = debit_exchange_rate * stability_fee / U128_MILLION;
				if debit_exchange_rate_increment > 0 {
					let new_debit_exchange_rate = Fixed64::from_parts(TryInto::<i64>::try_into(debit_exchange_rate + debit_exchange_rate_increment).unwrap_or(0));
					<DebitExchangeRate<T>>::insert(currency_id, new_debit_exchange_rate)
				}

				// stablecoin inflation
				let total_debit_balance = <vaults::Module<T>>::total_debits(currency_id);
				let inflation_balance = TryInto::<BalanceOf<T>>::try_into(DebitExchangeRateConvertor::<T>::convert((currency_id, total_debit_balance)).saturated_into::<u128>() * debit_exchange_rate_increment / U128_BILLION).unwrap_or(0.into());

				if inflation_balance > 0.into() {
					T::AuctionManagerHandler::increase_surplus(inflation_balance);
				}
			}
		}
	}
}

impl<T: Trait> Module<T> {
	pub fn calculate_collateral_ratio(
		currency_id: CurrencyIdOf<T>,
		collateral_balance: BalanceOf<T>,
		debit_balance: DebitBalanceOf<T>,
	) -> Ratio {
		let price =
			<T as Trait>::PriceSource::get_price(T::GetNativeCurrencyId::get(), currency_id).unwrap_or(Fixed64::from_parts(0));
		let exchange_rate = Self::debit_exchange_rate(currency_id).unwrap_or(Fixed64::from_parts(0));

		let locked_collateral_value: i64 = TryInto::<i64>::try_into(
			collateral_balance.saturated_into::<u128>()
				* TryInto::<u128>::try_into(price.into_inner()).unwrap_or(U128_BILLION)
				/ TryInto::<u128>::try_into(Fixed64::accuracy()).unwrap_or(U128_BILLION),
		)
		.unwrap_or(0);

		let debit_value: u64 = TryInto::<u64>::try_into(
			debit_balance.saturated_into::<u128>()
				* TryInto::<u128>::try_into(exchange_rate.into_inner()).unwrap_or(U128_BILLION)
				/ TryInto::<u128>::try_into(Fixed64::accuracy()).unwrap_or(U128_BILLION),
		)
		.unwrap_or(0);

		Fixed64::from_rational(locked_collateral_value, debit_value)
	}

	pub fn exceed_debit_value_cap(currency_id: CurrencyIdOf<T>, debit_balance: DebitBalanceOf<T>) -> bool {
		let hard_cap = Self::maximum_total_debit_value(currency_id);
		let issue = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		issue > hard_cap
	}

	// // TODO: params setter
	// fn update_collateral_params(currency_id: CurrencyIdOf<T>) -> result::Result<(), Error> {
	// 	Ok(())
	// }

	pub fn update_position(
		who: T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_adjustment: AmountOf<T>,
		debit_adjustment: DebitAmountOf<T>,
	) -> result::Result<(), Error> {
		<vaults::Module<T>>::update_position(who, currency_id, collateral_adjustment, debit_adjustment)
			.map_err(|_| Error::AmountConvertFailed)?;

		Ok(())
	}

	// TODO: how to trigger cdp liquidation
	pub fn liquidate_unsafe_cdp(who: T::AccountId, currency_id: CurrencyIdOf<T>) -> result::Result<(), Error> {
		let debit_balance = <vaults::Module<T>>::debits(&who, currency_id);
		let collateral_balance: BalanceOf<T> = <vaults::Module<T>>::collaterals(&who, currency_id);

		// judge the cdp is safe or not
		let collateral_ratio = Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance);
		if let Some(liquidation_ratio) = Self::liquidation_ratio(currency_id) {
			ensure!(collateral_ratio < liquidation_ratio, Error::CdpStillSafe);
		} else {
			ensure!(
				collateral_ratio < T::DefaultLiquidationRatio::get(),
				Error::CdpStillSafe
			);
		}

		// grab collaterals and debits from unsafe cdp
		let amount = TryInto::<AmountOf<T>>::try_into(collateral_balance).map_err(|_| Error::AmountConvertFailed)?;
		let debit_amount =
			TryInto::<DebitAmountOf<T>>::try_into(debit_balance).map_err(|_| Error::AmountConvertFailed)?;
		<vaults::Module<T>>::update_collaterals_and_debits(who.clone(), currency_id, -amount, -debit_amount)
			.map_err(|_| Error::AmountConvertFailed)?;
		// create collateral auction
		let bad_debt = DebitExchangeRateConvertor::<T>::convert((currency_id, debit_balance));
		let mut target = bad_debt;
		if let Some(penalty_ratio) = Self::liquidation_penalty(currency_id) {
			target += penalty_ratio * target;
		}
		T::AuctionManagerHandler::new_collateral_auction(who, currency_id, collateral_balance, target, bad_debt);

		Ok(())
	}
}

decl_error! {
	/// Error for cdp engine module.
	pub enum Error {
		CollateralRatioTooLow,
		ExceedDebitValueHardCap,
		DebitAmountConvertFailed,
		AmountConvertFailed,
		BelowRequiredCollateralRatio,
		BelowLiquidationRatio,
		CdpStillSafe,
	}
}

impl<T: Trait> RiskManager<T::AccountId, CurrencyIdOf<T>, AmountOf<T>, DebitAmountOf<T>> for Module<T> {
	type Error = Error;

	fn required_collateral_ratio(currency_id: CurrencyIdOf<T>) -> Fixed64 {
		<RequiredCollateralRatio<T>>::get(currency_id).unwrap_or(Fixed64::from_parts(0))
	}

	fn check_position_adjustment(
		account_id: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
		collateral_amount: AmountOf<T>,
		debit_amount: DebitAmountOf<T>,
	) -> Result<(), Self::Error> {
		let mut debit_balance = <vaults::Module<T>>::debits(account_id, currency_id);
		let mut collateral_balance = <vaults::Module<T>>::collaterals(account_id, currency_id);

		let collateral_balance_adjustment =
			TryInto::<BalanceOf<T>>::try_into(collateral_amount.abs()).map_err(|_| Error::AmountConvertFailed)?;
		if collateral_amount.is_positive() {
			collateral_balance += collateral_balance_adjustment;
		} else {
			collateral_balance -= collateral_balance_adjustment;
		}

		let debit_balance_adjustment =
			TryInto::<DebitBalanceOf<T>>::try_into(debit_amount.abs()).map_err(|_| Error::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			debit_balance += debit_balance_adjustment;
		} else {
			debit_balance -= debit_balance_adjustment;
		}

		let collateral_ratio = Self::calculate_collateral_ratio(currency_id, collateral_balance, debit_balance);
		if let Some(required_collateral_ratio) = Self::required_collateral_ratio(currency_id) {
			ensure!(
				collateral_ratio >= required_collateral_ratio,
				Error::BelowRequiredCollateralRatio
			);
		}
		if let Some(liquidation_ratio) = Self::liquidation_ratio(currency_id) {
			ensure!(collateral_ratio >= liquidation_ratio, Error::BelowLiquidationRatio);
		} else {
			ensure!(
				collateral_ratio >= T::DefaultLiquidationRatio::get(),
				Error::BelowLiquidationRatio
			);
		}

		Ok(())
	}

	fn check_debit_cap(currency_id: CurrencyIdOf<T>, debit_amount: DebitAmountOf<T>) -> Result<(), Self::Error> {
		let mut total_debit_balance = <vaults::Module<T>>::total_debits(currency_id);
		let debit_balance_adjustment =
			TryInto::<DebitBalanceOf<T>>::try_into(debit_amount.abs()).map_err(|_| Error::DebitAmountConvertFailed)?;
		if debit_amount.is_positive() {
			total_debit_balance += debit_balance_adjustment;
		} else {
			total_debit_balance -= debit_balance_adjustment;
		}
		ensure!(
			!Self::exceed_debit_value_cap(currency_id, total_debit_balance),
			Error::ExceedDebitValueHardCap
		);

		Ok(())
	}
}
