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

//! Unit tests for asset registry module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{AssetRegistry, CouncilAccount, Event, ExtBuilder, Origin, Runtime, System};

#[test]
fn versioned_multi_location_convert_work() {
	ExtBuilder::default().build().execute_with(|| {
		// v0
		let v0_location = VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000)));
		let location: MultiLocation = v0_location.try_into().unwrap();
		assert_eq!(
			location,
			MultiLocation {
				parents: 0,
				interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000))
			}
		);

		// v1
		let v1_location = VersionedMultiLocation::V1(MultiLocation {
			parents: 0,
			interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000)),
		});
		let location: MultiLocation = v1_location.try_into().unwrap();
		assert_eq!(
			location,
			MultiLocation {
				parents: 0,
				interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000))
			}
		);

		// handle all of VersionedMultiLocation
		assert!(match location.into() {
			VersionedMultiLocation::V0 { .. } | VersionedMultiLocation::V1 { .. } => true,
		});
	});
}

#[test]
fn register_foreign_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v0_location = VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000)));

		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::signed(CouncilAccount::get()),
			v0_location.clone(),
			AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			}
		));

		System::assert_last_event(Event::AssetRegistry(crate::Event::RegisteredForeignAsset(
			AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		)));

		let location: MultiLocation = v0_location.try_into().unwrap();
		assert_eq!(
			AssetMetadatas::<Runtime>::get(location),
			Some(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		);
	});
}

#[test]
fn register_foreign_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::signed(CouncilAccount::get()),
			VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000))),
			AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			}
		));

		// v0
		assert_noop!(
			AssetRegistry::register_foreign_asset(
				Origin::signed(CouncilAccount::get()),
				VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000))),
				AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				}
			),
			Error::<Runtime>::AssetMetadataExisted
		);

		// v1
		assert_noop!(
			AssetRegistry::register_foreign_asset(
				Origin::signed(CouncilAccount::get()),
				VersionedMultiLocation::V1(MultiLocation {
					parents: 0,
					interior: xcm::v1::Junctions::X1(xcm::v1::Junction::Parachain(1000))
				}),
				AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				}
			),
			Error::<Runtime>::AssetMetadataExisted
		);
	});
}

#[test]
fn update_foreign_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v0_location = VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000)));

		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::signed(CouncilAccount::get()),
			v0_location.clone(),
			AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			}
		));

		assert_ok!(AssetRegistry::update_foreign_asset(
			Origin::signed(CouncilAccount::get()),
			v0_location.clone(),
			AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			}
		));

		System::assert_last_event(Event::AssetRegistry(crate::Event::UpdatedForeignAsset(AssetMetadata {
			name: b"New Token Name".to_vec(),
			symbol: b"NTN".to_vec(),
			decimals: 13,
			minimal_balance: 2,
		})));

		let location: MultiLocation = v0_location.try_into().unwrap();
		assert_eq!(
			AssetMetadatas::<Runtime>::get(location),
			Some(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		);
	});
}

#[test]
fn update_foreign_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v0_location = VersionedMultiLocation::V0(xcm::v0::MultiLocation::X1(xcm::v0::Junction::Parachain(1000)));

		assert_noop!(
			AssetRegistry::update_foreign_asset(
				Origin::signed(CouncilAccount::get()),
				v0_location.clone(),
				AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				}
			),
			Error::<Runtime>::AssetMetadataNotExists
		);

		assert_ok!(AssetRegistry::register_foreign_asset(
			Origin::signed(CouncilAccount::get()),
			v0_location,
			AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			}
		));
	});
}
