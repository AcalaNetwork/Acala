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

//! Relaychain Oracle.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	dispatch::{Dispatchable, GetDispatchInfo, PostDispatchInfo},
	pallet_prelude::*,
	traits::{SortedMembers, Time},
};
use frame_system::pallet_prelude::*;
use primitives::Balance;
use sp_runtime::{
	traits::{BlockNumberProvider, Hash, One},
	ArithmeticError,
};

mod mock;
mod tests;

pub use module::*;

/// The unique identifier for a query
pub type QueryIndex = u64;

/// Type used in gilts pallet for indexing
pub type ActiveIndex = u32;

pub type OracleCount = u32;

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum QueryResult<AccountId, Balance, BlockNumber> {
	Thaw {
		index: ActiveIndex,
	},
	Bid {
		who: AccountId,
		duration: u32,
		amount: Balance,
		index: ActiveIndex,
		expiry: BlockNumber,
	},
	Retract {
		who: AccountId,
		duration: u32,
		amount: Balance,
	},
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct QueryState<AccountId, Balance, BlockNumber> {
	timeout: BlockNumber,
	oracle_response: QueryResult<AccountId, Balance, BlockNumber>,
	votes: OracleVotes<AccountId>,
}

impl<AccountId, Balance, BlockNumber> QueryState<AccountId, Balance, BlockNumber> {
	fn new_query(
		timeout: BlockNumber,
		oracle_response: QueryResult<AccountId, Balance, BlockNumber>,
		votes: OracleVotes<AccountId>,
	) -> Self {
		Self {
			timeout,
			oracle_response,
			votes,
		}
	}
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OracleVotes<AccountId> {
	yes: Vec<AccountId>,
	no: Vec<AccountId>,
}

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum RawOrigin {
	Members(OracleCount, OracleCount),
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	// export for ease of use
	pub type QueryStateOf<T> =
		QueryState<<T as frame_system::Config>::AccountId, Balance, <T as frame_system::Config>::BlockNumber>;

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_collective::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Time: Time;

		type Members: SortedMembers<Self::AccountId>;

		type MaxMembers: Get<OracleCount>;

		type Origin: From<RawOrigin>;

		type Callback: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ From<frame_system::Call<Self>>
			+ GetDispatchInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CreateActiveQuery { index: QueryIndex },
	}

	#[pallet::storage]
	#[pallet::getter(fn query_index)]
	pub type QueryCounter<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn query)]
	pub type ActiveQueries<T: Config> = StorageMap<_, Identity, QueryIndex, QueryStateOf<T>, OptionQuery>;

	#[pallet::storage]
	pub type Callback<T: Config> = StorageMap<_, Identity, QueryIndex, T::Callback, OptionQuery>;

	#[pallet::origin]
	pub type Origin = RawOrigin;

	#[pallet::error]
	pub enum Error<T> {
		NotMember,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn create_feed(
			origin: OriginFor<T>,
			query_result: QueryResult<T::AccountId, Balance, T::BlockNumber>,
			timeout: T::BlockNumber,
			callback: Box<T::Callback>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(T::Members::contains(&who), Error::<T>::NotMember);

			let query_index = QueryCounter::<T>::get();
			let increment = query_index.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			QueryCounter::<T>::put(increment);

			if T::Members::count() == 1 {
				let result = *callback.dispatch(RawOrigin::Members(One::one(), One::one()));
			} else {
				let pending_result = QueryState::new_query(
					timeout,
					query_result,
					OracleVotes {
						yes: vec![who],
						no: vec![],
					},
				);
				ActiveQueries::<T>::insert(query_index, pending_result);
				Callback::<T>::insert(query_index, *callback);
			}

			Self::deposit_event(Event::CreateActiveQuery { index: query_index });
			Ok(())
		}

		#[pallet::weight(0)]
		pub fn verify_feed(origin: OriginFor<T>, query_index: QueryIndex, verify: bool) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(T::Members::contains(&who), Error::<T>::NotMember);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
