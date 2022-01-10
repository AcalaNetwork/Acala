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

use frame_support::{
	dispatch::{CallMetadata, GetCallMetadata},
	pallet_prelude::*,
	traits::{Contains, PalletInfoAccess},
	transactional,
};
use frame_system::pallet_prelude::*;
use primitives::BlockNumber;
use sp_runtime::DispatchResult;
use sp_std::{prelude::*, vec::Vec};

use cumulus_primitives_core::relay_chain::v1::Id;
use cumulus_primitives_core::{DmpMessageHandler, XcmpMessageHandler};
/// Block number type used by the relay chain.
pub use polkadot_core_primitives::BlockNumber as RelayChainBlockNumber;

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
		/// Paused Xcm message
		XcmPaused,
		/// Resumed Xcm message
		XcmResumed,
	}

	/// The paused transaction map
	///
	/// map (PalletNameBytes, FunctionNameBytes) => Option<()>
	#[pallet::storage]
	#[pallet::getter(fn paused_transactions)]
	pub type PausedTransactions<T: Config> = StorageMap<_, Twox64Concat, (Vec<u8>, Vec<u8>), (), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn xcm_paused)]
	pub type XcmPaused<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::pallet]
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

		#[pallet::weight(T::WeightInfo::pause_xcm())]
		pub fn pause_xcm(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if !XcmPaused::<T>::get() {
				XcmPaused::<T>::set(true);
				Self::deposit_event(Event::XcmPaused);
			}
			Ok(())
		}

		#[pallet::weight(T::WeightInfo::resume_xcm())]
		pub fn resume_xcm(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if XcmPaused::<T>::get() {
				XcmPaused::<T>::set(false);
				Self::deposit_event(Event::XcmResumed);
			}
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

/// Dmp and Xcmp message handler
pub struct XcmMessageHandler<T, H>(PhantomData<(T, H)>);

/// XcmMessageHandler implements `DmpMessageHandler`. if xcm paused, the `max_weight` is set to `0`.
///
/// Parameters type:
/// - `H`: `DmpMessageHandler`
impl<T: Config, H> DmpMessageHandler for XcmMessageHandler<T, H>
where
	H: DmpMessageHandler,
{
	fn handle_dmp_messages(iter: impl Iterator<Item = (RelayChainBlockNumber, Vec<u8>)>, max_weight: Weight) -> Weight {
		let xcm_paused: bool = Pallet::<T>::xcm_paused();
		if !xcm_paused {
			H::handle_dmp_messages(iter, max_weight)
		} else {
			H::handle_dmp_messages(iter, 0)
		}
	}
}

/// XcmMessageHandler implements `XcmpMessageHandler`. if xcm paused, the `max_weight` is set to
/// `0`.
///
/// Parameters type:
/// - `H`: `XcmpMessageHandler`
impl<T: Config, H> XcmpMessageHandler for XcmMessageHandler<T, H>
where
	H: XcmpMessageHandler,
{
	fn handle_xcmp_messages<'a, I: Iterator<Item = (Id, BlockNumber, &'a [u8])>>(
		iter: I,
		max_weight: Weight,
	) -> Weight {
		let xcm_paused: bool = Pallet::<T>::xcm_paused();
		if !xcm_paused {
			H::handle_xcmp_messages(iter, max_weight)
		} else {
			H::handle_xcmp_messages(iter, 0)
		}
	}
}
