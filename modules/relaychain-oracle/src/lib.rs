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
	pallet_prelude::*,
	traits::{SortedMembers, Time},
};
use frame_system::pallet_prelude::*;

mod mock;
mod tests;

pub use module::*;

/// The unique identifier for a query
pub type QueryNonce = u64;

pub type QueryResult = Vec<u8>;

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct Feed<AccountId> {
	pub creator: AccountId,
	pub value: QueryResult,
}

#[derive(Clone, Eq, PartialEq, Encode, Decode, RuntimeDebug, TypeInfo)]
pub enum QueryState<BlockNumber> {
	Pending { timeout: BlockNumber },
	Ready { response: QueryResult, block: BlockNumber },
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Time: Time;

		type Members: SortedMembers<Self::AccountId>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn query_index)]
	pub type QueryIndex<T: Config> = StorageValue<_, QueryNonce, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn query)]
	pub type ActiveQueries<T: Config> =
		StorageMap<_, Blake2_128Concat, QueryNonce, QueryState<T::BlockNumber>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn feed_values(origin: OriginFor<T>) -> DispatchResult {
			Ok(())
		}

		#[pallet::weight(0)]
		pub fn create_feed(origin: OriginFor<T>, feed: Feed<T::AccountId>) -> DispatchResult {
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {}
