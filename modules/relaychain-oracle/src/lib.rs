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
	dispatch::{Dispatchable, GetDispatchInfo},
	pallet_prelude::*,
	traits::{SortedMembers, Time},
};
use frame_system::pallet_prelude::*;
use primitives::Balance;
use sp_runtime::{traits::One, ArithmeticError};

mod mock;
mod tests;

pub use module::*;

/// The unique identifier for a query
pub type QueryIndex = u64;

/// Type used in gilts pallet for indexing
pub type ActiveIndex = u32;

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
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

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct QueryState<AccountId, Balance, BlockNumber> {
	timeout: BlockNumber,
	response: QueryResult<AccountId, Balance, BlockNumber>,
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

		type Call: Parameter + Dispatchable + GetDispatchInfo + From<frame_system::Call<Self>>;

		type Time: Time;

		type Members: SortedMembers<Self::AccountId>;
	}

	#[pallet::pallet]
	#[pallet::generate_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CreateActiveQuery(QueryIndex),
	}

	#[pallet::storage]
	#[pallet::getter(fn query_index)]
	pub type QueryCounter<T: Config> = StorageValue<_, QueryIndex, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn query)]
	pub type ActiveQueries<T: Config> = StorageMap<_, Blake2_128Concat, QueryIndex, QueryStateOf<T>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn create_motion(
			origin: OriginFor<T>,
			query_result: QueryStateOf<T>,
			timeout: T::BlockNumber,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			let query_index = QueryCounter::<T>::get();
			let increment = query_index.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			QueryCounter::<T>::put(increment);

			ActiveQueries::<T>::insert(query_index, query_result);

			Self::deposit_event(Event::CreateActiveQuery(query_index));
			Ok(())
		}

		#[pallet::weight(0)]
		pub fn vote_motion(origin: OriginFor<T>, query_index: QueryIndex) -> DispatchResult {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
