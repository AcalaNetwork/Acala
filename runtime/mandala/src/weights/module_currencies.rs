// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

//! Autogenerated weights for module_currencies
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 35.0.1
//! DATE: 2024-04-29, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `ip-172-31-38-126`, CPU: `Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz`
//! WASM-EXECUTION: Compiled, CHAIN: Some("dev"), DB CACHE: 1024

// Executed Command:
// target/production/acala
// benchmark
// pallet
// --chain=dev
// --steps=50
// --repeat=20
// --pallet=*
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --template=./templates/runtime-weight-template.hbs
// --output=./runtime/mandala/src/weights/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for module_currencies.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_currencies::WeightInfo for WeightInfo<T> {
	// Storage: `Tokens::Accounts` (r:2 w:2)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	// Storage: `EvmAccounts::EvmAddresses` (r:1 w:0)
	// Proof: `EvmAccounts::EvmAddresses` (`max_values`: None, `max_size`: Some(60), added: 2535, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn transfer_non_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2559`
		//  Estimated: `6234`
		// Minimum execution time: 47_857 nanoseconds.
		Weight::from_parts(48_516_000, 6234)
			.saturating_add(T::DbWeight::get().reads(4))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	// Storage: `EvmAccounts::EvmAddresses` (r:1 w:0)
	// Proof: `EvmAccounts::EvmAddresses` (`max_values`: None, `max_size`: Some(60), added: 2535, mode: `MaxEncodedLen`)
	fn transfer_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2121`
		//  Estimated: `3593`
		// Minimum execution time: 59_229 nanoseconds.
		Weight::from_parts(60_067_000, 3593)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	// Storage: `Tokens::TotalIssuance` (r:1 w:1)
	// Proof: `Tokens::TotalIssuance` (`max_values`: None, `max_size`: Some(67), added: 2542, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_balance_non_native_currency() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2041`
		//  Estimated: `3612`
		// Minimum execution time: 30_051 nanoseconds.
		Weight::from_parts(30_734_000, 3612)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn update_balance_native_currency_creating() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1773`
		//  Estimated: `3593`
		// Minimum execution time: 29_629 nanoseconds.
		Weight::from_parts(30_280_000, 3593)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	// Storage: `EvmAccounts::EvmAddresses` (r:1 w:0)
	// Proof: `EvmAccounts::EvmAddresses` (`max_values`: None, `max_size`: Some(60), added: 2535, mode: `MaxEncodedLen`)
	fn update_balance_native_currency_killing() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1912`
		//  Estimated: `3593`
		// Minimum execution time: 33_224 nanoseconds.
		Weight::from_parts(33_929_000, 3593)
			.saturating_add(T::DbWeight::get().reads(2))
			.saturating_add(T::DbWeight::get().writes(1))
	}
	// Storage: `Tokens::Accounts` (r:4 w:4)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:3 w:3)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	/// The range of component `c` is `[1, 3]`.
	fn sweep_dust(c: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1913 + c * (339 ±0)`
		//  Estimated: `3612 + c * (2622 ±0)`
		// Minimum execution time: 38_434 nanoseconds.
		Weight::from_parts(19_529_506, 3612)
			// Standard Error: 35_673
			.saturating_add(Weight::from_parts(20_305_483, 0).saturating_mul(c.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(c.into())))
			.saturating_add(T::DbWeight::get().writes(1))
			.saturating_add(T::DbWeight::get().writes((2_u64).saturating_mul(c.into())))
			.saturating_add(Weight::from_parts(0, 2622).saturating_mul(c.into()))
	}
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1300), added: 3775, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn force_set_lock() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2242`
		//  Estimated: `4765`
		// Minimum execution time: 33_897 nanoseconds.
		Weight::from_parts(34_619_000, 4765)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(3))
	}
	// Storage: `Tokens::Locks` (r:1 w:1)
	// Proof: `Tokens::Locks` (`max_values`: None, `max_size`: Some(1300), added: 3775, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:1 w:1)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	fn force_remove_lock() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `2347`
		//  Estimated: `4765`
		// Minimum execution time: 35_270 nanoseconds.
		Weight::from_parts(36_298_000, 4765)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(3))
	}
}
