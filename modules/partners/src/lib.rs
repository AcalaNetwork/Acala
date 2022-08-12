// This file is part of Acala.

// Copyright (C) 2020-2022 Acala Foundation.
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

//! # Partners Module
//!
//! ## Overview
//!
//! Partners Module:

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::*, transactional, PalletId};
use frame_system::pallet_prelude::*;
use orml_traits::MultiCurrency;
use primitives::{Balance, CurrencyId};
use sp_runtime::{
	traits::{AccountIdConversion, BlockNumberProvider, One, Saturating, Zero},
	ArithmeticError, DispatchResult,
};

mod mock;
mod tests;

pub use module::*;

type PartnerId = u32;

pub trait OnFeeDeposited<AccountId, Balance, CurrencyId> {
	fn on_fee_deposited(origin: &AccountId, currency_id: CurrencyId, amount: Balance);
}

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct ReferralInfo<BlockNumber> {
	partner_id: PartnerId,
	expiry: BlockNumber,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_proxy::Config {
		/// Overarching event type
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Currency type for transfer of currencies
		type Currencies: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;

		/// Permission to do admin calls in pallet
		type AdminOrigin: EnsureOrigin<Self::Origin>;

		/// Native CurrencyId
		#[pallet::constant]
		type GetNativeCurrencyId: Get<CurrencyId>;

		/// Fee for registering a partner_id to account
		#[pallet::constant]
		type RegisterFee: Get<Balance>;

		/// The partner's module id
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Number of blocks a referral lasts
		#[pallet::constant]
		type ReferralExpire: Get<Self::BlockNumber>;

		/// Treasury's AccountId
		#[pallet::constant]
		type TreasuryAccount: Get<Self::AccountId>;

		/// Max size of metadata string
		#[pallet::constant]
		type MaxMetadataLength: Get<u32>;

		/// Provides current blocknumber
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;
	}

	#[pallet::error]
	pub enum Error<T> {
		SubAccountGenerationFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Referral succesfully set to PartnerId
		ReferralSet { who: T::AccountId, partner_id: PartnerId },
		/// New Partner successfully registered
		PartnerRegistered {
			owner: T::AccountId,
			partner_id: PartnerId,
			partner_account: T::AccountId,
		},
		/// Partner's metadata is updated
		PartnerMetadataUpdated { partner_id: PartnerId },
	}

	#[pallet::storage]
	pub type NextId<T: Config> = StorageValue<_, PartnerId, ValueQuery>;

	#[pallet::storage]
	pub type Partners<T: Config> =
		StorageMap<_, Identity, PartnerId, BoundedVec<u8, T::MaxMetadataLength>, OptionQuery>;

	#[pallet::storage]
	pub type Referral<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, ReferralInfo<T::BlockNumber>, OptionQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		#[transactional]
		pub fn register_partner(
			origin: OriginFor<T>,
			metadata: BoundedVec<u8, T::MaxMetadataLength>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			// transfer registration fee to treasury
			let treasury_account = T::TreasuryAccount::get();
			<T as module::Config>::Currencies::transfer(
				T::GetNativeCurrencyId::get(),
				&who,
				&treasury_account,
				T::RegisterFee::get(),
			)?;

			Self::feeless_register_partner(who, metadata)
		}

		#[pallet::weight(0)]
		#[transactional]
		pub fn set_referral(origin: OriginFor<T>, partner: PartnerId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Referral::<T>::mutate(&who, |referral_info| {
				*referral_info = Some(ReferralInfo {
					partner_id: partner,
					expiry: T::BlockNumberProvider::current_block_number().saturating_add(T::ReferralExpire::get()),
				});
			});

			Self::deposit_event(Event::ReferralSet {
				who,
				partner_id: partner,
			});
			Ok(())
		}

		#[pallet::weight(0)]
		#[transactional]
		pub fn admin_register_partner(
			origin: OriginFor<T>,
			owner: T::AccountId,
			metadata: BoundedVec<u8, T::MaxMetadataLength>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Self::feeless_register_partner(owner, metadata)
		}

		#[pallet::weight(0)]
		#[transactional]
		pub fn update_partner(
			origin: OriginFor<T>,
			partner: PartnerId,
			metadata: BoundedVec<u8, T::MaxMetadataLength>,
		) -> DispatchResult {
			T::AdminOrigin::ensure_origin(origin)?;
			Partners::<T>::insert(&partner, metadata);

			Self::deposit_event(Event::PartnerMetadataUpdated { partner_id: partner });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn feeless_register_partner(owner: T::AccountId, metadata: BoundedVec<u8, T::MaxMetadataLength>) -> DispatchResult {
		let id = NextId::<T>::get();
		// generate sub account, this shouldn't fail
		let sub_account: T::AccountId = T::PalletId::get()
			.try_into_sub_account(id)
			.ok_or(Error::<T>::SubAccountGenerationFailed)?;

		// increment NextId and add partner metadata to storage
		NextId::<T>::put(id.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?);
		Partners::<T>::insert(id, metadata);

		// make caller full proxy (default is the most permissioned value) to the now registered sub account
		pallet_proxy::Pallet::<T>::add_proxy_delegate(
			&owner,
			sub_account.clone(),
			T::ProxyType::default(),
			Zero::zero(),
		)?;

		Self::deposit_event(Event::PartnerRegistered {
			owner,
			partner_id: id,
			partner_account: sub_account,
		});
		Ok(())
	}
}

impl<T: Config> OnFeeDeposited<T::AccountId, Balance, CurrencyId> for Pallet<T> {
	fn on_fee_deposited(who: &T::AccountId, currency_id: CurrencyId, amount: Balance) {
		let treasury_account = T::TreasuryAccount::get();

		if let Some(referral) = Referral::<T>::get(who) {
			if T::BlockNumberProvider::current_block_number() < referral.expiry {
				// transfer funds to sub account
				let sub_account: T::AccountId = T::PalletId::get()
					.try_into_sub_account(referral.partner_id)
					.unwrap_or(treasury_account);
				_ = <T as module::Config>::Currencies::transfer(currency_id, who, &sub_account, amount);
			} else {
				// If referral is expired transfer funds to treasury
				_ = <T as module::Config>::Currencies::transfer(currency_id, who, &treasury_account, amount);
			}
		} else {
			// If no referral exists transfer funds to treasury
			_ = <T as module::Config>::Currencies::transfer(currency_id, who, &treasury_account, amount);
		}
	}
}
