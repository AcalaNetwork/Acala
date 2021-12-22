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

//! # Idle scheduler Module
//!
//! Allow pallets and chain maintainer to schedule a task to be dispatched when chain is idle.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(unused_must_use)]
use acala_primitives::{task::TaskResult, BlockNumber, Nonce};
use codec::FullCodec;
use frame_support::log;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
pub use module_support::{DispatchableTask, IdleScheduler};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{BlockNumberProvider, One, Zero},
	ArithmeticError,
};
use sp_std::{cmp::PartialEq, fmt::Debug, prelude::*};

mod mock;
mod tests;
mod weights;
pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// Dispatchable tasks
		type Task: DispatchableTask + FullCodec + Debug + Clone + PartialEq + TypeInfo;

		/// The minimum weight that should remain before idle tasks are dispatched.
		#[pallet::constant]
		type MinimumWeightRemainInBlock: Get<Weight>;

		/// Gets RelayChain Block Number
		type RelayChainBlockNumberProvider: BlockNumberProvider;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// A task has been dispatched on_idle.
		/// \[TaskId, DispatchResult\]
		TaskDispatched(Nonce, DispatchResult),
	}

	/// Some documentation
	#[pallet::storage]
	#[pallet::getter(fn tasks)]
	pub type Tasks<T: Config> = StorageMap<_, Twox64Concat, Nonce, T::Task, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn next_task_id)]
	pub type NextTaskId<T: Config> = StorageValue<_, Nonce, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn block_difference)]
	pub type BlockNumberDifference<T: Config> = StorageValue<_, BlockNumber, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_idle(n: T::BlockNumber, remaining_weight: Weight) -> Weight {
			// shouldn't fail, even if it does, it will equal 0 making scheduler not dispatch tasks
			let relay_block_number: BlockNumber = T::RelayChainBlockNumberProvider::current_block_number()
				.try_into()
				.unwrap_or_default();
			// divide relay block by two (right shift) and then subtract by parachain block number to get
			// difference. (Relay/2 - Para)
			let diff = (relay_block_number >> 1).saturating_sub(n.try_into().unwrap_or_default());
			// if 6 relaychain blocks are produced with no parachain block finialized we will not execute
			// dispatch tasks. Note this will occasionally happen as it doesn't have to be consecutive
			if diff.saturating_sub(3) >= Self::block_difference() {
				log::debug!(
					target: "idle-scheduler",
					"Relaychain produced blocks without finalizing our parachain blocks. The idle-scheduler will not execute. relay block number: {:?} parachain block number: {:?}",
					relay_block_number,
					n
				);
				// store new differrence
				BlockNumberDifference::<T>::put(diff);
				T::DbWeight::get().reads_writes(1, 1)
			} else {
				Self::do_dispatch_tasks(remaining_weight)
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(< T as Config >::WeightInfo::schedule_task())]
		pub fn schedule_task(origin: OriginFor<T>, task: T::Task) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_schedule_task(task)
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Add the task to the queue to be dispatched later
	fn do_schedule_task(task: T::Task) -> DispatchResult {
		let id = Self::get_next_task_id()?;
		Tasks::<T>::insert(id, task);
		Ok(())
	}

	/// Retrieves the next task ID from storage, and increment it by one.
	fn get_next_task_id() -> Result<Nonce, DispatchError> {
		NextTaskId::<T>::mutate(|current| -> Result<Nonce, DispatchError> {
			let id = *current;
			*current = current.checked_add(One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	/// Keep dispatching tasks in Storage, until insufficient weight remains.
	pub fn do_dispatch_tasks(total_weight: Weight) -> Weight {
		let mut weight_remaining = total_weight;
		if weight_remaining <= T::MinimumWeightRemainInBlock::get() {
			return Zero::zero();
		}

		let mut completed_tasks: Vec<(Nonce, TaskResult)> = vec![];

		for (id, task) in Tasks::<T>::iter() {
			let result = task.dispatch(weight_remaining);
			weight_remaining = weight_remaining.saturating_sub(result.used_weight);
			if result.finished {
				completed_tasks.push((id, result));
			}

			// If remaining weight falls below the minimmum, break from the loop.
			if weight_remaining <= T::MinimumWeightRemainInBlock::get() {
				break;
			}
		}

		// Deposit event and remove completed tasks.
		for (id, result) in completed_tasks {
			Self::deposit_event(Event::<T>::TaskDispatched(id, result.result));
			Tasks::<T>::remove(id);
		}

		total_weight.saturating_sub(weight_remaining)
	}
}

impl<T: Config> IdleScheduler<T::Task> for Pallet<T> {
	fn schedule(task: T::Task) -> DispatchResult {
		Self::do_schedule_task(task)
	}
}
