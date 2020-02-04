#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage, ensure};
use frame_system::{self as system, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::{traits::Zero, DispatchResult};
use support::EmergencyShutdown;

mod mock;
mod tests;

pub trait Trait: system::Trait + cdp_engine::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type CurrencyIdOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type AmountOf<T> = <<T as vaults::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

decl_storage! {
	trait Store for Module<T: Trait> as Honzon {
		pub Authorization get(fn authorization): double_map hasher(blake2_256) T::AccountId, hasher(blake2_256) (CurrencyIdOf<T>, T::AccountId) => bool;
		pub IsShutdown get(fn is_shutdown): bool;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
	{
		/// authorization (from, to, currency_id)
		Authorization(AccountId, AccountId, CurrencyId),
		/// cancel authorization (from, to, currency_id)
		UnAuthorization(AccountId, AccountId, CurrencyId),
		/// cancel all authorization
		UnAuthorizationAll(AccountId),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
		NoAuthorization,
		LiquidateFailed,
		AlreadyShutdown,
		MustAfterShutdown,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		pub fn liquidate(_origin, who: T::AccountId, currency_id: CurrencyIdOf<T>) {
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			<cdp_engine::Module<T>>::liquidate_unsafe_cdp(who.clone(), currency_id).map_err(|_| Error::<T>::LiquidateFailed)?;
		}

		pub fn settle_cdp(_origin, who: T::AccountId, currency_id: CurrencyIdOf<T>) {
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);

			<cdp_engine::Module<T>>::settle_cdp_has_debit(who, currency_id)?;
		}

		pub fn update_vault(
			origin,
			currency_id: CurrencyIdOf<T>,
			collateral: AmountOf<T>,
			debit: T::DebitAmount,
		) {
			let who = ensure_signed(origin)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			<cdp_engine::Module<T>>::update_position(&who, currency_id, collateral, debit)?;
		}

		pub fn withdraw_collateral(
			origin,
			currency_id: CurrencyIdOf<T>,
			collateral: AmountOf<T>,
		) {
			let who = ensure_signed(origin)?;
			ensure!(Self::is_shutdown(), Error::<T>::MustAfterShutdown);

			<cdp_engine::Module<T>>::update_position(&who, currency_id, collateral, T::DebitAmount::zero())?;
		}

		pub fn transfer_vault_from(
			origin,
			currency_id: CurrencyIdOf<T>,
			from: T::AccountId,
		) {
			let to = ensure_signed(origin)?;
			ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);

			// check authorization if `from` can manipulate `to`
			Self::check_authorization(&from, &to, currency_id)?;

			<vaults::Module<T>>::transfer(from.clone(), to.clone(), currency_id)?;
		}

		/// `origin` allow `to` to manipulate the `currency_id` vault
		pub fn authorize(
			origin,
			currency_id: CurrencyIdOf<T>,
			to: T::AccountId,
		) {
			let from = ensure_signed(origin)?;

			// update authorization
			<Authorization<T>>::insert(&from, (currency_id, &to), true);

			Self::deposit_event(RawEvent::Authorization(from, to, currency_id));
		}

		/// `origin` refuse `to` to manipulate the vault  of `currency_id`
		pub fn unauthorize(
			origin,
			currency_id: CurrencyIdOf<T>,
			to: T::AccountId,
		) {
			let from = ensure_signed(origin)?;

			// update authorization
			<Authorization<T>>::remove(&from, (currency_id, &to));

			Self::deposit_event(RawEvent::UnAuthorization(from, to, currency_id));
		}

		/// `origin` refuse anyone to manipulate its vault
		pub fn unauthorize_all(origin) {
			let from = ensure_signed(origin)?;

			// update authorization
			<Authorization<T>>::remove_prefix(&from);

			Self::deposit_event(RawEvent::UnAuthorizationAll(from));
		}
	}
}

impl<T: Trait> Module<T> {
	/// check if `from` allow `to` to manipulate its vault
	pub fn check_authorization(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyIdOf<T>) -> DispatchResult {
		ensure!(
			from == to || Self::authorization(from, (currency_id, to)),
			Error::<T>::NoAuthorization
		);
		Ok(())
	}

	pub fn emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}

impl<T: Trait> EmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		Self::emergency_shutdown();
	}
}
