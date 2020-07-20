//! # Honzon Module
//!
//! ## Overview
//!
//! The entry of the Honzon protocol for users, user can manipulate their CDP position to loan/payback,
//! and can also authorize others to manage the their CDP under specific collateral type.
//!
//! After system shutdown, some operations will be restricted.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, ensure,
	traits::Get,
	weights::{constants::WEIGHT_PER_MICROS, Weight},
};
use frame_system::{self as system, ensure_signed};
use orml_utilities::with_transaction_result;
use primitives::{Amount, CurrencyId};
use sp_runtime::{traits::Zero, DispatchResult};
use support::OnEmergencyShutdown;

mod mock;
mod tests;

pub trait Trait: system::Trait + cdp_engine::Trait {
	type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}

decl_storage! {
	trait Store for Module<T: Trait> as Honzon {
		/// The authorization relationship map from
		/// Authorizer -> (CollateralType, Authorizee) -> Authorized
		pub Authorization get(fn authorization): double_map hasher(twox_64_concat) T::AccountId, hasher(blake2_128_concat) (CurrencyId, T::AccountId) => bool;

		/// System shutdown flag
		pub IsShutdown get(fn is_shutdown): bool;
	}
}

decl_event!(
	pub enum Event<T> where
		<T as system::Trait>::AccountId,
		CurrencyId = CurrencyId,
	{
		/// Authorize someone to operate the loan of specific collateral (authorizer, authorizee, collateral_type)
		Authorization(AccountId, AccountId, CurrencyId),
		/// Cancel the authorization of specific collateral for someone  (authorizer, authorizee, collateral_type)
		UnAuthorization(AccountId, AccountId, CurrencyId),
		/// Cancel all authorization (authorizer)
		UnAuthorizationAll(AccountId),
	}
);

decl_error! {
	/// Error for the honzon module.
	pub enum Error for Module<T: Trait> {
		// No authorization
		NoAuthorization,
		// The system has been shutdown
		AlreadyShutdown,
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
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
		/// - Db reads: `IsShutdown`, (4 + 4 + 4 + 1 + 2) items in modules related to module_loans and module_cdp_engine
		/// - Db writes: (4 + 4 + 1) items in modules related to module_loans and module_cdp_engine
		/// -------------------
		/// Base Weight: 99.77 µs
		/// # </weight>
		#[weight = 100 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(16, 9)]
		pub fn adjust_loan(
			origin,
			currency_id: CurrencyId,
			collateral_adjustment: Amount,
			debit_adjustment: T::DebitAmount,
		) {
			with_transaction_result(|| {
				let who = ensure_signed(origin)?;

				// not allowed to adjust the debit after system shutdown
				if !debit_adjustment.is_zero() {
					ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);
				}
				<cdp_engine::Module<T>>::adjust_position(&who, currency_id, collateral_adjustment, debit_adjustment)?;
				Ok(())
			})?;
		}

		/// Transfer the whole CDP of `from` under `currency_id` to caller's CDP under the same `currency_id`,
		/// caller must have the authorization of `from` for the specific collateral type
		///
		/// - `currency_id`: collateral currency id.
		/// - `from`: authorizer account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads: `IsShutdown`, `Authorization`, (4 + 3 + 2) items in modules related to module_loans and module_cdp_engine
		/// - Db writes: 4 items in module_loans
		/// -------------------
		/// Base Weight: 74.81 µs
		/// # </weight>
		#[weight = 75 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(11, 4)]
		pub fn transfer_loan_from(
			origin,
			currency_id: CurrencyId,
			from: T::AccountId,
		) {
			with_transaction_result(|| {
				let to = ensure_signed(origin)?;
				ensure!(!Self::is_shutdown(), Error::<T>::AlreadyShutdown);
				Self::check_authorization(&from, &to, currency_id)?;
				<loans::Module<T>>::transfer_loan(&from, &to, currency_id)?;
				Ok(())
			})?;
		}

		/// Authorize `to` to manipulate the loan under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads:
		/// - Db writes: `Authorization`
		/// -------------------
		/// Base Weight: 20.04 µs
		/// # </weight>
		#[weight = 20 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(0, 1)]
		pub fn authorize(
			origin,
			currency_id: CurrencyId,
			to: T::AccountId,
		) {
			with_transaction_result(|| {
				let from = ensure_signed(origin)?;
				<Authorization<T>>::insert(&from, (currency_id, &to), true);
				Self::deposit_event(RawEvent::Authorization(from, to, currency_id));
				Ok(())
			})?;
		}

		/// Cancel the authorization for `to` under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		///
		/// # <weight>
		/// - Complexity: `O(1)`
		/// - Db reads:
		/// - Db writes: `Authorization`
		/// -------------------
		/// Base Weight: 19.77 µs
		/// # </weight>
		#[weight = 20 * WEIGHT_PER_MICROS + T::DbWeight::get().reads_writes(0, 1)]
		pub fn unauthorize(
			origin,
			currency_id: CurrencyId,
			to: T::AccountId,
		) {
			with_transaction_result(|| {
				let from = ensure_signed(origin)?;
				<Authorization<T>>::remove(&from, (currency_id, &to));
				Self::deposit_event(RawEvent::UnAuthorization(from, to, currency_id));
				Ok(())
			})?;
		}

		/// Cancel all authorization of caller
		///
		/// # <weight>
		/// - Complexity: `O(C + M)` where C is the length of collateral_ids and M is the number of authorizees
		/// - Db reads:
		/// - Db writes: `Authorization`
		/// -------------------
		/// Base Weight: 0 + 2.5 * M + 115 * C µs
		/// # </weight>
		#[weight = T::DbWeight::get().reads_writes(0, 1) +
			((WEIGHT_PER_MICROS as u64) * 115).saturating_mul(Weight::from(<T as cdp_engine::Trait>::CollateralCurrencyIds::get().len() as u32))
		]
		pub fn unauthorize_all(origin) {
			with_transaction_result(|| {
				let from = ensure_signed(origin)?;
				<Authorization<T>>::remove_prefix(&from);
				Self::deposit_event(RawEvent::UnAuthorizationAll(from));
				Ok(())
			})?;
		}
	}
}

impl<T: Trait> Module<T> {
	/// Check if `from` has the authorization of `to` under `currency_id`
	fn check_authorization(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		ensure!(
			from == to || Self::authorization(from, (currency_id, to)),
			Error::<T>::NoAuthorization
		);
		Ok(())
	}
}

impl<T: Trait> OnEmergencyShutdown for Module<T> {
	fn on_emergency_shutdown() {
		<IsShutdown>::put(true);
	}
}
