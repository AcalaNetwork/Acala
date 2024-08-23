// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

//! # Liquid Crowdloan Module
//!
//! Allow people to redeem lcDOT for DOT.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, traits::EnsureOrigin, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId};
use sp_runtime::{traits::AccountIdConversion, ArithmeticError};

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Liquid crowdloan currency Id, i.e. LCDOT for Polkadot.
		#[pallet::constant]
		type LiquidCrowdloanCurrencyId: Get<CurrencyId>;

		/// Relay chain currency Id, i.e. DOT for Polkadot.
		#[pallet::constant]
		type RelayChainCurrencyId: Get<CurrencyId>;

		/// Pallet Id for liquid crowdloan module.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The governance origin for liquid crowdloan module. For instance for DOT cross-chain
		/// transfer DOT from relay chain crowdloan vault to liquid crowdloan module account.
		type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Liquid Crowdloan asset was redeemed.
		Redeemed { currency_id: CurrencyId, amount: Balance },
		/// The redeem currency id was updated.
		RedeemCurrencyIdUpdated { currency_id: CurrencyId },
	}

	/// The redeem currency id.
	#[pallet::storage]
	pub(crate) type RedeemCurrencyId<T: Config> = StorageValue<_, CurrencyId, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Redeem liquid crowdloan currency for relay chain currency.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn redeem(origin: OriginFor<T>, #[pallet::compact] amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::do_redeem(&who, amount)?;

			Ok(())
		}

		// removed because it is no longer needed
		// #[pallet::call_index(1)]
		// pub fn transfer_from_crowdloan_vault

		/// Set the redeem currency id.
		///
		/// This call requires `GovernanceOrigin`.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_redeem_currency_id())]
		pub fn set_redeem_currency_id(origin: OriginFor<T>, currency_id: CurrencyId) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;

			<RedeemCurrencyId<T>>::put(currency_id);

			Self::deposit_event(Event::RedeemCurrencyIdUpdated { currency_id });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	pub fn do_redeem(who: &T::AccountId, amount: Balance) -> Result<Balance, DispatchError> {
		let (currency_id, redeem_amount) = if let Some(redeem_currency_id) = RedeemCurrencyId::<T>::get() {
			// redeem the RedeemCurrencyId
			// amount_pect = amount / lcdot_total_supply
			// amount_redeem = amount_pect * redeem_currency_balance

			let redeem_currency_balance = T::Currency::free_balance(redeem_currency_id, &Self::account_id());
			let lcdot_total_supply = T::Currency::total_issuance(T::LiquidCrowdloanCurrencyId::get());

			let amount_redeem = amount
				.checked_mul(redeem_currency_balance)
				.and_then(|x| x.checked_div(lcdot_total_supply))
				.ok_or(ArithmeticError::Overflow)?;

			(redeem_currency_id, amount_redeem)
		} else {
			// redeem DOT
			let currency_id = T::RelayChainCurrencyId::get();
			(currency_id, amount)
		};

		T::Currency::withdraw(T::LiquidCrowdloanCurrencyId::get(), who, amount)?;
		T::Currency::transfer(currency_id, &Self::account_id(), who, redeem_amount)?;

		Self::deposit_event(Event::Redeemed {
			currency_id,
			amount: redeem_amount,
		});

		Ok(redeem_amount)
	}

	pub fn redeem_currency() -> CurrencyId {
		RedeemCurrencyId::<T>::get().unwrap_or_else(T::RelayChainCurrencyId::get)
	}
}
