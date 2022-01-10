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

use crate::{Config, DurationOffset, SessionDuration, Weight};
use frame_support::traits::Get;
use sp_runtime::traits::Zero;

pub mod v1 {
	use super::*;

	// https://github.com/AcalaNetwork/Acala/blob/ea218feb68bfce954513cf61d754b0a9ddb36c2c/runtime/karura/src/lib.rs#L268
	const PERIOD: u32 = 1800u32;

	pub fn pre_migrate<T: Config>() -> Result<(), &'static str> {
		assert!(SessionDuration::<T>::get().is_zero(), "SessionDuration already set.");
		assert!(DurationOffset::<T>::get().is_zero(), "SessionDuration already set.");
		Ok(())
	}

	pub fn post_migrate<T: Config>() -> Result<(), &'static str> {
		assert!(
			SessionDuration::<T>::get() == Into::<T::BlockNumber>::into(PERIOD),
			"SessionDuration not set."
		);
		assert!(DurationOffset::<T>::get().is_zero(), "DurationOffset has been set.");
		Ok(())
	}

	pub fn migrate<T: Config>() -> Weight {
		log::info!(target: "session-manager", "Migrating session-manager v1");

		if SessionDuration::<T>::get().is_zero() {
			SessionDuration::<T>::put(Into::<T::BlockNumber>::into(PERIOD));
		}
		log::info!(target: "session-manager", "Completed session-manager migration to v1");

		T::DbWeight::get().reads_writes(1, 1)
	}
}
