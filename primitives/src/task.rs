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

/// Contains Task related primitive types and definition. Used for Scheduled tasks during idle
/// blocks.
// in promitives/task.rs
use frame_support::weights::Weight;

#[allow(dead_code)]
pub struct TaskResult {
	pub used_weight: Weight,
	pub finished: bool,
}

pub trait DispatchableTask {
	fn dispatch(self, weight: Weight) -> TaskResult;
}

pub trait IdelScheduler<Task> {
	fn schedule(task: Task);
}

/// Produce a new combined Enum that wraps the individual enum and their subcomponents.
/// Each enum must derive Encode and Decode.
#[macro_export]
macro_rules! define_combined_task {
	(
		pub enum $combined_name:ident {
			$($task:ident), *$(,)?
		}
	) => {
		#[derive(Clone, Debug, PartialEq, Encode, Decode)]
		pub enum $combined_name {
			$(
				$task($task),
			)*
		}

		impl DispatchableTask for $combined_name {
			fn dispatch(self, weight: Weight) -> TaskResult {
				match self {
					$(
						$combined_name::$task(t) => t.dispatch(weight),
					)*
				}
			}
		}

        $(
            impl From<$task> for $combined_name {
                fn from(t: $task) -> Self{
                    $combined_name::$task(t)
                }
            }
        )*
	};
}
