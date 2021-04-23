// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

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
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::ReservableCurrency, transactional};
use frame_system::pallet_prelude::*;
use primitives::{Amount, Balance, CurrencyId};
use sp_runtime::{
	traits::{StaticLookup, Zero},
	DispatchResult,
};
use sp_std::vec::Vec;
use support::EmergencyShutdown;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + cdp_engine::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Currency for authorization reserved.
		type Currency: ReservableCurrency<Self::AccountId, Balance = Balance>;

		/// Reserved amount per authorization.
		type DepositPerAuthorization: Get<Balance>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		// No authorization
		NoAuthorization,
		// The system has been shutdown
		AlreadyShutdown,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Authorize someone to operate the loan of specific collateral.
		/// \[authorizer, authorizee, collateral_type\]
		Authorization(T::AccountId, T::AccountId, CurrencyId),
		/// Cancel the authorization of specific collateral for someone.
		/// \[authorizer, authorizee, collateral_type\]
		UnAuthorization(T::AccountId, T::AccountId, CurrencyId),
		/// Cancel all authorization. \[authorizer\]
		UnAuthorizationAll(T::AccountId),
	}

	/// The authorization relationship map from
	/// Authorizer -> (CollateralType, Authorizee) -> Authorized
	///
	/// Authorization: double_map AccountId, (CurrencyId, T::AccountId) => Option<Balance>
	#[pallet::storage]
	#[pallet::getter(fn authorization)]
	pub type Authorization<T: Config> = StorageDoubleMap<
		_,
		Twox64Concat,
		T::AccountId,
		Blake2_128Concat,
		(CurrencyId, T::AccountId),
		Balance,
		OptionQuery,
	>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Adjust the loans of `currency_id` by specific
		/// `collateral_adjustment` and `debit_adjustment`
		///
		/// - `currency_id`: collateral currency id.
		/// - `collateral_adjustment`: signed amount, positive means to deposit collateral currency
		///   into CDP, negative means withdraw collateral currency from CDP.
		/// - `debit_adjustment`: signed amount, positive means to issue some amount of stablecoin
		///   to caller according to the debit adjustment, negative means caller will payback some
		///   amount of stablecoin to CDP according to to the debit adjustment.
		#[pallet::weight(<T as Config>::WeightInfo::adjust_loan())]
		#[transactional]
		pub fn adjust_loan(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			collateral_adjustment: Amount,
			debit_adjustment: Amount,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// not allowed to adjust the debit after system shutdown
			if !debit_adjustment.is_zero() {
				ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			}
			<cdp_engine::Pallet<T>>::adjust_position(&who, currency_id, collateral_adjustment, debit_adjustment)?;
			Ok(().into())
		}

		#[pallet::weight(<T as Config>::WeightInfo::close_loan_has_debit_by_dex())]
		#[transactional]
		pub fn close_loan_has_debit_by_dex(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			maybe_path: Option<Vec<CurrencyId>>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			<cdp_engine::Pallet<T>>::close_cdp_has_debit_by_dex(who, currency_id, maybe_path.as_deref())?;
			Ok(().into())
		}

		/// Transfer the whole CDP of `from` under `currency_id` to caller's CDP
		/// under the same `currency_id`, caller must have the authorization of
		/// `from` for the specific collateral type
		///
		/// - `currency_id`: collateral currency id.
		/// - `from`: authorizer account
		#[pallet::weight(<T as Config>::WeightInfo::transfer_loan_from())]
		#[transactional]
		pub fn transfer_loan_from(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			from: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let to = ensure_signed(origin)?;
			let from = T::Lookup::lookup(from)?;
			ensure!(!T::EmergencyShutdown::is_shutdown(), Error::<T>::AlreadyShutdown);
			Self::check_authorization(&from, &to, currency_id)?;
			<loans::Pallet<T>>::transfer_loan(&from, &to, currency_id)?;
			Ok(().into())
		}

		/// Authorize `to` to manipulate the loan under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		#[pallet::weight(<T as Config>::WeightInfo::authorize())]
		#[transactional]
		pub fn authorize(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			to: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			Authorization::<T>::try_mutate_exists(&from, (currency_id, &to), |maybe_reserved| -> DispatchResult {
				if maybe_reserved.is_none() {
					let reserve_amount = T::DepositPerAuthorization::get();
					<T as Config>::Currency::reserve(&from, reserve_amount)?;
					*maybe_reserved = Some(reserve_amount);
					Self::deposit_event(Event::Authorization(from.clone(), to.clone(), currency_id));
				}
				Ok(())
			})?;
			Ok(().into())
		}

		/// Cancel the authorization for `to` under `currency_id`
		///
		/// - `currency_id`: collateral currency id.
		/// - `to`: authorizee account
		#[pallet::weight(<T as Config>::WeightInfo::unauthorize())]
		#[transactional]
		pub fn unauthorize(
			origin: OriginFor<T>,
			currency_id: CurrencyId,
			to: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let to = T::Lookup::lookup(to)?;
			if let Some(reserved) = Authorization::<T>::take(&from, (currency_id, &to)) {
				<T as Config>::Currency::unreserve(&from, reserved);
				Self::deposit_event(Event::UnAuthorization(from, to, currency_id));
			}
			Ok(().into())
		}

		/// Cancel all authorization of caller
		#[pallet::weight(<T as Config>::WeightInfo::unauthorize_all(<T as cdp_engine::Config>::CollateralCurrencyIds::get().len() as u32))]
		#[transactional]
		pub fn unauthorize_all(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let from = ensure_signed(origin)?;
			let total_reserved: Balance = Authorization::<T>::drain_prefix(&from)
				.fold(Zero::zero(), |total_reserved, (_, reserved)| {
					total_reserved.saturating_add(reserved)
				});
			<T as Config>::Currency::unreserve(&from, total_reserved);
			Self::deposit_event(Event::UnAuthorizationAll(from));
			Ok(().into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Check if `from` has the authorization of `to` under `currency_id`
	fn check_authorization(from: &T::AccountId, to: &T::AccountId, currency_id: CurrencyId) -> DispatchResult {
		ensure!(
			from == to || Authorization::<T>::contains_key(from, (currency_id, to)),
			Error::<T>::NoAuthorization
		);
		Ok(())
	}
}
