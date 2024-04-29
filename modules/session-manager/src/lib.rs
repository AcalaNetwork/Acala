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

//! # Session Manager Module
//!
//! The module implement the `ShouldEndSession` and `EstimateNextSessionRotation`
//! trait to handle the change of session time.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]

use frame_support::{
	pallet_prelude::*,
	traits::{EstimateNextSessionRotation, ValidatorSet},
};
use frame_system::pallet_prelude::*;
use pallet_session::ShouldEndSession;
use sp_runtime::{
	traits::{One, Saturating, Zero},
	Permill,
};
use sp_staking::SessionIndex;

mod mock;
mod tests;
pub mod weights;

pub use module::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod module {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		/// A type for retrieving the validators supposed to be online in a session.
		type ValidatorSet: ValidatorSet<Self::AccountId, ValidatorId = Self::AccountId>;
		/// Weight information for the extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The session is invalid.
		InvalidSession,
		/// The duration is invalid.
		InvalidDuration,
		/// Failed to estimate next session.
		EstimateNextSessionFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(fn deposit_event)]
	pub enum Event<T: Config> {
		/// Scheduled session duration.
		ScheduledSessionDuration {
			block_number: BlockNumberFor<T>,
			session_index: SessionIndex,
			session_duration: BlockNumberFor<T>,
		},
	}

	/// The current session duration.
	///
	/// SessionDuration: BlockNumberFor<T>
	#[pallet::storage]
	#[pallet::getter(fn session_duration)]
	pub type SessionDuration<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	/// The current session duration offset.
	///
	/// DurationOffset: BlockNumberFor<T>
	#[pallet::storage]
	#[pallet::getter(fn duration_offset)]
	pub type DurationOffset<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery>;

	/// Mapping from block number to new session index and duration.
	///
	/// SessionDurationChanges: map BlockNumber => (SessionIndex, SessionDuration)
	#[pallet::storage]
	#[pallet::getter(fn session_duration_changes)]
	pub type SessionDurationChanges<T: Config> =
		StorageMap<_, Twox64Concat, BlockNumberFor<T>, (SessionIndex, BlockNumberFor<T>), ValueQuery>;

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub session_duration: BlockNumberFor<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			SessionDuration::<T>::put(self.session_duration);
		}
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(n: BlockNumberFor<T>) -> Weight {
			let mut skip = true;
			SessionDurationChanges::<T>::mutate_exists(n, |maybe_changes| {
				if let Some((_, duration)) = maybe_changes.take() {
					skip = false;
					SessionDuration::<T>::put(duration);
					DurationOffset::<T>::put(n);
				}
			});

			if skip {
				T::WeightInfo::on_initialize_skip()
			} else {
				T::WeightInfo::on_initialize()
			}
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Schedule a new session duration in the specified session index.
		///
		/// - `start_session`: the session index that the new change become effective.
		/// - `duration`:  new session duration.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::schedule_session_duration())]
		pub fn schedule_session_duration(
			origin: OriginFor<T>,
			#[pallet::compact] start_session: SessionIndex,
			#[pallet::compact] duration: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_root(origin)?;

			let target_block_number = Self::do_schedule_session_duration(start_session, duration)?;

			Self::deposit_event(Event::ScheduledSessionDuration {
				block_number: target_block_number,
				session_index: start_session,
				session_duration: duration,
			});
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_schedule_session_duration(
		start_session: SessionIndex,
		duration: BlockNumberFor<T>,
	) -> Result<BlockNumberFor<T>, DispatchError> {
		let block_number = <frame_system::Pallet<T>>::block_number();
		let current_session = T::ValidatorSet::session_index();

		ensure!(start_session > current_session, Error::<T>::InvalidSession);
		ensure!(!duration.is_zero(), Error::<T>::InvalidDuration);

		if duration == Self::session_duration() {
			return Ok(block_number);
		}

		let next_session = Self::estimate_next_session_rotation(block_number)
			.0
			.ok_or(Error::<T>::EstimateNextSessionFailed)?;
		let target_block_number =
			Into::<BlockNumberFor<T>>::into(start_session.saturating_sub(current_session).saturating_sub(1))
				.saturating_mul(Self::session_duration())
				.saturating_add(next_session);

		SessionDurationChanges::<T>::insert(target_block_number, (start_session, duration));

		Ok(target_block_number)
	}
}

impl<T: Config> ShouldEndSession<BlockNumberFor<T>> for Pallet<T> {
	fn should_end_session(now: BlockNumberFor<T>) -> bool {
		let offset = Self::duration_offset();
		let period = Self::session_duration();

		if period.is_zero() {
			return false;
		}

		now >= offset && (now.saturating_sub(offset) % period).is_zero()
	}
}

impl<T: Config> EstimateNextSessionRotation<BlockNumberFor<T>> for Pallet<T> {
	fn average_session_length() -> BlockNumberFor<T> {
		Self::session_duration()
	}

	fn estimate_current_session_progress(now: BlockNumberFor<T>) -> (Option<Permill>, Weight) {
		let offset = Self::duration_offset();
		let period = Self::session_duration();

		if period.is_zero() {
			return (None, T::WeightInfo::estimate_current_session_progress());
		}

		// NOTE: we add one since we assume that the current block has already elapsed,
		// i.e. when evaluating the last block in the session the progress should be 100%
		// (0% is never returned).
		let progress = if now >= offset {
			let current = (now.saturating_sub(offset) % period).saturating_add(One::one());
			Some(Permill::from_rational(current, period))
		} else {
			None
		};

		(progress, T::WeightInfo::estimate_next_session_rotation())
	}

	fn estimate_next_session_rotation(now: BlockNumberFor<T>) -> (Option<BlockNumberFor<T>>, Weight) {
		let offset = Self::duration_offset();
		let period = Self::session_duration();

		if period.is_zero() {
			return (None, T::WeightInfo::estimate_next_session_rotation());
		}

		let next_session = if now > offset {
			let block_after_last_session = now.saturating_sub(offset) % period;
			if block_after_last_session > Zero::zero() {
				now.saturating_add(period.saturating_sub(block_after_last_session))
			} else {
				// this branch happens when the session is already rotated or will rotate in this
				// block (depending on being called before or after `session::on_initialize`). Here,
				// we assume the latter, namely that this is called after `session::on_initialize`,
				// and thus we add period to it as well.
				now.saturating_add(period)
			}
		} else {
			offset
		};

		(Some(next_session), T::WeightInfo::estimate_next_session_rotation())
	}
}
