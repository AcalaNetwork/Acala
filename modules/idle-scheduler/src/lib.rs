// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
pub use module_support::{DispatchableTask, IdleScheduler};
use parity_scale_codec::FullCodec;
use primitives::{task::TaskResult, BlockNumber};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{BlockNumberProvider, CheckedAdd, One},
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
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;

		/// The index of tasks.
		type Index: Parameter + Default + One + CheckedAdd + Copy + TypeInfo;

		/// Dispatchable tasks.
		type Task: DispatchableTask + FullCodec + Debug + Clone + PartialEq + TypeInfo;

		/// The minimum weight that should remain before idle tasks are dispatched.
		#[pallet::constant]
		type MinimumWeightRemainInBlock: Get<Weight>;

		/// Gets RelayChain Block Number
		type RelayChainBlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumber>;

		/// Number of Relay Chain blocks skipped to disable `on_idle` dispatching scheduled tasks,
		/// this shuts down idle-scheduler when block production is slower than this number of
		/// relaychain blocks.
		#[pallet::constant]
		type DisableBlockThreshold: Get<BlockNumber>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// A task has been dispatched on_idle.
		TaskDispatched { task_id: T::Index, result: DispatchResult },
		/// A task is added.
		TaskAdded { task_id: T::Index, task: T::Task },
	}

	/// The schedule tasks waiting to dispatch. After task is dispatched, it's removed.
	///
	/// Tasks: map T::Index => Task
	#[pallet::storage]
	#[pallet::getter(fn tasks)]
	pub type Tasks<T: Config> = StorageMap<_, Twox64Concat, T::Index, T::Task, OptionQuery>;

	/// The task id used to index tasks.
	#[pallet::storage]
	#[pallet::getter(fn next_task_id)]
	pub type NextTaskId<T: Config> = StorageValue<_, T::Index, ValueQuery>;

	/// A temporary variable used to check if should skip dispatch schedule task or not.
	#[pallet::storage]
	#[pallet::getter(fn previous_relay_block)]
	pub type PreviousRelayBlockNumber<T: Config> = StorageValue<_, BlockNumber, ValueQuery>;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			// This is the previous relay block because `on_initialize` is executed
			// before the inherent that sets the new relay chain block number
			let previous_relay_block: BlockNumber = T::RelayChainBlockNumberProvider::current_block_number();

			PreviousRelayBlockNumber::<T>::put(previous_relay_block);
			T::WeightInfo::on_initialize()
		}

		fn on_idle(_n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			// Checks if we have skipped enough relay blocks without block production to skip dispatching
			// scheduled tasks
			let current_relay_block_number: BlockNumber = T::RelayChainBlockNumberProvider::current_block_number();
			let previous_relay_block_number = PreviousRelayBlockNumber::<T>::take();
			if current_relay_block_number.saturating_sub(previous_relay_block_number) >= T::DisableBlockThreshold::get()
			{
				log::debug!(
					target: "idle-scheduler",
					"Relaychain produced blocks without finalizing parachain blocks. Idle-scheduler will not execute.\ncurrent relay block number: {:?}\nprevious relay block number: {:?}",
					current_relay_block_number,
					previous_relay_block_number
				);
				// something is not correct so exhaust all remaining weight (note: any on_idle hooks after
				// IdleScheduler won't execute)
				remaining_weight
			} else {
				Self::do_dispatch_tasks(remaining_weight)
			}
		}

		fn on_finalize(_n: BlockNumberFor<T>) {
			// Don't commit to storage, needed for the case block is full and `on_idle` isn't called
			PreviousRelayBlockNumber::<T>::kill();
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(< T as Config >::WeightInfo::schedule_task())]
		pub fn schedule_task(origin: OriginFor<T>, task: T::Task) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_schedule_task(task).map(|_| ())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Add the task to the queue to be dispatched later.
	fn do_schedule_task(task: T::Task) -> Result<T::Index, DispatchError> {
		let id = Self::get_next_task_id()?;
		Tasks::<T>::insert(id, &task);
		Self::deposit_event(Event::<T>::TaskAdded { task_id: id, task });
		Ok(id)
	}

	/// Retrieves the next task ID from storage, and increment it by one.
	fn get_next_task_id() -> Result<T::Index, DispatchError> {
		NextTaskId::<T>::mutate(|current| -> Result<T::Index, DispatchError> {
			let id = *current;
			*current = current.checked_add(&One::one()).ok_or(ArithmeticError::Overflow)?;
			Ok(id)
		})
	}

	/// Keep dispatching tasks in Storage, until insufficient weight remains.
	pub fn do_dispatch_tasks(total_weight: Weight) -> Weight {
		let mut weight_remaining = total_weight.saturating_sub(T::WeightInfo::on_idle_base());
		if weight_remaining.ref_time() <= T::MinimumWeightRemainInBlock::get().ref_time() {
			// return total weight so no `on_idle` hook will execute after IdleScheduler
			return total_weight;
		}

		let mut completed_tasks: Vec<(T::Index, TaskResult)> = vec![];

		for (id, task) in Tasks::<T>::iter() {
			let result = task.dispatch(weight_remaining);
			weight_remaining = weight_remaining.saturating_sub(result.used_weight);
			if result.finished {
				completed_tasks.push((id, result));
				weight_remaining = weight_remaining.saturating_sub(T::WeightInfo::clear_tasks());
			}

			// If remaining weight falls below the minimmum, break from the loop.
			if weight_remaining.ref_time() <= T::MinimumWeightRemainInBlock::get().ref_time() {
				break;
			}
		}

		Self::remove_completed_tasks(completed_tasks);

		total_weight.saturating_sub(weight_remaining)
	}

	/// Removes completed tasks and deposits events.
	pub fn remove_completed_tasks(completed_tasks: Vec<(T::Index, TaskResult)>) {
		// Deposit event and remove completed tasks.
		for (id, result) in completed_tasks {
			Self::deposit_event(Event::<T>::TaskDispatched {
				task_id: id,
				result: result.result,
			});
			Tasks::<T>::remove(id);
		}
	}
}

impl<T: Config> IdleScheduler<T::Index, T::Task> for Pallet<T> {
	fn schedule(task: T::Task) -> Result<T::Index, DispatchError> {
		Self::do_schedule_task(task)
	}

	/// If the task can be executed under given weight limit, dispatch it.
	/// Otherwise the scheduler will keep the task and run it later.
	/// NOTE: Only used for synchronous execution case, because `T::WeightInfo::clear_tasks()` is
	/// not considered.
	fn dispatch(id: T::Index, weight_limit: Weight) -> Weight {
		if let Some(task) = Tasks::<T>::get(id) {
			let result = task.dispatch(weight_limit);
			let used_weight = result.used_weight;
			if result.finished {
				Self::remove_completed_tasks(vec![(id, result)]);
			}

			weight_limit.saturating_sub(used_weight)
		} else {
			weight_limit
		}
	}
}
