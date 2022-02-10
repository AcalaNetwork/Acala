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
	traits::{Currency, ExistenceRequirement, NamedReservableCurrency},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use module_support::ForeignChainStateQuery;
use primitives::{Balance, ReserveIdentifier};
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider, Saturating};

mod mock;
mod tests;

pub use module::*;

// Unique Identifier for each query
pub type QueryIndex = u64;

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum RawOrigin {
	RelaychainOracle { data: Vec<u8> },
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct VerifiableCall<Call, BlockNumber, Balance> {
	dispatchable_call: Call,
	expiry: BlockNumber,
	oracle_reward: Balance,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type VerifiableCallOf<T> =
		VerifiableCall<<T as Config>::VerifiableTask, <T as frame_system::Config>::BlockNumber, Balance>;

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

		/// The foreign state oracle module id, keeps expired queries deposits
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Fee to be paid to oracles for servicing query
		#[pallet::constant]
		type QueryFee: Get<Balance>;

		/// Timeout for query requests
		#[pallet::constant]
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

	/// Index of Queries, each query gets unique number
	#[pallet::storage]
	#[pallet::getter(fn query_index)]
	pub(super) type QueryCounter<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	///  The tasks to be dispatched with foriegn chain state
	#[pallet::storage]
	#[pallet::getter(fn active_query)]
	pub(super) type ActiveQuery<T: Config> = StorageMap<_, Identity, QueryIndex, VerifiableCallOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Index key does not have an active query currently
		NoMatchingCall,
		/// Verifiable Call is too large
		TooLargeVerifiableCall,
		/// Query has expired
		QueryExpired,
		/// Query has not yet expired
		QueryNotExpired,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		#[transactional]
		pub fn dispatch_task(origin: OriginFor<T>, query_index: QueryIndex, data: Vec<u8>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			let verifiable_call = ActiveQuery::<T>::take(query_index).ok_or(Error::<T>::NoMatchingCall)?;
			// check that query has not expired
			ensure!(
				verifiable_call.expiry > frame_system::Pallet::<T>::current_block_number(),
				Error::<T>::QueryExpired
			);

			let result = verifiable_call
				.dispatchable_call
				.dispatch(RawOrigin::RelaychainOracle { data }.into());

			Self::deposit_event(Event::CallDispatched {
				task_result: result.map(|_| ()).map_err(|e| e.error),
			});

			Ok(())
		}

		// TODO: Change to use idle scheduler
		#[pallet::weight(0)]
		#[transactional]
		pub fn remove_expired_call(origin: OriginFor<T>, query_index: QueryIndex) -> DispatchResult {
			ensure_none(origin)?;

			let verifiable_call = ActiveQuery::<T>::take(query_index).ok_or(Error::<T>::NoMatchingCall)?;
			// make sure query is expired
			ensure!(
				verifiable_call.expiry <= frame_system::Pallet::<T>::current_block_number(),
				Error::<T>::QueryNotExpired
			);

			Self::deposit_event(Event::<T>::StaleQueryRemoved { query_index });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}
}

impl<T: Config> ForeignChainStateQuery<T::AccountId, T::VerifiableTask> for Pallet<T> {
	fn query_task(who: &T::AccountId, length_bound: u32, dispatchable_call: T::VerifiableTask) -> DispatchResult {
		let call_len = dispatchable_call.using_encoded(|x| x.len());
		ensure!(call_len <= length_bound as usize, Error::<T>::TooLargeVerifiableCall);
		T::Currency::transfer(
			who,
			&Self::account_id(),
			T::QueryFee::get(),
			ExistenceRequirement::KeepAlive,
		)?;

		let expiry = frame_system::Pallet::<T>::current_block_number().saturating_add(T::QueryDuration::get());
		let verifiable_call = VerifiableCall {
			dispatchable_call,
			expiry,
			oracle_reward: T::QueryFee::get(),
		};

		let index = QueryCounter::<T>::get();
		// Increment counter
		QueryCounter::<T>::put(index + 1);

		ActiveQuery::<T>::insert(index, verifiable_call);
		Self::deposit_event(Event::CreateActiveQuery { index });

		Ok(())
	}
}
