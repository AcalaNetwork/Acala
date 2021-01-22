//! # Prices Module
//!
//! ## Overview
//!
//! The data from Oracle cannot be used in business, prices module will do some
//! process and feed prices for Acala. Process include:
//!   - specify a fixed price for stable currency
//!   - feed price in USD or related price bewteen two currencies
//!   - lock/unlock the price data get from oracle

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

mod default_weight;
mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use frame_support::{pallet_prelude::*, transactional};
	use frame_system::pallet_prelude::*;
	use orml_traits::{DataFeeder, DataProvider};
	use primitives::CurrencyId;
	use sp_runtime::traits::{CheckedDiv, CheckedMul};
	use support::{ExchangeRateProvider, Price, PriceProvider};

	pub trait WeightInfo {
		fn lock_price() -> Weight;
		fn unlock_price() -> Weight;
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The data source, such as Oracle.
		type Source: DataProvider<CurrencyId, Price> + DataFeeder<CurrencyId, Price, Self::AccountId>;

		#[pallet::constant]
		/// The stable currency id, it should be AUSD in Acala.
		type GetStableCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		/// The fixed prices of stable currency, it should be 1 USD in Acala.
		type StableCurrencyFixedPrice: Get<Price>;

		#[pallet::constant]
		/// The staking currency id, it should be DOT in Acala.
		type GetStakingCurrencyId: Get<CurrencyId>;

		#[pallet::constant]
		/// The liquid currency id, it should be LDOT in Acala.
		type GetLiquidCurrencyId: Get<CurrencyId>;

		/// The origin which may lock and unlock prices feed to system.
		type LockOrigin: EnsureOrigin<Self::Origin>;

		/// The provider of the exchange rate between liquid currency and
		/// staking currency.
		type LiquidStakingExchangeRateProvider: ExchangeRateProvider;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Lock price. \[currency_id, locked_price\]
		LockPrice(CurrencyId, Price),
		/// Unlock price. \[currency_id\]
		UnlockPrice(CurrencyId),
	}

	#[pallet::storage]
	#[pallet::getter(fn locked_price)]
	/// Mapping from currency id to it's locked price
	pub type LockedPrice<T: Config> = StorageMap<_, Twox64Concat, CurrencyId, Price, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock the price and feed it to system.
		///
		/// The dispatch origin of this call must be `LockOrigin`.
		///
		/// - `currency_id`: currency type.
		#[pallet::weight((T::WeightInfo::lock_price(), DispatchClass::Operational))]
		#[transactional]
		pub fn lock_price(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResultWithPostInfo {
			T::LockOrigin::ensure_origin(origin)?;
			<Pallet<T> as PriceProvider<CurrencyId>>::lock_price(currency_id);
			Ok(().into())
		}

		/// Unlock the price and get the price from `PriceProvider` again
		///
		/// The dispatch origin of this call must be `LockOrigin`.
		///
		/// - `currency_id`: currency type.
		#[pallet::weight((T::WeightInfo::unlock_price(), DispatchClass::Operational))]
		#[transactional]
		pub fn unlock_price(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResultWithPostInfo {
			T::LockOrigin::ensure_origin(origin)?;
			<Pallet<T> as PriceProvider<CurrencyId>>::unlock_price(currency_id);
			Ok(().into())
		}
	}

	impl<T: Config> PriceProvider<CurrencyId> for Pallet<T> {
		/// get relative price between two currency types
		fn get_relative_price(base_currency_id: CurrencyId, quote_currency_id: CurrencyId) -> Option<Price> {
			if let (Some(base_price), Some(quote_price)) =
				(Self::get_price(base_currency_id), Self::get_price(quote_currency_id))
			{
				base_price.checked_div(&quote_price)
			} else {
				None
			}
		}

		/// get price in USD
		fn get_price(currency_id: CurrencyId) -> Option<Price> {
			if currency_id == T::GetStableCurrencyId::get() {
				// if is stable currency, return fixed price
				Some(T::StableCurrencyFixedPrice::get())
			} else if currency_id == T::GetLiquidCurrencyId::get() {
				// if is homa liquid currency, return the product of staking currency price and
				// liquid/staking exchange rate.
				Self::get_price(T::GetStakingCurrencyId::get())
					.and_then(|n| n.checked_mul(&T::LiquidStakingExchangeRateProvider::get_exchange_rate()))
			} else {
				// if locked price exists, return it, otherwise return latest price from oracle.
				Self::locked_price(currency_id).or_else(|| T::Source::get(&currency_id))
			}
		}

		fn lock_price(currency_id: CurrencyId) {
			// lock price when get valid price from source
			if let Some(val) = T::Source::get(&currency_id) {
				LockedPrice::<T>::insert(currency_id, val);
				<Pallet<T>>::deposit_event(Event::LockPrice(currency_id, val));
			}
		}

		fn unlock_price(currency_id: CurrencyId) {
			LockedPrice::<T>::remove(currency_id);
			<Pallet<T>>::deposit_event(Event::UnlockPrice(currency_id));
		}
	}
}
