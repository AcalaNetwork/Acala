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

use crate::{Config, DurationOffset, SessionDuration, Weight};
use frame_support::traits::Get;
use sp_runtime::traits::Zero;

pub mod v1 {
	use super::*;

	pub fn pre_migrate<T: Config>(session_duration: T::BlockNumber) -> Result<(), &'static str> {
		assert!(!session_duration.is_zero(), "session_duration is zero.");
		assert!(SessionDuration::<T>::get().is_zero(), "SessionDuration already set.");
		assert!(DurationOffset::<T>::get().is_zero(), "SessionDuration already set.");
		Ok(())
	}

	pub fn post_migrate<T: Config>(session_duration: T::BlockNumber) -> Result<(), &'static str> {
		assert!(
			SessionDuration::<T>::get() == session_duration,
			"SessionDuration not set."
		);
		assert!(DurationOffset::<T>::get().is_zero(), "DurationOffset has been set.");
		Ok(())
	}

	pub fn migrate<T: Config>(session_duration: T::BlockNumber) -> Weight {
		log::info!(target: "session-manager", "Migrating session-manager v1");

		if SessionDuration::<T>::get().is_zero() && !session_duration.is_zero() {
			SessionDuration::<T>::put(session_duration);
		}
		log::info!(target: "session-manager", "Completed session-manager migration to v1");

		T::DbWeight::get().reads_writes(1, 1)
	}
}
