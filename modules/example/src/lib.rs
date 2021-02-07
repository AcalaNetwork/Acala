//! # Example Module
//!
//! A simple example of a FRAME pallet demonstrating
//! concepts, APIs and structures common to most FRAME runtimes.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

mod mock;
mod tests;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Balance: Parameter + codec::HasCompact + From<u32> + Into<Weight> + Default + MaybeSerializeDeserialize;
		#[pallet::constant]
		type SomeConst: Get<Self::Balance>;
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Some wrong behavior
		Wrong,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	#[pallet::metadata(T::Balance = "Balance")]
	pub enum Event<T: Config> {
		/// Dummy event, just here so there's a generic type that's used.
		Dummy(T::Balance),
	}

	#[pallet::type_value]
	pub fn OnFooEmpty<T: Config>() -> T::Balance {
		3.into()
	}

	/// Some documentation
	#[pallet::storage]
	#[pallet::getter(fn dummy)]
	type Dummy<T: Config> = StorageValue<_, T::Balance, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bar)]
	pub(crate) type Bar<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::Balance, ValueQuery>;

	#[pallet::storage]
	type Foo<T: Config> = StorageValue<_, T::Balance, ValueQuery, OnFooEmpty<T>>;

	#[pallet::storage]
	type Double<T: Config> = StorageDoubleMap<_, Blake2_128Concat, u32, Twox64Concat, u64, T::Balance, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub dummy: Option<T::Balance>,
		pub bar: Vec<(T::AccountId, T::Balance)>,
		pub foo: T::Balance,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				dummy: Default::default(),
				bar: Default::default(),
				foo: OnFooEmpty::<T>::get(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			if let Some(dummy) = self.dummy.as_ref() {
				Dummy::<T>::put(dummy);
			}
			for (k, v) in &self.bar {
				Bar::<T>::insert(k, v);
			}
			Foo::<T>::put(&self.foo);
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(_n: T::BlockNumber) -> Weight {
			Dummy::<T>::put(T::Balance::from(10));
			10
		}

		fn on_finalize(_n: T::BlockNumber) {
			Dummy::<T>::put(T::Balance::from(11));
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T::Balance as Into<Weight>>::into(new_value.clone()))]
		pub fn set_dummy(origin: OriginFor<T>, #[pallet::compact] new_value: T::Balance) -> DispatchResultWithPostInfo {
			ensure_root(origin)?;

			Dummy::<T>::put(&new_value);
			Self::deposit_event(Event::Dummy(new_value));

			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_set_bar(who: &T::AccountId, amount: T::Balance) {
		Bar::<T>::insert(who, amount);
	}
}
