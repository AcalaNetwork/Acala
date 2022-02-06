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
};
use frame_system::pallet_prelude::*;
use module_support::ForeignChainStateQuery;
use sp_runtime::traits::Hash;

mod mock;
mod tests;

pub use module::*;

#[derive(PartialEq, Eq, Clone, RuntimeDebug, Encode, Decode, TypeInfo)]
pub enum RawOrigin {
	RelaychainOracle,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct VerifiableCall<Call, Hash> {
	dispatchable_call: Call,
	verify_state: Hash,
}

#[frame_support::pallet]
pub mod module {
	use super::*;

	pub type VerifiableCallOf<T> = VerifiableCall<<T as Config>::VerifiableTask, <T as frame_system::Config>::Hash>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// The outer origin type.
		type Origin: From<RawOrigin>;

		/// Dispatchable task that needs to be verified by oracle for dispatch
		type VerifiableTask: Parameter
			+ Dispatchable<Origin = <Self as Config>::Origin, PostInfo = PostDispatchInfo>
			+ From<frame_system::Call<Self>>
			+ GetDispatchInfo;

		/// Origin that can dispatch calls that have been verified with foreign state
		type OracleOrigin: EnsureOrigin<<Self as frame_system::Config>::Origin>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::origin]
	pub type Origin = RawOrigin;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CreateActiveQuery { query_hash: T::Hash },
		CallDispatched { task_result: PostDispatchInfo },
		CallDispatchFailed,
	}

	// Tasks to be dispatched if foriegn chain state is valid
	#[pallet::storage]
	#[pallet::getter(fn query)]
	pub type ValidateTask<T: Config> = StorageMap<_, Blake2_128Concat, T::Hash, VerifiableCallOf<T>, OptionQuery>;

	#[pallet::error]
	pub enum Error<T> {
		IncorrectStateHash,
		NoMatchingCall,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn dispatch_task(origin: OriginFor<T>, call_hash: T::Hash, state_hash: T::Hash) -> DispatchResult {
			ValidateTask::<T>::try_mutate_exists(call_hash, |maybe_verifiable_call| -> DispatchResult {
				T::OracleOrigin::ensure_origin(origin)?;

				let verifiable_call = maybe_verifiable_call.clone().ok_or(Error::<T>::NoMatchingCall)?;
				ensure!(
					state_hash == verifiable_call.verify_state,
					Error::<T>::IncorrectStateHash
				);

				let result = verifiable_call
					.dispatchable_call
					.dispatch(RawOrigin::RelaychainOracle.into());
				*maybe_verifiable_call = None;
				match result {
					Ok(res) => Self::deposit_event(Event::CallDispatched { task_result: res }),
					Err(_) => Self::deposit_event(Event::CallDispatchFailed),
				}
				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {}

impl<T: Config> ForeignChainStateQuery<T::VerifiableTask, T::Hash> for Pallet<T> {
	fn query_task(call: T::VerifiableTask, state_hash: T::Hash) {
		let call_hash = T::Hashing::hash_of(&call);
		let verifiable_call = VerifiableCall {
			dispatchable_call: call,
			verify_state: state_hash,
		};

		ValidateTask::<T>::insert(call_hash, verifiable_call);
		Self::deposit_event(Event::CreateActiveQuery { query_hash: call_hash });
	}
}
