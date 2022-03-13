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
//! This module provide interface for other internal modules to create a QueryRequest that
//! requires Oracles to verify external states (such as states on the Relay Chain).
//! Basic workflow:
//! 1. Internal module will create a QueryRequest, providing a dispatchable "callback"
//! 2. External oracles will then verify states and provide feedback
//! 3. Feedback are tallied, depending on how `OracleOrigin` is configured
//! 4. This module will dispatch the callback, injecting any response data into the Origin.

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
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

// Unique Identifier for each query
pub type QueryIndex = u64;

// Origin with arbitrary bytes included
#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub struct RawOrigin {
	data: Vec<u8>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct ForeignQueryRequest<Call, BlockNumber> {
	// Call to be dispatched by oracle
	dispatchable_call: Call,
	// Blocknumber at which call will be expired
	expiry: BlockNumber,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type ForeignQueryRequestOf<T> =
		ForeignQueryRequest<<T as Config>::DispatchableCall, <T as frame_system::Config>::BlockNumber>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The outer origin type.
		type Origin: From<RawOrigin>;

		/// Weight Info
		type WeightInfo: WeightInfo;

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

		#[pallet::constant]
		type MaxQueryCallSize: Get<u32>;

		/// Dispatchable call that needs to be verified by oracle for dispatch
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
		/// An Query request is created, under the query_id as the key
		QueryRequestCreated {
			query_id: QueryIndex,
			expiry: T::BlockNumber,
		},
		/// Call is dispatched with data
		CallDispatched {
			query_id: QueryIndex,
			task_result: DispatchResult,
		},
		/// Query that has expired is removed from storage
		StaleQueryRemoved { query_id: QueryIndex },
		/// Query is canceled
		QueryCanceled { query_id: QueryIndex },
	}

	/// Index of Queries, each query gets an unique index.
	#[pallet::storage]
	#[pallet::getter(fn next_query_id)]
	pub(super) type NextQueryId<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	/// The tasks to be dispatched with data provided by foreign state oracle
	///
	/// QueryRequests: map QueryIndex => Option<ForeignQueryRequestOF<T>>
	#[pallet::storage]
	#[pallet::getter(fn query_requests)]
	pub(super) type QueryRequests<T: Config> =
		StorageMap<_, Identity, QueryIndex, ForeignQueryRequestOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// No query request with the given index.
		NoMatchingCall,
		/// Request query is larger than `MaxQueryCallSize`.
		TooLargeForeignQueryRequest,
		/// Query has expired
		QueryExpired,
		/// Query has not yet expired
		QueryNotExpired,
		/// Query request's `DispatchableCall` weight is greater than
		/// the weight bound specified by caller
		WrongRequestWeightBound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Respond to a query request with information from the relay chain.
		/// Dispatch the callback with response data in origin.
		///
		/// - `query_id`: Unique index mapped to a particular query
		/// - `data`: Aribtrary data to be injected into the call via origin
		#[pallet::weight(T::WeightInfo::respond_query_request().saturating_add(*call_weight_bound))]
		#[transactional]
		pub fn respond_query_request(
			origin: OriginFor<T>,
			#[pallet::compact] query_id: QueryIndex,
			data: Vec<u8>,
			#[pallet::compact] call_weight_bound: Weight,
		) -> DispatchResultWithPostInfo {
			T::OracleOrigin::ensure_origin(origin)?;

			let foreign_request = QueryRequests::<T>::take(query_id).ok_or(Error::<T>::NoMatchingCall)?;
			// Check that query has not expired
			ensure!(
				foreign_request.expiry > T::BlockNumberProvider::current_block_number(),
				Error::<T>::QueryExpired
			);
			let request_weight = foreign_request.dispatchable_call.get_dispatch_info().weight;
			ensure!(request_weight <= call_weight_bound, Error::<T>::WrongRequestWeightBound);

			let result = foreign_request.dispatchable_call.dispatch(RawOrigin { data }.into());

			Self::deposit_event(Event::CallDispatched {
				query_id,
				task_result: result.map(|_| ()).map_err(|e| e.error),
			});
			Ok((Some(get_result_weight(result).unwrap_or(request_weight)), Pays::Yes).into())
		}

		/// Remove Query that has expired so chain state does not bloat. This rewards the oracle
		/// with a portion of the query fee
		///
		/// - `query_id`: Unique index that is mapped to a particular query
		#[pallet::weight(T::WeightInfo::purge_expired_query())]
		#[transactional]
		pub fn purge_expired_query(origin: OriginFor<T>, #[pallet::compact] query_id: QueryIndex) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let foreign_request = QueryRequests::<T>::take(query_id).ok_or(Error::<T>::NoMatchingCall)?;
			// Make sure query is expired
			ensure!(
				foreign_request.expiry <= T::BlockNumberProvider::current_block_number(),
				Error::<T>::QueryNotExpired
			);

			// Gives half of reward for clearing expired query from storage
			let reward: Balance = T::ExpiredCallPurgeReward::get().mul(T::QueryFee::get());
			T::Currency::transfer(&Self::account_id(), &who, reward, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::<T>::StaleQueryRemoved { query_id });
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
	#[require_transactional]
	fn create_query(
		who: &T::AccountId,
		dispatchable_call: T::DispatchableCall,
		query_duration: Option<T::BlockNumber>,
	) -> DispatchResult {
		let call_len = dispatchable_call.encoded_size();
		ensure!(
			call_len <= T::MaxQueryCallSize::get() as usize,
			Error::<T>::TooLargeForeignQueryRequest
		);
		T::Currency::transfer(
			who,
			&Self::account_id(),
			T::QueryFee::get(),
			ExistenceRequirement::KeepAlive,
		)?;
		let duration = query_duration.unwrap_or_else(T::DefaultQueryDuration::get);
		let expiry = T::BlockNumberProvider::current_block_number().saturating_add(duration);
		let foreign_request = ForeignQueryRequest {
			dispatchable_call,
			expiry,
		};

		let query_id = NextQueryId::<T>::get();
		// Increment counter by one
		NextQueryId::<T>::put(query_id.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?);

		QueryRequests::<T>::insert(query_id, foreign_request);
		Self::deposit_event(Event::QueryRequestCreated { query_id, expiry });
		Ok(())
	}

	#[require_transactional]
	fn cancel_query(who: &T::AccountId, query_id: QueryIndex) -> DispatchResult {
		QueryRequests::<T>::take(query_id).ok_or(Error::<T>::NoMatchingCall)?;

		// Reimburse (query fee - cancel fee) to account.
		T::Currency::transfer(
			&Self::account_id(),
			who,
			T::QueryFee::get().saturating_sub(T::CancelFee::get()),
			ExistenceRequirement::AllowDeath,
		)?;
		Self::deposit_event(Event::QueryCanceled { query_id });
		Ok(())
	}
}

/// Return the weight of a dispatch call result as an `Option`.
///
/// Will return the weight regardless of what the state of the result is.
fn get_result_weight(result: DispatchResultWithPostInfo) -> Option<Weight> {
	match result {
		Ok(post_info) => post_info.actual_weight,
		Err(err) => err.post_info.actual_weight,
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
