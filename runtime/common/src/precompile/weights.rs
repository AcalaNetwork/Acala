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

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]
#![allow(clippy::unnecessary_cast)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

pub struct PrecompileWeights<T>(PhantomData<T>);
impl<T: frame_system::Config> PrecompileWeights<T> {
	// AssetRegistry::AssetMetadatas (r: 1, w: 0)
	// Oracle::Values (r: 1, w: 0)
	pub fn oracle_get_price() -> Weight {
		(19_623_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
	}
	pub fn evm_query_new_contract_extra_bytes() -> Weight {
		(1_696_000 as Weight)
	}
	pub fn evm_query_storage_deposit_per_byte() -> Weight {
		(1_761_000 as Weight)
	}
	// EVMModule::Accounts (r: 1, w: 0)
	pub fn evm_query_maintainer() -> Weight {
		(7_088_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(1 as Weight))
	}
	pub fn evm_query_developer_deposit() -> Weight {
		(1_716_000 as Weight)
	}
	pub fn evm_query_publication_fee() -> Weight {
		(1_744_000 as Weight)
	}
	// Balances::Reserves (r: 1, w: 0)
	// EvmAccounts::Accounts (r: 1, w: 0)
	pub fn evm_query_developer_status() -> Weight {
		(8_506_000 as Weight)
			.saturating_add(T::DbWeight::get().reads(2 as Weight))
	}
}
