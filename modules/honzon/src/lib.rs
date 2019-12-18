#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_error, decl_event, decl_module, decl_storage};
use frame_system::{self as system, ensure_signed};
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_runtime::traits::StaticLookup;

mod mock;
mod tests;

pub trait Trait: system::Trait + cdp_engine::Trait + vaults::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type CurrencyIdOf<T> = <<T as vaults::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type AmountOf<T> = <<T as vaults::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;
type DebitAmountOf<T> =
	<<T as vaults::Trait>::DebitCurrency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

decl_storage! {
	trait Store for Module<T: Trait> as Honzon {
		pub Authorization get(fn authorization): double_map T::AccountId, blake2_256((CurrencyIdOf<T>, T::AccountId)) => bool;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyIdOf<T>,
		Amount = AmountOf<T>,
		DebitAmount = DebitAmountOf<T>,
	{
		/// liquidate `who` `currency` vault
		Liquidate(AccountId, CurrencyId),
		/// update vault success (from, to, currency_id)
		UpdateVault(AccountId, CurrencyId, Amount, DebitAmount),
		/// transfer vault success (from, to, currency_id)
		TransferVault(AccountId, AccountId, CurrencyId),
		/// authorization (from, to, currency_id)
		Authorization(AccountId, AccountId, CurrencyId),
		/// cancel authorization (from, to, currency_id)
		UnAuthorization(AccountId, AccountId, CurrencyId),
		/// cancel all authorization
		UnAuthorizationAll(AccountId),
	}
);

decl_error! {
	pub enum Error {
		AccountUnSigned,
		NoAuthorization,
		TransferVaultFailed,
		UpdatePositionFailed,
		LiquidateFailed,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {

		fn deposit_event() = default;

		fn liquidate(_origin, who: <T::Lookup as StaticLookup>::Source, currency_id: CurrencyIdOf<T>) {
			let who = T::Lookup::lookup(who).map_err(|_| Error::AccountUnSigned)?;

			<cdp_engine::Module<T>>::liquidate_unsafe_cdp(who.clone(), currency_id).map_err(|_| Error::LiquidateFailed)?;

			Self::deposit_event(RawEvent::Liquidate(who, currency_id));
		}

		fn update_vault(
			origin,
			currency_id: CurrencyIdOf<T>,
			collateral: AmountOf<T>,
			debit: DebitAmountOf<T>
		) {
			let who = ensure_signed(origin).map_err(|_| Error::AccountUnSigned)?;

			<cdp_engine::Module<T>>::update_position(&who, currency_id, collateral, debit).map_err(|_| Error::UpdatePositionFailed)?;

			Self::deposit_event(RawEvent::UpdateVault(who, currency_id, collateral, debit));
		}

		fn transfer_vault(
			origin,
			currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) {
			let from = ensure_signed(origin).map_err(|_| Error::AccountUnSigned)?;
			let to = T::Lookup::lookup(to).map_err(|_| Error::AccountUnSigned)?;

			// check authorization if `from` can manipulate `to`
			Self::check_authorization(&to, &from, currency_id)?;

			<vaults::Module<T>>::transfer(from.clone(), to.clone(), currency_id).map_err(|_|
				Error::TransferVaultFailed
			)?;

			Self::deposit_event(RawEvent::TransferVault(from, to, currency_id));
		 }

		/// `origin` allow `to` to manipulate the `currency_id` vault
		fn authorize(
			origin,
			currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) {
			let from = ensure_signed(origin).map_err(|_| Error::AccountUnSigned)?;
			let to = T::Lookup::lookup(to).map_err(|_| Error::AccountUnSigned)?;

			// update authorization
			<Authorization<T>>::insert(&from, (currency_id, &to), true);

			Self::deposit_event(RawEvent::Authorization(from, to, currency_id));
		}

		/// `origin` refuse `to` to manipulate the vault  of `currency_id`
		fn unauthorize(
			origin,
			currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) {
			let from = ensure_signed(origin).map_err(|_| Error::AccountUnSigned)?;
			let to = T::Lookup::lookup(to).map_err(|_| Error::AccountUnSigned)?;

			// update authorization
			<Authorization<T>>::remove(&from, (currency_id, &to));

			Self::deposit_event(RawEvent::UnAuthorization(from, to, currency_id));
		}

		/// `origin` refuse anyone to manipulate its vault
		fn unauthorize_all(origin) {
			let from = ensure_signed(origin).map_err(|_| Error::AccountUnSigned)?;

			// update authorization
			<Authorization<T>>::remove_prefix(&from);

			Self::deposit_event(RawEvent::UnAuthorizationAll(from));
		}
	}
}

impl<T: Trait> Module<T> {
	/// check if `from` allow `to` to manipulate its vault
	pub fn check_authorization(
		from: &T::AccountId,
		to: &T::AccountId,
		currency_id: CurrencyIdOf<T>,
	) -> Result<(), Error> {
		if from == to {
			return Ok(());
		}

		if Self::authorization(from, (currency_id, to)) {
			return Ok(());
		}

		Err(Error::NoAuthorization)
	}
}
