#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_event, decl_module, decl_storage,
	traits::{EnsureOrigin, Get},
};
use frame_system::{self as system, ensure_root};
use orml_traits::DataProvider;
use primitives::CurrencyId;
use support::{ExchangeRateProvider, Price, PriceProvider};

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type Event: From<Event> + Into<<Self as system::Trait>::Event>;
	type Source: DataProvider<CurrencyId, Price>;
	type GetStableCurrencyId: Get<CurrencyId>;
	type StableCurrencyFixedPrice: Get<Price>;
	type GetStakingCurrencyId: Get<CurrencyId>;
	type GetLiquidCurrencyId: Get<CurrencyId>;
	type LockOrigin: EnsureOrigin<Self::Origin>;
	type LiquidStakingExchangeRateProvider: ExchangeRateProvider;
}

decl_event!(
	pub enum Event {
		LockPrice(CurrencyId, Price),
		UnlockPrice(CurrencyId),
	}
);

decl_storage! {
	trait Store for Module<T: Trait> as Prices {
		LockedPrice get(fn locked_price): map hasher(twox_64_concat) CurrencyId => Option<Price>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		const GetStableCurrencyId: CurrencyId = T::GetStableCurrencyId::get();
		const StableCurrencyFixedPrice: Price = T::StableCurrencyFixedPrice::get();

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		fn lock_price(origin, currency_id: CurrencyId) {
			T::LockOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;

			<Module<T> as PriceProvider<CurrencyId>>::lock_price(currency_id);
		}

		#[weight = frame_support::weights::SimpleDispatchInfo::default()]
		fn unlock_price(origin, currency_id: CurrencyId) {
			T::LockOrigin::try_origin(origin)
				.map(|_| ())
				.or_else(ensure_root)?;

			<Module<T> as PriceProvider<CurrencyId>>::unlock_price(currency_id);
		}
	}
}

impl<T: Trait> Module<T> {}

impl<T: Trait> PriceProvider<CurrencyId> for Module<T> {
	fn get_relative_price(base_currency_id: CurrencyId, quote_currency_id: CurrencyId) -> Option<Price> {
		if let (Some(base_price), Some(quote_price)) =
			(Self::get_price(base_currency_id), Self::get_price(quote_currency_id))
		{
			base_price.checked_div(&quote_price)
		} else {
			None
		}
	}

	fn get_price(currency_id: CurrencyId) -> Option<Price> {
		if currency_id == T::GetStableCurrencyId::get() {
			// if is stable currency, return fix price
			Some(T::StableCurrencyFixedPrice::get())
		} else if currency_id == T::GetLiquidCurrencyId::get() {
			// if is homa liquid currency,
			// return the product of staking currency price and liquid/staking exchange rate.
			if let Some(staking_currency_price) = Self::get_price(T::GetStakingCurrencyId::get()) {
				let exchange_rate: Price = T::LiquidStakingExchangeRateProvider::get_exchange_rate().into();
				staking_currency_price.checked_mul(&exchange_rate)
			} else {
				None
			}
		} else {
			// if locked price exists, return it,
			// otherwise return the price get from oracle.
			if let Some(locked_price) = Self::locked_price(currency_id) {
				// if there's locked price return it
				Some(locked_price)
			} else {
				// get latest price from oracle
				T::Source::get(&currency_id)
			}
		}
	}

	fn lock_price(currency_id: CurrencyId) {
		// lock price when get valid price from source
		if let Some(val) = T::Source::get(&currency_id) {
			LockedPrice::insert(currency_id, val);
			<Module<T>>::deposit_event(Event::LockPrice(currency_id, val));
		}
	}

	fn unlock_price(currency_id: CurrencyId) {
		LockedPrice::remove(currency_id);
		<Module<T>>::deposit_event(Event::UnlockPrice(currency_id));
	}
}
