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

//! Tests to make sure that Acala's weights and fees match what we
//! expect from Substrate or ORML.
//!
//! These test are not meant to be exhaustive, as it is inevitable that
//! weights in Substrate will change. Instead they are supposed to provide
//! some sort of indicator that calls we consider important (e.g
//! Balances::transfer) have not suddenly changed from under us.

use frame_support::weights::constants::*;

#[test]
fn sanity_check_weight_per_time_constants_are_as_expected() {
	// These values comes from Substrate, we want to make sure that if it
	// ever changes we don't accidently break Polkadot
	assert_eq!(WEIGHT_REF_TIME_PER_SECOND, 1_000_000_000_000);
	assert_eq!(WEIGHT_REF_TIME_PER_MILLIS, WEIGHT_REF_TIME_PER_SECOND / 1000);
	assert_eq!(WEIGHT_REF_TIME_PER_MICROS, WEIGHT_REF_TIME_PER_MILLIS / 1000);
	assert_eq!(WEIGHT_REF_TIME_PER_NANOS, WEIGHT_REF_TIME_PER_MICROS / 1000);
}
