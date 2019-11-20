#![cfg_attr(not(feature = "std"), no_std)]

use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use plette_support::{decl_error, decl_event, decl_module, decl_storage};
use plette_system::{self as system, ensure_signed, ensure_origin};
use sr_primitives::traits::StaticLookup;


use mock;

pub trait Trait: system::Trait + cdp_engine::Trait + vaults::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

type CurrencyIdOf<T> = <<T as cdp_engine::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::CurrencyId;
type BalanceOf<T> = <<T as cdp_engine::Trait>::Currency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type DebitBalanceOf<T> = <<T as cdp_engine::Trait>::DebitCurrency as MultiCurrency<<T as system::Trait>::AccountId>>::Balance;
type AmountOf<T> = <<T as cdp_engine::Trait>::Currency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;
type DebitAmountOf<T> = <<T as cdp_engine::Trait>::DebitCurrency as MultiCurrencyExtended<<T as system::Trait>::AccountId>>::Amount;

decl_storage! {
	trait Store for Module<T: Trait> as Honzon {
		pub Authorization get(fn authorization): double_map T::AccountId, blake2_256((CurrencyIdOf<T>, T::AccountId)) => bool;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId: CurrencyIdOf<T>,
		Amount: AmountOf<T>,
		DebitAmount: AmountOf<T>,
	{
		/// update vaualt success (from, to, currency_id)
		UpdateVault(AccountId, CurrencyId, Amount, DebitAmount),
		/// tranfer vault success (from, to, currency_id)
		TransferVault(AccountId, AccountId, CurrencyId),
		/// authorization (from, to, currency_id)
		Authorization(AccountId, AccountId, CurrencyId),
		/// cancel authorization (from, to, currency_id)
		UnAuthorization(AccountId, AccountId, CurrencyId),
		/// cancel all authorization
		UnAuthorizationAll(AccountId)
	}
);

decl_error! {
	pub enum Error {
		NoAuthorization,
		TransferVaultIsUnSafe,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		pub fn update_vault(
				origin,
				#[compact] currency_id: CurrencyIdOf<T>,
				#[compact] collateral: AmountOf<T>,
				#[compact] debit: AmountOf<T>
			) {
				let who = ensure_signed(origin)?;

				<Self as cdp_engine::Module<T>>::update_position(who, currency_id, collateral, debit)?;

				Self::deposit_event(RawEvent::UpdateVault(who, currency_id, collateral, debit));
		}

		fn transfer_vault(
			origin,
			#[compact] currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;

			// check authorization if `from` can manipulate `to`
			self::check_authorization(to, from, currency_id).map_error(|_| Error::NoAuthorization)?;

			<vaults::Module<T>>::transfer(from.clone(), to.clone(), currency_id)?;

			Self::deposit_event(RawEvent::TransferVault(form, to, currency_id));
		 }

		/// `origin` allow `to` to manipulate the `currency_id` vault
		fn authorize(
			origin,
			#[compact] currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) -> Result<(), Error> {
			let from= ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;

			// update authoration
			<Authorization<T>>::insert(from, (currency_id, to), true);

			Self::deposit_event(RawEvent::Authorization(form, to, currency_id));

			Ok(())
		}

		/// `origin` refuse `to` to manipulate the vault  of `currency_id`
		fn unauthorize(
			origin,
			#[compact] currency_id: CurrencyIdOf<T>,
			to: <T::Lookup as StaticLookup>::Source
		) -> Result<(), Error> {
			let from= ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;

			// update authoration
			<Authorization<T>>::remove(from, (currency_id, to));

			Self::deposit_event(RawEvent::UnAuthorization(form, to, currency_id));

			Ok(())
		}

		/// `origin` refuse anyone to manipulate its vault
		fn unauthorize_all(origin) -> Result<(), Error> {
			let from = ensure_signed(origin)?;

			// update authoration
			<Authorization<T>>::remove_prefix(from);

			Self::deposit_event(RawEvent::AuthorizationAll(form));

			Ok(())
		}
	}
}

impl<T: Trait> Module<T> {
	/// check if `from` allow `to` to manipulate its vault
	pub fn check_authorization(from: T::AccountId, to: T::AccountId, currency_id: CurrencyIdOf<T>) -> Result<(), Error> {

		if from == to {
			return Ok(());
		}

		if let true = T::authorization(from, (currency_id, to)) {
			return Ok(());
		}

		None
	}
}
