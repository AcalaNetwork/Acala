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

//! Autogenerated weights for module_aggregated_dex
//!
//! THIS FILE WAS AUTO-GENERATED USING THE SUBSTRATE BENCHMARK CLI VERSION 35.0.1
//! DATE: 2024-04-29, STEPS: `50`, REPEAT: 20, LOW RANGE: `[]`, HIGH RANGE: `[]`
//! HOSTNAME: `ip-172-31-41-141`, CPU: `Intel(R) Xeon(R) Platinum 8375C CPU @ 2.90GHz`
//! WASM-EXECUTION: Compiled, CHAIN: Some("acala-dev"), DB CACHE: 1024

// Executed Command:
// target/production/acala
// benchmark
// pallet
// --chain=acala-dev
// --steps=50
// --repeat=20
// --pallet=*
// --extrinsic=*
// --wasm-execution=compiled
// --heap-pages=4096
// --template=./templates/runtime-weight-template.hbs
// --output=./runtime/acala/src/weights/

#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::Weight};
use sp_std::marker::PhantomData;

/// Weight functions for module_aggregated_dex.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_aggregated_dex::WeightInfo for WeightInfo<T> {
	// Storage: `Dex::TradingPairStatuses` (r:3 w:0)
	// Proof: `Dex::TradingPairStatuses` (`max_values`: None, `max_size`: Some(195), added: 2670, mode: `MaxEncodedLen`)
	// Storage: `Dex::LiquidityPool` (r:3 w:3)
	// Proof: `Dex::LiquidityPool` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:2 w:2)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// The range of component `u` is `[2, 4]`.
	fn swap_with_exact_supply(u: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1842 + u * (112 ±0)`
		//  Estimated: `6234 + u * (643 ±18)`
		// Minimum execution time: 86_812 nanoseconds.
		Weight::from_parts(66_480_055, 6234)
			// Standard Error: 91_043
			.saturating_add(Weight::from_parts(12_057_473, 0).saturating_mul(u.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(u.into())))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(u.into())))
			.saturating_add(Weight::from_parts(0, 643).saturating_mul(u.into()))
	}
	// Storage: `Dex::TradingPairStatuses` (r:3 w:0)
	// Proof: `Dex::TradingPairStatuses` (`max_values`: None, `max_size`: Some(195), added: 2670, mode: `MaxEncodedLen`)
	// Storage: `Dex::LiquidityPool` (r:3 w:3)
	// Proof: `Dex::LiquidityPool` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	// Storage: `System::Account` (r:1 w:1)
	// Proof: `System::Account` (`max_values`: None, `max_size`: Some(128), added: 2603, mode: `MaxEncodedLen`)
	// Storage: `Tokens::Accounts` (r:2 w:2)
	// Proof: `Tokens::Accounts` (`max_values`: None, `max_size`: Some(147), added: 2622, mode: `MaxEncodedLen`)
	/// The range of component `u` is `[2, 4]`.
	fn swap_with_exact_target(u: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `1842 + u * (112 ±0)`
		//  Estimated: `6234 + u * (643 ±18)`
		// Minimum execution time: 93_849 nanoseconds.
		Weight::from_parts(65_961_366, 6234)
			// Standard Error: 152_574
			.saturating_add(Weight::from_parts(17_376_660, 0).saturating_mul(u.into()))
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(u.into())))
			.saturating_add(T::DbWeight::get().writes(2))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(u.into())))
			.saturating_add(Weight::from_parts(0, 643).saturating_mul(u.into()))
	}
	// Storage: `AggregatedDex::AggregatedSwapPaths` (r:0 w:5)
	// Proof: `AggregatedDex::AggregatedSwapPaths` (`max_values`: None, `max_size`: None, mode: `Measured`)
	/// The range of component `n` is `[0, 6]`.
	fn update_aggregated_swap_paths(n: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `666`
		//  Estimated: `666`
		// Minimum execution time: 3_971 nanoseconds.
		Weight::from_parts(3_692_678, 666)
			// Standard Error: 11_381
			.saturating_add(Weight::from_parts(1_464_785, 0).saturating_mul(n.into()))
			.saturating_add(T::DbWeight::get().writes((1_u64).saturating_mul(n.into())))
	}
}
