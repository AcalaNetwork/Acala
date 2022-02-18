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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	dispatch::{CallMetadata, GetCallMetadata},
	pallet_prelude::*,
	traits::{Contains, PalletInfoAccess},
	transactional,
};
use frame_system::pallet_prelude::*;
use sp_runtime::DispatchResult;
use sp_std::{prelude::*, vec::Vec};

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
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The origin which may set filter.
		type UpdateOrigin: EnsureOrigin<Self::Origin>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// can not pause
		CannotPause,
		/// invalid character encoding
		InvalidCharacter,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Paused transaction
		TransactionPaused {
			pallet_name_bytes: Vec<u8>,
			function_name_bytes: Vec<u8>,
		},
		/// Unpaused transaction
		TransactionUnpaused {
			pallet_name_bytes: Vec<u8>,
			function_name_bytes: Vec<u8>,
		},
	}

	/// The paused transaction map
	///
	/// map (PalletNameBytes, FunctionNameBytes) => Option<()>
	#[pallet::storage]
	#[pallet::getter(fn paused_transactions)]
	pub type PausedTransactions<T: Config> = StorageMap<_, Twox64Concat, (Vec<u8>, Vec<u8>), (), OptionQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(T::WeightInfo::pause_transaction())]
		#[transactional]
		pub fn pause_transaction(origin: OriginFor<T>, pallet_name: Vec<u8>, function_name: Vec<u8>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			// not allowed to pause calls of this pallet to ensure safe
			let pallet_name_string = sp_std::str::from_utf8(&pallet_name).map_err(|_| Error::<T>::InvalidCharacter)?;
			ensure!(
				pallet_name_string != <Self as PalletInfoAccess>::name(),
				Error::<T>::CannotPause
			);

			PausedTransactions::<T>::mutate_exists((pallet_name.clone(), function_name.clone()), |maybe_paused| {
				if maybe_paused.is_none() {
					*maybe_paused = Some(());
					Self::deposit_event(Event::TransactionPaused {
						pallet_name_bytes: pallet_name,
						function_name_bytes: function_name,
					});
				}
			});
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::unpause_transaction())]
		#[transactional]
		pub fn unpause_transaction(
			origin: OriginFor<T>,
			pallet_name: Vec<u8>,
			function_name: Vec<u8>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if PausedTransactions::<T>::take((&pallet_name, &function_name)).is_some() {
				Self::deposit_event(Event::TransactionUnpaused {
					pallet_name_bytes: pallet_name,
					function_name_bytes: function_name,
				});
			};
			Ok(())
		}
	}
}

pub struct PausedTransactionFilter<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> Contains<T::Call> for PausedTransactionFilter<T>
where
	<T as frame_system::Config>::Call: GetCallMetadata,
{
	fn contains(call: &T::Call) -> bool {
		let CallMetadata {
			function_name,
			pallet_name,
		} = call.get_call_metadata();
		PausedTransactions::<T>::contains_key((pallet_name.as_bytes(), function_name.as_bytes()))
	}
}
