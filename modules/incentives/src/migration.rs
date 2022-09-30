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

use super::*;
use crate::log;
use frame_support::traits::OnRuntimeUpgrade;

/// Clear all DexSavingRewardRates storage
pub struct ClearDexSavingRewardRates<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> OnRuntimeUpgrade for ClearDexSavingRewardRates<T> {
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "incentives",
			"ClearDexSavingRewardRates::on_runtime_upgrade execute, will clear Storage DexSavingRewardRates",
		);

		// clear storage DexSavingRewardRates,
		let _ = DexSavingRewardRates::<T>::clear(u32::max_value(), None);

		0
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		assert_eq!(DexSavingRewardRates::<T>::iter().count(), 0);

		log::info!(
			target: "incentives",
			"ClearDexSavingRewardRates done!",
		);

		Ok(())
	}
}
