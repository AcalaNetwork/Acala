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

//! Foreign State Oracle.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	pallet_prelude::*,
	traits::NamedReservableCurrency,
	transactional,
};
use frame_system::pallet_prelude::*;
use module_support::ForeignChainStateQuery;
use primitives::{Balance, ReserveIdentifier};
use sp_runtime::traits::{BlockNumberProvider, Saturating};

mod mock;
mod tests;

pub use module::*;

// Unique Identifier for each query
pub type QueryIndex = u64;
pub const RESERVE_ID_QUERY_DEPOSIT: ReserveIdentifier = ReserveIdentifier::ForeignStateQueryDeposit;

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum RawOrigin {
	RelaychainOracle { data: Vec<u8> },
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct VerifiableCall<Call, BlockNumber> {
	dispatchable_call: Call,
	expiry: BlockNumber,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type VerifiableCallOf<T> =
		VerifiableCall<<T as Config>::VerifiableTask, <T as frame_system::Config>::BlockNumber>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The outer origin type.
		type Origin: From<RawOrigin>;

		/// Currency for query payments
		type Currency: NamedReservableCurrency<
			Self::AccountId,
			Balance = Balance,
			ReserveIdentifier = ReserveIdentifier,
		>;

		/// Fee to be paid to oracles for servicing query
		type QueryFee: Get<Balance>;

		/// Timeout for query requests
		type QueryDuration: Get<Self::BlockNumber>;

		/// Dispatchable task that needs to be verified by oracle for dispatch
		type VerifiableTask: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		/// Origin that can dispatch calls that have been verified with foreign state
		type OracleOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	pub type Origin = RawOrigin;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CreateActiveQuery { index: QueryIndex },
		CallDispatched { task_result: DispatchResult },
		StaleQueryRemoved { query_index: QueryIndex },
	}

	#[pallet::storage]
	#[pallet::getter(fn query_index)]
	pub type QueryCounter<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	// Tasks to be dispatched if foriegn chain state is valid
	#[pallet::storage]
	#[pallet::getter(fn active_query)]
	pub type ActiveQuery<T: Config> = StorageMap<_, Identity, QueryIndex, VerifiableCallOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		IncorrectStateHash,
		NoMatchingCall,
		TooLargeVerifiableCall,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		#[transactional]
		pub fn dispatch_task(origin: OriginFor<T>, query_index: QueryIndex, data: Vec<u8>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			let verifiable_call = ActiveQuery::<T>::take(query_index).ok_or(Error::<T>::NoMatchingCall)?;

			let result = verifiable_call
				.dispatchable_call
				.dispatch(RawOrigin::RelaychainOracle { data }.into());

			Self::deposit_event(Event::CallDispatched {
				task_result: result.map(|_| ()).map_err(|e| e.error),
			});

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn remove_expired_call(origin: OriginFor<T>, query_index: QueryIndex) -> DispatchResult {
			ensure_signed(origin)?;

			Self::deposit_event(Event::<T>::StaleQueryRemoved { query_index });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}

impl<T: Config> ForeignChainStateQuery<T::AccountId, T::VerifiableTask> for Pallet<T> {
	fn query_task(who: &T::AccountId, length_bound: u32, dispatchable_call: T::VerifiableTask) -> DispatchResult {
		let call_len = dispatchable_call.using_encoded(|x| x.len());
		ensure!(call_len <= length_bound as usize, Error::<T>::TooLargeVerifiableCall);
		T::Currency::reserve_named(&RESERVE_ID_QUERY_DEPOSIT, who, T::QueryFee::get())?;

		let expiry = frame_system::Pallet::<T>::current_block_number().saturating_add(T::QueryDuration::get());
		let verifiable_call = VerifiableCall {
			dispatchable_call,
			expiry,
		};

		let index = QueryCounter::<T>::get();
		// Increment counter
		QueryCounter::<T>::put(index + 1);

		ActiveQuery::<T>::insert(index, verifiable_call);
		Self::deposit_event(Event::CreateActiveQuery { index });

		Ok(())
	}
}
