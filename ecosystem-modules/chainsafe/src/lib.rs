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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{pallet_prelude::*, transactional};
use frame_system::pallet_prelude::*;
use orml_traits::{GetByKey, MultiCurrency};
use primitives::{Balance, CurrencyId};
use sp_runtime::SaturatedConversion;
use sp_std::vec::Vec;

pub use module::*;

#[frame_support::pallet]
pub mod module {
	use super::*;

	type ResourceId = chainbridge::ResourceId;

	#[pallet::config]
	pub trait Config: frame_system::Config + chainbridge::Config {
		#[pallet::constant]
		/// Ids can be defined by the runtime and passed in, perhaps from
		/// blake2b_128 hashes.
		type HashId: Get<ResourceId>;

		type ResourceIds: GetByKey<CurrencyId, ResourceId>;

		type Currency: MultiCurrency<Self::AccountId, CurrencyId = CurrencyId, Balance = Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		InvalidDestChainId,
	}

	#[pallet::pallet]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(1_000_000)]
		#[transactional]
		pub fn transfer_tokens(
			origin: OriginFor<T>,
			dest_chain_id: chainbridge::ChainId,
			recipient: Vec<u8>,
			currency_id: CurrencyId,
			amount: Balance,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(
				chainbridge::Module::<T>::chain_whitelisted(dest_chain_id),
				Error::<T>::InvalidDestChainId
			);

			let bridge_account_id = chainbridge::Module::<T>::account_id();
			let resource_id = T::ResourceIds::get(&currency_id);
			T::Currency::transfer(currency_id, &who, &bridge_account_id, amount.into())?;
			chainbridge::Module::<T>::transfer_fungible(
				dest_chain_id,
				resource_id,
				recipient,
				sp_core::U256::from(amount.saturated_into::<u128>()),
			)?;

			Ok(().into())
		}

		#[pallet::weight(1_000_000)]
		#[transactional]
		pub fn transfer_hash(
			origin: OriginFor<T>,
			dest_chain_id: chainbridge::ChainId,
			hash: T::Hash,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			let resource_id = T::HashId::get();
			let metadata: Vec<u8> = hash.as_ref().to_vec();
			chainbridge::Module::<T>::transfer_generic(dest_chain_id, resource_id, metadata)?;

			Ok(().into())
		}
	}
}
