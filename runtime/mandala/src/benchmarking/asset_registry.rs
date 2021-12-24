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

use crate::{AssetRegistry, Runtime};

use frame_system::RawOrigin;
use module_asset_registry::AssetMetadata;
use orml_benchmarking::runtime_benchmarks;
use sp_std::boxed::Box;
use xcm::{v1::MultiLocation, VersionedMultiLocation};

runtime_benchmarks! {
	{ Runtime, module_asset_registry }

	register_foreign_asset {
		let location = VersionedMultiLocation::V1(MultiLocation {
			parents: 0,
			interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000)),
		});
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};
	}: _(RawOrigin::Root, Box::new(location), Box::new(asset_metadata))

	update_foreign_asset {
		let location = VersionedMultiLocation::V1(MultiLocation {
			parents: 0,
			interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000)),
		});
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};

		AssetRegistry::register_foreign_asset(RawOrigin::Root.into(), Box::new(location.clone()), Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, 0, Box::new(location), Box::new(asset_metadata))

	register_stable_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};
	}: _(RawOrigin::Root, Box::new(asset_metadata))

	update_stable_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};

		AssetRegistry::register_stable_asset(RawOrigin::Root.into(), Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, 0, Box::new(asset_metadata))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
