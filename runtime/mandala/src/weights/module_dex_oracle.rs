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

//! Autogenerated weights for module_dex_oracle
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

/// Weight functions for module_dex_oracle.
pub struct WeightInfo<T>(PhantomData<T>);
impl<T: frame_system::Config> module_dex_oracle::WeightInfo for WeightInfo<T> {
	// Storage: `Aura::CurrentSlot` (r:1 w:1)
	// Proof: `Aura::CurrentSlot` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	// Storage: `Aura::Authorities` (r:1 w:0)
	// Proof: `Aura::Authorities` (`max_values`: Some(1), `max_size`: Some(1025), added: 1520, mode: `MaxEncodedLen`)
	// Storage: `DexOracle::AveragePrices` (r:4 w:3)
	// Proof: `DexOracle::AveragePrices` (`max_values`: None, `max_size`: None, mode: `Measured`)
	// Storage: `Dex::LiquidityPool` (r:3 w:0)
	// Proof: `Dex::LiquidityPool` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	// Storage: `DexOracle::Cumulatives` (r:3 w:3)
	// Proof: `DexOracle::Cumulatives` (`max_values`: None, `max_size`: None, mode: `Measured`)
	// Storage: `System::InherentsApplied` (r:0 w:1)
	// Proof: `System::InherentsApplied` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	// Storage: `System::ParentHash` (r:0 w:1)
	// Proof: `System::ParentHash` (`max_values`: Some(1), `max_size`: Some(32), added: 527, mode: `MaxEncodedLen`)
	// Storage: `System::Digest` (r:0 w:1)
	// Proof: `System::Digest` (`max_values`: Some(1), `max_size`: None, mode: `Measured`)
	// Storage: `System::BlockHash` (r:0 w:1)
	// Proof: `System::BlockHash` (`max_values`: None, `max_size`: Some(44), added: 2519, mode: `MaxEncodedLen`)
	// Storage: UNKNOWN KEY `0x3a65787472696e7369635f696e646578` (r:0 w:1)
	// Proof: UNKNOWN KEY `0x3a65787472696e7369635f696e646578` (r:0 w:1)
	// Storage: UNKNOWN KEY `0x3a696e747261626c6f636b5f656e74726f7079` (r:0 w:1)
	// Proof: UNKNOWN KEY `0x3a696e747261626c6f636b5f656e74726f7079` (r:0 w:1)
	// Storage: `Timestamp::Now` (r:0 w:1)
	// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	// Storage: `Timestamp::DidUpdate` (r:0 w:1)
	// Proof: `Timestamp::DidUpdate` (`max_values`: Some(1), `max_size`: Some(1), added: 496, mode: `MaxEncodedLen`)
	/// The range of component `n` is `[0, 3]`.
	/// The range of component `u` is `[0, 3]`.
	fn on_initialize_with_update_average_prices(n: u32, u: u32, ) -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `203 + n * (377 ±0) + u * (249 ±0)`
		//  Estimated: `4886 + n * (2864 ±33) + u * (346 ±33)`
		// Minimum execution time: 13_884 nanoseconds.
		Weight::from_parts(14_463_000, 4886)
			// Standard Error: 147_155
			.saturating_add(Weight::from_parts(10_440_038, 0).saturating_mul(n.into()))
			// Standard Error: 147_155
			.saturating_add(Weight::from_parts(4_618_173, 0).saturating_mul(u.into()))
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().reads((2_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().reads((1_u64).saturating_mul(u.into())))
			.saturating_add(T::DbWeight::get().writes(3))
			.saturating_add(T::DbWeight::get().writes((2_u64).saturating_mul(n.into())))
			.saturating_add(T::DbWeight::get().writes((2_u64).saturating_mul(u.into())))
			.saturating_add(Weight::from_parts(0, 2864).saturating_mul(n.into()))
			.saturating_add(Weight::from_parts(0, 346).saturating_mul(u.into()))
	}
	// Storage: `DexOracle::AveragePrices` (r:1 w:1)
	// Proof: `DexOracle::AveragePrices` (`max_values`: None, `max_size`: None, mode: `Measured`)
	// Storage: `Dex::LiquidityPool` (r:1 w:0)
	// Proof: `Dex::LiquidityPool` (`max_values`: None, `max_size`: Some(126), added: 2601, mode: `MaxEncodedLen`)
	// Storage: `Timestamp::Now` (r:1 w:0)
	// Proof: `Timestamp::Now` (`max_values`: Some(1), `max_size`: Some(8), added: 503, mode: `MaxEncodedLen`)
	// Storage: `DexOracle::Cumulatives` (r:0 w:1)
	// Proof: `DexOracle::Cumulatives` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn enable_average_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `990`
		//  Estimated: `4455`
		// Minimum execution time: 19_313 nanoseconds.
		Weight::from_parts(19_636_000, 4455)
			.saturating_add(T::DbWeight::get().reads(3))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	// Storage: `DexOracle::AveragePrices` (r:1 w:1)
	// Proof: `DexOracle::AveragePrices` (`max_values`: None, `max_size`: None, mode: `Measured`)
	// Storage: `DexOracle::Cumulatives` (r:0 w:1)
	// Proof: `DexOracle::Cumulatives` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn disable_average_price() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `960`
		//  Estimated: `4425`
		// Minimum execution time: 12_499 nanoseconds.
		Weight::from_parts(12_818_000, 4425)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(2))
	}
	// Storage: `DexOracle::AveragePrices` (r:1 w:1)
	// Proof: `DexOracle::AveragePrices` (`max_values`: None, `max_size`: None, mode: `Measured`)
	fn update_average_price_interval() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `960`
		//  Estimated: `4425`
		// Minimum execution time: 11_603 nanoseconds.
		Weight::from_parts(12_058_000, 4425)
			.saturating_add(T::DbWeight::get().reads(1))
			.saturating_add(T::DbWeight::get().writes(1))
	}
}
