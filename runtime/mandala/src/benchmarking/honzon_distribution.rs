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

use super::utils::{initialize_swap_pools, SEED, STABLECOIN, STAKING};
use crate::{AccountId, HonzonDistribution, Runtime};
use frame_benchmarking::account;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use module_honzon_distribution::{DistributedBalance, DistributionDestination, DistributionToStableAsset};
use module_support::Ratio;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::FixedPointNumber;

runtime_benchmarks! {
	{ Runtime, module_honzon_distribution }

	update_params {
		let treasury: AccountId = account("treasury", 0, SEED);

		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 0,
			stable_currency_id: STABLECOIN,
			account_id: treasury,
		};
		let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);

	}: _(RawOrigin::Root, destination, None, None, None, None)

	force_adjust {
		let treasury: AccountId = account("treasury", 0, SEED);
		let funder: AccountId = account("funder", 0, SEED);

		// STAKING -> LIQUID -> STABLECOIN
		initialize_swap_pools(funder)?;

		let distribution_to_stable_asset = DistributionToStableAsset::<AccountId> {
			pool_id: 0,
			stable_token_index: 0,
			stable_currency_id: STAKING,
			account_id: treasury,
		};
		let destination = DistributionDestination::StableAsset(distribution_to_stable_asset);
		assert_ok!(HonzonDistribution::update_params(
			RawOrigin::Root.into(),
			destination.clone(),
			Some(1_000_000_000_000_000),
			Some(1_000_000_000_000_000),
			Some(Ratio::saturating_from_rational(6, 10)),
			Some(Ratio::saturating_from_rational(7, 10)),
		));

	}: _(RawOrigin::Root, destination.clone())
	verify {
		assert!(DistributedBalance::<Runtime>::get(&destination).is_some());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
