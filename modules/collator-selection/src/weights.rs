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

#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{
	traits::Get,
	weights::{constants::RocksDbWeight, Weight},
};
use sp_std::marker::PhantomData;

// The weight info trait for `pallet_collator_selection`.
pub trait WeightInfo {
	fn set_invulnerables(_b: u32) -> Weight;
	fn set_desired_candidates() -> Weight;
	fn set_candidacy_bond() -> Weight;
	fn register_as_candidate(_c: u32) -> Weight;
	fn leave_intent(_c: u32) -> Weight;
	fn note_author(_c: u32) -> Weight;
	fn new_session(_c: u32, _r: u32) -> Weight;
}

/// Weights for pallet_collator_selection using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn set_invulnerables(b: u32) -> Weight {
		(28_060_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((118_000 as Weight).saturating_mul(b as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn set_desired_candidates() -> Weight {
		(25_000_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn set_candidacy_bond() -> Weight {
		(25_000_000 as Weight).saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn register_as_candidate(c: u32) -> Weight {
		(82_496_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((266_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn leave_intent(c: u32) -> Weight {
		(65_836_000 as Weight)
			// Standard Error: 2_000
			.saturating_add((273_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
	fn note_author(c: u32) -> Weight {
		(108_730_000 as Weight)
			// Standard Error: 3_000
			.saturating_add((286_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(4 as Weight))
			.saturating_add(T::DbWeight::get().writes(4 as Weight))
	}
	fn new_session(r: u32, c: u32) -> Weight {
		(50_005_000 as Weight)
			// Standard Error: 2_000
			.saturating_add((8_000 as Weight).saturating_mul(r as Weight))
			// Standard Error: 2_000
			.saturating_add((291_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(T::DbWeight::get().reads(3 as Weight))
			.saturating_add(T::DbWeight::get().writes(1 as Weight))
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	fn set_invulnerables(b: u32) -> Weight {
		(28_060_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((118_000 as Weight).saturating_mul(b as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn set_desired_candidates() -> Weight {
		(25_000_000 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn set_candidacy_bond() -> Weight {
		(25_000_000 as Weight).saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn register_as_candidate(c: u32) -> Weight {
		(82_496_000 as Weight)
			// Standard Error: 1_000
			.saturating_add((266_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(3 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn leave_intent(c: u32) -> Weight {
		(65_836_000 as Weight)
			// Standard Error: 2_000
			.saturating_add((273_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(1 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
	fn note_author(c: u32) -> Weight {
		(108_730_000 as Weight)
			// Standard Error: 3_000
			.saturating_add((286_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(4 as Weight))
			.saturating_add(RocksDbWeight::get().writes(4 as Weight))
	}
	fn new_session(r: u32, c: u32) -> Weight {
		(50_005_000 as Weight)
			// Standard Error: 2_000
			.saturating_add((8_000 as Weight).saturating_mul(r as Weight))
			// Standard Error: 2_000
			.saturating_add((291_000 as Weight).saturating_mul(c as Weight))
			.saturating_add(RocksDbWeight::get().reads(3 as Weight))
			.saturating_add(RocksDbWeight::get().writes(1 as Weight))
	}
}
