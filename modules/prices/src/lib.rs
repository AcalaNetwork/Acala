#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, traits::Get, Parameter};
use orml_traits::DataProvider;
use sp_runtime::traits::{MaybeSerializeDeserialize, Member};
use support::{Price, PriceProvider};

mod mock;
mod tests;

pub trait Trait: system::Trait {
	type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize;
	type Source: DataProvider<Self::CurrencyId, Price>;
	type GetStableCurrencyId: Get<Self::CurrencyId>;
	type StableCurrencyFixedPrice: Get<Price>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Prices {
		LockedPrice get(fn locked_price): map hasher(blake2_256) T::CurrencyId => Option<Option<Price>>;
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		const GetStableCurrencyId: T::CurrencyId = T::GetStableCurrencyId::get();
		const StableCurrencyFixedPrice: Price = T::StableCurrencyFixedPrice::get();
	}
}

impl<T: Trait> Module<T> {}

impl<T: Trait> PriceProvider<T::CurrencyId, Price> for Module<T> {
	fn get_price(base_currency_id: T::CurrencyId, quote_currency_id: T::CurrencyId) -> Option<Price> {
		let stable_currency_id = T::GetStableCurrencyId::get();
		let stable_currency_price = T::StableCurrencyFixedPrice::get();

		let base_price = if base_currency_id == stable_currency_id {
			stable_currency_price
		} else if let Some(locked_price) = Self::locked_price(base_currency_id) {
			locked_price?
		} else {
			T::Source::get(&base_currency_id)?
		};

		let quote_price = if quote_currency_id == stable_currency_id {
			stable_currency_price
		} else if let Some(locked_price) = Self::locked_price(quote_currency_id) {
			locked_price?
		} else {
			T::Source::get(&quote_currency_id)?
		};

		quote_price.checked_div(&base_price)
	}

	fn lock_price(currency_id: T::CurrencyId) {
		<LockedPrice<T>>::insert(currency_id, T::Source::get(&currency_id));
	}

	fn unlock_price(currency_id: T::CurrencyId) {
		<LockedPrice<T>>::remove(currency_id);
	}
}
