//! # Honzon Module
//!
//! ## Overview
//!
//! The entry of the Honzon protocol for users, user can manipulate their CDP
//! position to loan/payback, and can also authorize others to manage the their
//! CDP under specific collateral type.
//!
//! After system shutdown, some operations will be restricted.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure, traits::Get, transactional, weights::Weight,
};
use frame_system::{self as system, ensure_signed};
use primitives::{Amount, CurrencyId};
use sp_runtime::{traits::Zero, DispatchResult};
use support::EmergencyShutdown;

mod default_weight;
mod mock;
mod tests;

pub trait WeightInfo {
	fn authorize() -> Weight;
	fn unauthorize() -> Weight;
	fn unauthorize_all(c: u32) -> Weight;
	fn adjust_loan() -> Weight;
	fn transfer_loan_from() -> Weight;
}

pub trait Config: system::Config + cdp_engine::Config {
	type Event: From<Event<Self>> + Into<<Self as system::Config>::Event>;

	/// Weight information for the extrinsics in this module.
	type WeightInfo: WeightInfo;
}

decl_storage! {
	trait Store for Module<T: Config> as Honzon {
		/// The authorization relationship map from
		/// Authorizer -> (CollateralType, Authorizee) -> Authorized
		pub Authorization get(fn authorization): double_map hasher(twox_64_concat) T::AccountId, hasher(blake2_128_concat) (CurrencyId, T::AccountId) => bool;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Config>::AccountId,
		CurrencyId = CurrencyId,
	{
		/// Authorize someone to operate the loan of specific collateral. \[authorizer, authorizee, collateral_type\]
		Authorization(AccountId, AccountId, CurrencyId),
		/// Cancel the authorization of specific collateral for someone. \[authorizer, authorizee, collateral_type\]
		UnAuthorization(AccountId, AccountId, CurrencyId),
		/// Cancel all authorization. \[authorizer\]
		UnAuthorizationAll(AccountId),
	}
);

decl_error! {
	/// Error for the honzon module.
	pub enum Error for Module<T: Config> {
		// No authorization
		NoAuthorization,
		// The system has been shutdown
		AlreadyShutdown,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;
		fn deposit_event() = default;

		/// Adjust the loans of `currency_id` by specific `collateral_adjustment` and `debit_adjustment`
		///
		/// - `currency_id`: collateral currency id.
		/// - `collateral_adjustment`: signed amount, positive means to deposit collateral currency into CDP,
		///			negative means withdraw collateral currency from CDP.
		/// - `debit_adjustment`: signed amount, positive means to issue some amount of stablecoin to caller according to the debit adjustment,
		///			negative means caller will payback some amount of stablecoin to CDP according to to the debit adjustment.
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: 17
		/// - Db writes: 9
		/// -------------------
		/// Base Weight: 246.2 µs
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::adjust_loan()]
		#[transactional]
		pub fn adjust_loan(
			origin,
			currency_id: CurrencyId,
			collateral_adjustment: Amount,
			debit_adjustment: Amount,
		) {
			let who = ensure_signed(origin)?;

			// not allowed to adjust the debit after system shutdown
			if !debit_adjustment.is_zero() {
				ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			}
			<cdp_engine::Module<T>>::adjust_position(&who, currency_id, collateral_adjustment, debit_adjustment)?;
		}

		/// Transfer the whole CDP of `from` under `currency_id` to caller's CDP under the same `currency_id`,
		/// caller must have the authorization of `from` for the specific collateral type
		///
		/// - `currency_id`: collateral currency id.
		/// - `from`: authorizer account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: 13
		/// - Db writes: 6
		/// -------------------
		/// Base Weight: 178.2 µs
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::transfer_loan_from()]
		#[transactional]
		pub fn transfer_loan_from(
			origin,
			currency_id: CurrencyId,
			from: T::AccountId,
		) {
			let to = ensure_signed(origin)?;
			ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			Self::check_authorization(&from, &to, currency_id)?;
			<loans::Module<T>>::transfer_loan(&from, &to, currency_id)?;
		}

		/// Authorize `to` to manipulate the loan under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: 0
		/// - Db writes: 1
		/// -------------------
		/// Base Weight: 27.82 µs
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::authorize()]
		#[transactional]
		pub fn authorize(
			origin,
			currency_id: CurrencyId,
			to: T::AccountId,
		) {
			let from = ensure_signed(origin)?;
			<Authorization<T>>::insert(&from, (currency_id, &to), true);
			Self::deposit_event(RawEvent::Authorization(from, to, currency_id));
		}

		/// Cancel the authorization for `to` under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: 0
		/// - Db writes: 1
		/// -------------------
		/// Base Weight: 28.14 µs
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::unauthorize()]
		#[transactional]
		pub fn unauthorize(
			origin,
			currency_id: CurrencyId,
			to: T::AccountId,
		) {
			let from = ensure_signed(origin)?;
			<Authorization<T>>::remove(&from, (currency_id, &to));
			Self::deposit_event(RawEvent::UnAuthorization(from, to, currency_id));
		}

		/// Cancel all authorization of caller
		///
		/// # <weight>
		/// - Complexity: `O(C + M)` where C is the length of collateral_ids and M is the number of authorizees
		/// - Db reads: 0
		/// - Db writes: 1
		/// -------------------
		/// Base Weight: 0 + 3.8 * M + 128.4 * C µs
		/// # </weight>
		#[weight = <T as Config>::WeightInfo::unauthorize_all(<T as cdp_engine::Config>::CollateralCurrencyIds::get().len() as u32)]
		#[transactional]
		pub fn unauthorize_all(origin) {
			let from = ensure_signed(origin)?;
			<Authorization<T>>::remove_prefix(&from);
			Self::deposit_event(RawEvent::UnAuthorizationAll(from));
		}
	}
}

impl<T: Config> Module<T> {
	/// Check if `from` has the authorization of `to` under `currency_id`
	fn check_authorization(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		ensure!(
			from == to || Self::authorization(from, (currency_id, to)),
			Error::<T>::NoAuthorization
		);
		Ok(())
	}
}
