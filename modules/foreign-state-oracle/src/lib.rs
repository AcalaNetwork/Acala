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
	require_transactional,
	traits::{Currency, ExistenceRequirement},
	transactional, PalletId,
};
use frame_system::pallet_prelude::*;
use module_support::ForeignChainStateQuery;
use primitives::Balance;
use sp_runtime::{
	traits::{AccountIdConversion, BlockNumberProvider, One, Saturating},
	ArithmeticError, Permill,
};
use sp_std::{ops::Mul, prelude::Vec};

mod mock;
mod tests;

pub use module::*;

// Unique Identifier for each query
pub type QueryIndex = u64;

// Origin with arbitrary bytes included
#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub struct RawOrigin {
	data: Vec<u8>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct RelayQueryRequest<Call, BlockNumber, Balance> {
	// Call to be dispatched by oracle
	dispatchable_call: Call,
	// Blocknumber at which call will be expired
	expiry: BlockNumber,
	// Reward available for responding to this query
	oracle_reward: Balance,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type RelayQueryRequestOf<T> =
		RelayQueryRequest<<T as Config>::DispatchableCall, <T as frame_system::Config>::BlockNumber, Balance>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The outer origin type.
		type Origin: From<RawOrigin>;

		/// Currency for query payments
		type Currency: Currency<Self::AccountId, Balance = Balance>;

		/// The foreign state oracle module id, keeps expired queries deposits
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Fee to be paid to oracles for servicing query
		#[pallet::constant]
		type QueryFee: Get<Balance>;

		/// Fee for cancelling query
		#[pallet::constant]
		type CancelFee: Get<Balance>;

		/// Timeout for query requests
		#[pallet::constant]
		type DefaultQueryDuration: Get<Self::BlockNumber>;

		#[pallet::constant]
		type ExpiredCallPurgeReward: Get<Permill>;

		/// Dispatchable task that needs to be verified by oracle for dispatch
		type DispatchableCall: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		/// Origin that can dispatch calls that have been verified with foreign state
		type OracleOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;

		/// Provides current blocknumber
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	pub type Origin = RawOrigin;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Active Query is created, under the index as the key
		CreateQueryRequests { index: QueryIndex, expiry: T::BlockNumber },
		/// Call is dispatched with data, includes the result of the extrinsic
		CallDispatched { task_result: DispatchResult },
		/// Query that has expired is removed from storage, includes index
		StaleQueryRemoved { next_query_id: QueryIndex },
		/// Query is canceled, includes index
		QueryCanceled { index: QueryIndex },
	}

	/// Index of Queries, each query gets unique number
	#[pallet::storage]
	#[pallet::getter(fn next_query_id)]
	pub(super) type NextQueryId<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	/// The tasks to be dispatched with data provideed by foreign state oracle
	///
	/// QueryRequests: map QueryIndex => Option<RelayQueryRequestOF<T>>
	#[pallet::storage]
	#[pallet::getter(fn query_requests)]
	pub(super) type QueryRequests<T: Config> = StorageMap<_, Identity, QueryIndex, RelayQueryRequestOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Index key does not have an active query currently
		NoMatchingCall,
		/// Request query is too large
		TooLargeRelayQueryRequest,
		/// Query has expired
		QueryExpired,
		/// Query has not yet expired
		QueryNotExpired,
		/// Not account that requested query
		NotQueryAccount,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dispatch task with arbitrary data in origin.
		///
		/// - `next_query_id`: Unique index mapped to a particular query
		/// - `data`: Aribtrary data to be injected into the call via origin
		#[pallet::weight(0)]
		#[transactional]
		pub fn respond_query_request(origin: OriginFor<T>, next_query_id: QueryIndex, data: Vec<u8>) -> DispatchResult {
			T::OracleOrigin::ensure_origin(origin)?;

			let verifiable_call = QueryRequests::<T>::take(next_query_id).ok_or(Error::<T>::NoMatchingCall)?;
			// Check that query has not expired
			ensure!(
				verifiable_call.expiry > T::BlockNumberProvider::current_block_number(),
				Error::<T>::QueryExpired
			);
			let result = verifiable_call.dispatchable_call.dispatch(RawOrigin { data }.into());

			Self::deposit_event(Event::CallDispatched {
				task_result: result.map(|_| ()).map_err(|e| e.error),
			});
			Ok(())
		}

		/// Remove Query that has expired so chain state does not bloat. This rewards the oracle
		/// with half the query fee
		///
		/// - `next_query_id`: Unique index that is mapped to a particular query
		#[pallet::weight(0)]
		#[transactional]
		pub fn purge_expired_query(origin: OriginFor<T>, next_query_id: QueryIndex) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let verifiable_call = QueryRequests::<T>::take(next_query_id).ok_or(Error::<T>::NoMatchingCall)?;
			// Make sure query is expired
			ensure!(
				verifiable_call.expiry <= T::BlockNumberProvider::current_block_number(),
				Error::<T>::QueryNotExpired
			);

			// Gives half of reward for clearing expired query from storage
			let reward: Balance = T::ExpiredCallPurgeReward::get().mul(verifiable_call.oracle_reward);
			T::Currency::transfer(&Self::account_id(), &who, reward, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::<T>::StaleQueryRemoved { next_query_id });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	// Returns pallet account
	pub fn account_id() -> T::AccountId {
		T::PalletId::get().into_account()
	}
}

impl<T: Config> ForeignChainStateQuery<T::AccountId, T::DispatchableCall, T::BlockNumber> for Pallet<T> {
	#[transactional]
	fn query_task(
		who: &T::AccountId,
		length_bound: usize,
		dispatchable_call: T::DispatchableCall,
		query_duration: Option<T::BlockNumber>,
	) -> DispatchResult {
		let call_len = dispatchable_call.encoded_size();
		ensure!(call_len <= length_bound, Error::<T>::TooLargeRelayQueryRequest);
		T::Currency::transfer(
			who,
			&Self::account_id(),
			T::QueryFee::get(),
			ExistenceRequirement::KeepAlive,
		)?;
		let duration = query_duration.unwrap_or_else(T::DefaultQueryDuration::get);
		let expiry = T::BlockNumberProvider::current_block_number().saturating_add(duration);
		let verifiable_call = RelayQueryRequest {
			dispatchable_call,
			expiry,
			oracle_reward: T::QueryFee::get(),
		};

		let index = NextQueryId::<T>::get();
		// Increment counter by one
		NextQueryId::<T>::put(index.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?);

		QueryRequests::<T>::insert(index, verifiable_call);
		Self::deposit_event(Event::CreateQueryRequests { index, expiry });
		Ok(())
	}

	#[transactional]
	fn cancel_task(who: &T::AccountId, index: QueryIndex) -> DispatchResult {
		let task = QueryRequests::<T>::take(index).ok_or(Error::<T>::NoMatchingCall)?;

		// Reimbursts (query fee - cancel fee) to account.
		T::Currency::transfer(
			&Self::account_id(),
			who,
			task.oracle_reward.saturating_sub(T::CancelFee::get()),
			ExistenceRequirement::AllowDeath,
		)?;
		Self::deposit_event(Event::QueryCanceled { index });
		Ok(())
	}
}

#[cfg(feature = "runtime-benchmarks")]
use frame_benchmarking::vec;

pub struct EnsureForeignStateOracle;

impl<O: Into<Result<RawOrigin, O>> + From<RawOrigin>> EnsureOrigin<O> for EnsureForeignStateOracle {
	type Success = Vec<u8>;

	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().map(|o| {
			let RawOrigin { data } = o;
			data
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		O::from(RawOrigin { data: vec![] })
	}
}
