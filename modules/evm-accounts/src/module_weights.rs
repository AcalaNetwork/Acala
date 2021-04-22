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
#![allow(dead_code)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct ModuleWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> ModuleWeights<T> {
	pub fn ethereum_signable_message() -> Weight {
		(1_470_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn eth_recover() -> Weight {
		(141_047_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn eth_public() -> Weight {
		(46_536_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn eth_address() -> Weight {
		(46_861_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn eth_sign() -> Weight {
		(130_974_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn get_account_id() -> Weight {
		(2_122_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn get_evm_address() -> Weight {
		(2_023_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn get_or_create_evm_address() -> Weight {
		(5_912_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(2 as Weight))
	}
	pub fn get_default_evm_address() -> Weight {
		(1_023_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(0 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
	pub fn is_linked() -> Weight {
		(2_802_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
			.saturating_add(T::DbWeight::get().writes(0 as Weight))
	}
}
