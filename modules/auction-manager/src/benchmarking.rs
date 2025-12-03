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

use super::*;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper {
	fn setup() -> Option<AuctionId>;
}

impl BenchmarkHelper for () {
	fn setup() -> Option<AuctionId> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	// `cancel` a collateral auction, worst case:
	// auction have been already bid
	#[benchmark]
	fn cancel_collateral_auction() {
		let auction_id: AuctionId = T::BenchmarkHelper::setup().unwrap();

		#[extrinsic_call]
		cancel(RawOrigin::None, auction_id);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
