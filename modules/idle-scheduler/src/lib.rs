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

//! # Example Module
//!
//! A simple example of a FRAME pallet demonstrating
//! concepts, APIs and structures common to most FRAME runtimes.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(unused_must_use)]
use acala_primitives::{
	task::{DispatchableTask, IdelScheduler},
	Nonce,
};
use codec::{Codec, EncodeLike};
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_runtime::traits::{One, Zero};
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
		type Task: DispatchableTask + Codec + EncodeLike + Debug + Clone + PartialEq + TypeInfo;

		/// The minimum weight that should remain before idle tasks are dispatched.
		#[pallet::constant]
		type MinimumWeightRemainInBlock: Get<Weight>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// A task has been dispatched on_idle.
		/// \[TaskId\]
		TaskDispatched(Nonce),
	}

	/// Some documentation
	#[pallet::storage]
	#[pallet::getter(fn tasks)]
	pub type Tasks<T: Config> = StorageMap<_, Twox64Concat, Nonce, T::Task, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn next_task_id)]
	pub type NextTaskId<T: Config> = StorageValue<_, Nonce, ValueQuery>;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_idle(_n: T::BlockNumber, remaining_weight: Weight) -> Weight {
			Self::do_dispatch_tasks(remaining_weight)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(< T as Config >::WeightInfo::schedule_task())]
		pub fn schedule_task(origin: OriginFor<T>, task: T::Task) -> DispatchResult {
			ensure_root(origin)?;
			Self::do_schedule_task(task);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Add the task to the queue to be dispatched later
	fn do_schedule_task(task: T::Task) {
		let id = Self::get_next_task_id();
		Tasks::<T>::insert(id, task);
	}

	/// Retrieves the next task ID from storage, and increment it by one.
	fn get_next_task_id() -> Nonce {
		NextTaskId::<T>::mutate(|current| {
			let id = *current;
			*current = current.saturating_add(One::one());
			id
		})
	}

	/// Keep dispatching tasks in Storage, until insufficient weight remains.
	pub fn do_dispatch_tasks(total_weight: Weight) -> Weight {
		let mut weight_remaining = total_weight;
		if weight_remaining <= T::MinimumWeightRemainInBlock::get() {
			return Zero::zero();
		}

		let mut completed_tasks: Vec<Nonce> = vec![];

		for (id, task) in Tasks::<T>::iter() {
			let result = task.dispatch(weight_remaining);
			weight_remaining = weight_remaining.saturating_sub(result.used_weight);
			if result.finished {
				completed_tasks.push(id);
			}

			// If remaining weight falls below the minimmum, break from the loop.
			if weight_remaining <= T::MinimumWeightRemainInBlock::get() {
				break;
			}
		}

		// Deposit event and remove completed tasks.
		for id in completed_tasks {
			Self::deposit_event(Event::<T>::TaskDispatched(id));
			Tasks::<T>::remove(id);
		}

		total_weight.saturating_sub(weight_remaining)
	}
}

impl<T: Config> IdelScheduler<T::Task> for Pallet<T> {
	fn schedule(task: T::Task) {
		Self::do_schedule_task(task);
	}
}
