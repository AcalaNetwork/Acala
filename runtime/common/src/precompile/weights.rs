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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct PrecompileWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> PrecompileWeights<T> {
	// AssetRegistry::AssetMetadatas (r: 1, w: 0)
	// Oracle::Values (r: 1, w: 0)
	pub fn oracle_get_price() -> Weight {
		Weight::from_parts(18_457_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
	}
	pub fn evm_query_new_contract_extra_bytes() -> Weight {
		Weight::from_parts(913_000, 0)
	}
	pub fn evm_query_storage_deposit_per_byte() -> Weight {
		Weight::from_parts(905_000, 0)
	}
	// EVMModule::Accounts (r: 1, w: 0)
	pub fn evm_query_maintainer() -> Weight {
		Weight::from_parts(6_214_000, 0)
			.saturating_add(T::DbWeight::get().reads(1))
	}
	pub fn evm_query_developer_deposit() -> Weight {
		Weight::from_parts(881_000, 0)
	}
	pub fn evm_query_publication_fee() -> Weight {
		Weight::from_parts(874_000, 0)
	}
	// Balances::Reserves (r: 1, w: 0)
	// EvmAccounts::Accounts (r: 1, w: 0)
	pub fn evm_query_developer_status() -> Weight {
		Weight::from_parts(7_198_000, 0)
			.saturating_add(T::DbWeight::get().reads(2))
	}
}
