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
use frame_support::assert_ok;
use frame_system::RawOrigin;

/// Helper trait for benchmarking.
pub trait BenchmarkHelper {
	fn setup_deploy_contract() -> Option<EvmAddress>;
}

impl BenchmarkHelper for () {
	fn setup_deploy_contract() -> Option<EvmAddress> {
		None
	}
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn register_foreign_asset() {
		let location = VersionedLocation::V5(Location::new(0, [Parachain(1000)]));
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		#[extrinsic_call]
		_(RawOrigin::Root, Box::new(location), Box::new(asset_metadata));
	}

	#[benchmark]
	fn update_foreign_asset() {
		let location = VersionedLocation::V5(Location::new(0, [Parachain(1000)]));
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		assert_ok!(Pallet::<T>::register_foreign_asset(
			RawOrigin::Root.into(),
			Box::new(location.clone()),
			Box::new(asset_metadata.clone())
		));

		#[extrinsic_call]
		_(RawOrigin::Root, 0, Box::new(location), Box::new(asset_metadata));
	}

	#[benchmark]
	fn register_stable_asset() {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		#[extrinsic_call]
		_(RawOrigin::Root, Box::new(asset_metadata));
	}

	#[benchmark]
	fn update_stable_asset() {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		assert_ok!(Pallet::<T>::register_stable_asset(
			RawOrigin::Root.into(),
			Box::new(asset_metadata.clone())
		));

		#[extrinsic_call]
		_(RawOrigin::Root, 0, Box::new(asset_metadata));
	}

	#[benchmark]
	fn register_erc20_asset() {
		let erc20_address = T::BenchmarkHelper::setup_deploy_contract().unwrap();

		#[extrinsic_call]
		_(RawOrigin::Root, erc20_address, 1u32.into());
	}

	#[benchmark]
	fn update_erc20_asset() {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		let erc20_address = T::BenchmarkHelper::setup_deploy_contract().unwrap();
		assert_ok!(Pallet::<T>::register_erc20_asset(
			RawOrigin::Root.into(),
			erc20_address,
			1u32.into()
		));

		#[extrinsic_call]
		_(RawOrigin::Root, erc20_address, Box::new(asset_metadata));
	}

	#[benchmark]
	fn register_native_asset() {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			CurrencyId::LiquidCrowdloan(0),
			Box::new(asset_metadata),
		);
	}

	#[benchmark]
	fn update_native_asset() {
		let currency_id = CurrencyId::LiquidCrowdloan(0);
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12u8,
			minimal_balance: 1u32.into(),
		};

		assert_ok!(Pallet::<T>::register_native_asset(
			RawOrigin::Root.into(),
			currency_id,
			Box::new(asset_metadata.clone())
		));

		#[extrinsic_call]
		_(RawOrigin::Root, currency_id, Box::new(asset_metadata));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Runtime);
}
