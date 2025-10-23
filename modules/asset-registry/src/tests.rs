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

//! Unit tests for asset registry module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	alice, deploy_contracts, deploy_contracts_same_prefix, erc20_address, erc20_address_not_exists,
	erc20_address_same_prefix, AssetRegistry, CouncilAccount, ExtBuilder, Runtime, RuntimeEvent, RuntimeOrigin, System,
};
use primitives::TokenSymbol;
use sp_core::H160;
use std::str::{from_utf8, FromStr};

#[test]
fn key_to_currency_work() {
	let erc20 = CurrencyId::Erc20(EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba644").unwrap());
	let mut data = [0u8; 32];
	data[..erc20.encode().len()].copy_from_slice(&erc20.encode()[..]);
	let v3_location = xcm::v3::MultiLocation::new(
		0,
		xcm::v3::Junctions::X1(xcm::v3::Junction::GeneralKey {
			length: erc20.encode().len() as u8,
			data: data,
		}),
	);
	let v4_location_from_v3 = v4::Location::try_from(v3_location.clone()).unwrap();
	let v4_location = v4::Location::new(
		0,
		v4::Junctions::X1([v4::Junction::from(BoundedVec::try_from(erc20.encode()).unwrap())].into()),
	);
	assert_eq!(v4_location_from_v3, v4_location);

	let v5_location_from_v4 = Location::try_from(v4_location.clone()).unwrap();
	let v5_location = Location::new(0, Junction::from(BoundedVec::try_from(erc20.encode()).unwrap()));
	assert_eq!(v5_location_from_v4, v5_location);
	assert_eq!(crate::key_to_currency(v5_location), Some(erc20));
}

#[test]
fn test_v3_to_v4_incompatible_location() {
	let v3_location = xcm::v3::MultiLocation::new(
		0,
		xcm::v3::Junctions::X1(xcm::v3::Junction::GeneralKey {
			length: 32,
			data: [0u8; 32],
		}),
	);

	let v4_location = v4::Location::new(0, v4::Junction::from(BoundedVec::try_from(vec![]).unwrap()));

	// Assert that V3 and V4 Location both are encoded differently
	assert!(v3_location.encode() != v4_location.encode());
}

#[test]
fn test_v4_to_v5_compatible_location() {
	let v4_location = v4::Location::new(0, v4::Junction::from(BoundedVec::try_from(vec![0]).unwrap()));

	let v5_location = Location::new(0, Junction::from(BoundedVec::try_from(vec![0]).unwrap()));

	// Assert that V4 and V5 Location both are encoded the same way
	assert!(v4_location.encode() == v5_location.encode());
}

#[test]
fn versioned_location_convert_work() {
	ExtBuilder::default().build().execute_with(|| {
		// v3
		let v3_location = VersionedLocation::V3(xcm::v3::MultiLocation {
			parents: 0,
			interior: xcm::v3::Junctions::X1(xcm::v3::Junction::Parachain(1000)),
		});
		let v4_location: Location = v3_location.try_into().unwrap();
		assert_eq!(
			v4_location,
			Location {
				parents: 0,
				interior: Junction::Parachain(1000).into()
			}
		);

		// v4
		let v4_location = VersionedLocation::V4(v4::Location {
			parents: 0,
			interior: xcm::v4::Junctions::X1([xcm::v4::Junction::Parachain(1000)].into()),
		});
		let v5_location: Location = v4_location.clone().try_into().unwrap();
		assert_eq!(
			v5_location,
			Location {
				parents: 0,
				interior: Junction::Parachain(1000).into()
			}
		);

		// handle all of VersionedLocation
		assert!(match v5_location.into() {
			VersionedLocation::V3 { .. } | VersionedLocation::V4 { .. } | VersionedLocation::V5 { .. } => true,
		});
	});
}

#[test]
fn register_foreign_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		// v3
		let v3_versioned_location = VersionedLocation::V3(v3::MultiLocation {
			parents: 0,
			interior: v3::Junctions::X1(v3::Junction::Parachain(1000)),
		});

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v3_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		let v3_location: v3::Location = v3_versioned_location.clone().try_into().unwrap();
		let v4_location: v4::Location = v3_versioned_location.try_into().unwrap();
		let v5_location: Location = v4_location.try_into().unwrap();
		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::ForeignAssetRegistered {
			asset_id: 0,
			asset_address: v5_location.clone(),
			metadata: AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		}));

		assert_eq!(ForeignAssetLocations::<Runtime>::get(0), Some(v3_location.clone()));
		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::ForeignAssetId(0)),
			Some(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		);
		assert_eq!(
			LocationToCurrencyIds::<Runtime>::get(v3_location),
			Some(CurrencyId::ForeignAsset(0))
		);

		// v4
		let v4_versioned_location = VersionedLocation::V4(v4::Location {
			parents: 0,
			interior: v4::Junctions::X1(
				[v4::Junction::GeneralKey {
					length: 32,
					data: [0u8; 32],
				}]
				.into(),
			),
		});
		let v3_location: v3::Location = v4_versioned_location.clone().try_into().unwrap();

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v4_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"Another Token Name".to_vec(),
				symbol: b"ATN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		let v4_location: v4::Location = v4_versioned_location.try_into().unwrap();
		let v5_location: Location = v4_location.clone().try_into().unwrap();
		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::ForeignAssetRegistered {
			asset_id: 1,
			asset_address: v5_location.clone(),
			metadata: AssetMetadata {
				name: b"Another Token Name".to_vec(),
				symbol: b"ATN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		}));

		assert_eq!(ForeignAssetLocations::<Runtime>::get(1), Some(v3_location.clone()));
		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::ForeignAssetId(1)),
			Some(AssetMetadata {
				name: b"Another Token Name".to_vec(),
				symbol: b"ATN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		);
		assert_eq!(
			LocationToCurrencyIds::<Runtime>::get(v3_location),
			Some(CurrencyId::ForeignAsset(1))
		);

		// v5
		let v5_versioned_location = VersionedLocation::V5(Location::new(
			0,
			[GeneralKey {
				length: 32,
				data: [1u8; 32],
			}],
		));

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v5_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"Another Token Name2".to_vec(),
				symbol: b"ATN2".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		let v3_location: v3::Location = v5_versioned_location.clone().try_into().unwrap();
		let _v4_location: v4::Location = v5_versioned_location.clone().try_into().unwrap();
		let v5_location: Location = v5_versioned_location.try_into().unwrap();
		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::ForeignAssetRegistered {
			asset_id: 2,
			asset_address: v5_location.clone(),
			metadata: AssetMetadata {
				name: b"Another Token Name2".to_vec(),
				symbol: b"ATN2".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		}));

		assert_eq!(ForeignAssetLocations::<Runtime>::get(2), Some(v3_location.clone()));
		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::ForeignAssetId(2)),
			Some(AssetMetadata {
				name: b"Another Token Name2".to_vec(),
				symbol: b"ATN2".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		);
		assert_eq!(
			LocationToCurrencyIds::<Runtime>::get(v3_location),
			Some(CurrencyId::ForeignAsset(2))
		);
	});
}

#[test]
fn register_foreign_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v5_location = VersionedLocation::V5(Location::new(0, [Parachain(1000)]));

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v5_location.clone()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		assert_noop!(
			AssetRegistry::register_foreign_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				Box::new(v5_location.clone()),
				Box::new(AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			),
			Error::<Runtime>::LocationExisted
		);

		NextForeignAssetId::<Runtime>::set(u16::MAX);
		assert_noop!(
			AssetRegistry::register_foreign_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				Box::new(v5_location),
				Box::new(AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn update_foreign_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v5_versioned_location = VersionedLocation::V5(Location::new(0, [Parachain(1000)]));

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v5_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		assert_ok!(AssetRegistry::update_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			0,
			Box::new(v5_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		));

		let v3_location: v3::Location = v5_versioned_location.clone().try_into().unwrap();
		let _v4_location: v4::Location = v5_versioned_location.clone().try_into().unwrap();
		let v5_location: Location = v5_versioned_location.try_into().unwrap();
		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::ForeignAssetUpdated {
			asset_id: 0,
			asset_address: v5_location.clone(),
			metadata: AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			},
		}));

		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::ForeignAssetId(0)),
			Some(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		);
		assert_eq!(ForeignAssetLocations::<Runtime>::get(0), Some(v3_location.clone()));
		assert_eq!(
			LocationToCurrencyIds::<Runtime>::get(v3_location.clone()),
			Some(CurrencyId::ForeignAsset(0))
		);

		// modify location
		let new_v5_versioned_location = VersionedLocation::V5(Location::new(0, [Parachain(2000)]));

		assert_ok!(AssetRegistry::update_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			0,
			Box::new(new_v5_versioned_location.clone()),
			Box::new(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		));
		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::ForeignAssetId(0)),
			Some(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		);
		let new_v3_location: v3::Location = v3::Location::try_from(new_v5_versioned_location).unwrap();
		assert_eq!(ForeignAssetLocations::<Runtime>::get(0), Some(new_v3_location.clone()));
		assert_eq!(LocationToCurrencyIds::<Runtime>::get(v3_location), None);
		assert_eq!(
			LocationToCurrencyIds::<Runtime>::get(new_v3_location),
			Some(CurrencyId::ForeignAsset(0))
		);
	});
}

#[test]
fn update_foreign_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		let v5_location = VersionedLocation::V5(Location::new(0, [Parachain(1000)]));

		assert_noop!(
			AssetRegistry::update_foreign_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				0,
				Box::new(v5_location.clone()),
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			),
			Error::<Runtime>::AssetIdNotExists
		);

		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(v5_location.clone()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		assert_ok!(AssetRegistry::update_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			0,
			Box::new(v5_location),
			Box::new(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		));

		// existed location
		let new_v5_location = VersionedLocation::V5(Location::new(0, [Parachain(2000)]));
		assert_ok!(AssetRegistry::register_foreign_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(new_v5_location.clone()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));
		assert_noop!(
			AssetRegistry::update_foreign_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				0,
				Box::new(new_v5_location),
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			),
			Error::<Runtime>::LocationExisted
		);
	});
}

#[test]
fn register_stable_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistry::register_stable_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetRegistered {
			asset_id: AssetIds::StableAssetId(0),
			metadata: AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		}));

		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::StableAssetId(0)),
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
fn register_stable_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistry::register_stable_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		NextStableAssetId::<Runtime>::set(0);
		assert_noop!(
			AssetRegistry::register_stable_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				Box::new(AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			),
			Error::<Runtime>::AssetIdExisted
		);

		NextStableAssetId::<Runtime>::set(u32::MAX);
		assert_noop!(
			AssetRegistry::register_stable_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				Box::new(AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			),
			ArithmeticError::Overflow
		);
	});
}

#[test]
fn update_stable_asset_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistry::register_stable_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		assert_ok!(AssetRegistry::update_stable_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			0,
			Box::new(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		));

		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetUpdated {
			asset_id: AssetIds::StableAssetId(0),
			metadata: AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			},
		}));

		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::StableAssetId(0)),
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
fn update_stable_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AssetRegistry::update_stable_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				0,
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			),
			Error::<Runtime>::AssetIdNotExists
		);
	});
}

#[test]
fn register_erc20_asset_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));

			System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetRegistered {
				asset_id: AssetIds::Erc20(erc20_address()),
				metadata: AssetMetadata {
					name: b"long string name, long string name, long string name, long string name, long string name"
						.to_vec(),
					symbol: b"TestToken".to_vec(),
					decimals: 17,
					minimal_balance: 1,
				},
			}));

			assert_eq!(Erc20IdToAddress::<Runtime>::get(0x5dddfce5), Some(erc20_address()));

			assert_eq!(
				AssetMetadatas::<Runtime>::get(AssetIds::Erc20(erc20_address())),
				Some(AssetMetadata {
					name: b"long string name, long string name, long string name, long string name, long string name"
						.to_vec(),
					symbol: b"TestToken".to_vec(),
					decimals: 17,
					minimal_balance: 1,
				})
			);
		});
}

#[test]
fn register_erc20_asset_should_not_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			deploy_contracts_same_prefix();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));

			assert_noop!(
				AssetRegistry::register_erc20_asset(
					RuntimeOrigin::signed(CouncilAccount::get()),
					erc20_address_same_prefix(),
					1
				),
				Error::<Runtime>::AssetIdExisted
			);

			assert_noop!(
				AssetRegistry::register_erc20_asset(
					RuntimeOrigin::signed(CouncilAccount::get()),
					erc20_address_not_exists(),
					1
				),
				module_evm_bridge::Error::<Runtime>::InvalidReturnValue,
			);
		});
}

#[test]
fn update_erc20_asset_work() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));

			assert_ok!(AssetRegistry::update_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			));

			System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetUpdated {
				asset_id: AssetIds::Erc20(erc20_address()),
				metadata: AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				},
			}));

			assert_eq!(
				AssetMetadatas::<Runtime>::get(AssetIds::Erc20(erc20_address())),
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
fn register_native_asset_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AssetRegistry::register_native_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			CurrencyId::Token(TokenSymbol::DOT),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));
		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetRegistered {
			asset_id: AssetIds::NativeAssetId(CurrencyId::Token(TokenSymbol::DOT)),
			metadata: AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			},
		}));

		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::NativeAssetId(CurrencyId::Token(TokenSymbol::DOT))),
			Some(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		);
		// Can't duplicate
		assert_noop!(
			AssetRegistry::register_native_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				CurrencyId::Token(TokenSymbol::DOT),
				Box::new(AssetMetadata {
					name: b"Token Name".to_vec(),
					symbol: b"TN".to_vec(),
					decimals: 12,
					minimal_balance: 1,
				})
			),
			Error::<Runtime>::AssetIdExisted
		);
	});
}

#[test]
fn update_native_asset_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AssetRegistry::update_native_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				CurrencyId::Token(TokenSymbol::DOT),
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			),
			Error::<Runtime>::AssetIdNotExists
		);

		assert_ok!(AssetRegistry::register_native_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			CurrencyId::Token(TokenSymbol::DOT),
			Box::new(AssetMetadata {
				name: b"Token Name".to_vec(),
				symbol: b"TN".to_vec(),
				decimals: 12,
				minimal_balance: 1,
			})
		));

		assert_ok!(AssetRegistry::update_native_asset(
			RuntimeOrigin::signed(CouncilAccount::get()),
			CurrencyId::Token(TokenSymbol::DOT),
			Box::new(AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			})
		));

		System::assert_last_event(RuntimeEvent::AssetRegistry(crate::Event::AssetUpdated {
			asset_id: AssetIds::NativeAssetId(CurrencyId::Token(TokenSymbol::DOT)),
			metadata: AssetMetadata {
				name: b"New Token Name".to_vec(),
				symbol: b"NTN".to_vec(),
				decimals: 13,
				minimal_balance: 2,
			},
		}));

		assert_eq!(
			AssetMetadatas::<Runtime>::get(AssetIds::NativeAssetId(CurrencyId::Token(TokenSymbol::DOT))),
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
fn update_erc20_asset_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AssetRegistry::update_stable_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				0,
				Box::new(AssetMetadata {
					name: b"New Token Name".to_vec(),
					symbol: b"NTN".to_vec(),
					decimals: 13,
					minimal_balance: 2,
				})
			),
			Error::<Runtime>::AssetIdNotExists
		);
	});
}

#[test]
fn name_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::Token(TokenSymbol::ACA)),
				Some(b"Acala".to_vec())
			);
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::Erc20(erc20_address())),
				Some(b"long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Token(TokenSymbol::AUSD))),
				Some(b"LP Acala - Acala Dollar".to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Token(TokenSymbol::AUSD))),
				Some(b"LP long string name, long string name, long string name, long string name, long string name - Acala Dollar"[..32].to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Erc20(erc20_address()))),
				Some(b"LP long string name, long string name, long string name, long string name, long string name - long string name, long string name, long string name, long string name, long string name"[..32].to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA), DexShare::Erc20(erc20_address_not_exists()))),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::name(CurrencyId::DexShare(DexShare::Erc20(erc20_address()), DexShare::Erc20(erc20_address_not_exists()))),
				None
			);

			assert_eq!(
				from_utf8(&EvmErc20InfoMapping::<Runtime>::name(CurrencyId::LiquidCrowdloan(0)).unwrap()),
				Ok("LiquidCrowdloan-Kusama-0")
			);
		});
}

#[test]
fn symbol_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::Token(TokenSymbol::ACA)),
				Some(b"ACA".to_vec())
			);
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::Erc20(erc20_address())),
				Some(b"TestToken".to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(b"LP_ACA_AUSD".to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(b"LP_TestToken_AUSD".to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				Some(b"LP_TestToken_TestToken".to_vec())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				from_utf8(&EvmErc20InfoMapping::<Runtime>::symbol(CurrencyId::LiquidCrowdloan(0)).unwrap()),
				Ok("LCKSM-0")
			);
		});
}

#[test]
fn decimals_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::Token(TokenSymbol::ACA)),
				Some(12)
			);
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::Erc20(erc20_address())),
				Some(17)
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::Erc20(erc20_address_not_exists())),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(12)
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				Some(17)
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				Some(17)
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				Some(17)
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decimals(CurrencyId::LiquidCrowdloan(0)),
				Some(12)
			);
		});
}

#[test]
fn encode_evm_address_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));

			// Token
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Token(TokenSymbol::ACA)),
				H160::from_str("0x0000000000000000000100000000000000000000").ok()
			);

			// Erc20
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address())),
				Some(erc20_address())
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address_not_exists())),
				Some(erc20_address_not_exists())
			);

			// DexShare
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				H160::from_str("0x0000000000000000000200000000000000000001").ok()
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				)),
				H160::from_str("0x00000000000000000002015dddfce50000000001").ok()
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::AUSD),
					DexShare::Erc20(erc20_address())
				)),
				H160::from_str("0x000000000000000000020000000001015dddfce5").ok()
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				)),
				H160::from_str("0x00000000000000000002015dddfce5015dddfce5").ok()
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address_not_exists())
				)),
				None
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::LiquidCrowdloan(1),
					DexShare::ForeignAsset(2)
				)),
				H160::from_str("0x0000000000000000000202000000010300000002").ok()
			);
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
					DexShare::ForeignAsset(2),
					DexShare::LiquidCrowdloan(1)
				)),
				H160::from_str("0x0000000000000000000203000000020200000001").ok()
			);

			// StableAssetPoolToken
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::StableAssetPoolToken(1)),
				H160::from_str("0x0000000000000000000300000000000000000001").ok()
			);

			// LiquidCrowdloan
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::LiquidCrowdloan(1)),
				H160::from_str("0x0000000000000000000400000000000000000001").ok()
			);

			// ForeignAsset
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::ForeignAsset(1)),
				H160::from_str("0x0000000000000000000500000000000000000001").ok()
			);
		});
}

#[test]
fn decode_evm_address_works() {
	ExtBuilder::default()
		.balances(vec![(alice(), 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			deploy_contracts();
			assert_ok!(AssetRegistry::register_erc20_asset(
				RuntimeOrigin::signed(CouncilAccount::get()),
				erc20_address(),
				1
			));

			// Token
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Token(TokenSymbol::ACA)).unwrap()
				),
				Some(CurrencyId::Token(TokenSymbol::ACA))
			);

			// Erc20
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address())).unwrap()
				),
				Some(CurrencyId::Erc20(erc20_address()))
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::Erc20(erc20_address_not_exists()))
						.unwrap()
				),
				None,
			);

			// DexShare
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Token(TokenSymbol::ACA),
						DexShare::Token(TokenSymbol::AUSD)
					))
					.unwrap(),
				),
				Some(CurrencyId::DexShare(
					DexShare::Token(TokenSymbol::ACA),
					DexShare::Token(TokenSymbol::AUSD)
				))
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Erc20(erc20_address()),
						DexShare::Token(TokenSymbol::AUSD)
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Token(TokenSymbol::AUSD)
				))
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::Erc20(erc20_address()),
						DexShare::Erc20(erc20_address())
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::Erc20(erc20_address()),
					DexShare::Erc20(erc20_address())
				))
			);

			// decode invalid evm address
			// CurrencyId::DexShare(DexShare::Token(TokenSymbol::ACA),
			// DexShare::Erc20(erc20_address_not_exists()))
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					H160::from_str("0x0000000000000000000000010000000002000001").unwrap()
				),
				None
			);

			// decode invalid evm address
			// CurrencyId::DexShare(DexShare::Erc20(erc20_address()),
			// DexShare::Erc20(erc20_address_not_exists()))
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					H160::from_str("0x0000000000000000000000010200000002000001").unwrap()
				),
				None
			);

			// Allow non-system contracts
			let non_system_contracts = H160::from_str("0x1000000000000000000000000000000000000000").unwrap();
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(non_system_contracts),
				Some(CurrencyId::Erc20(non_system_contracts))
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::LiquidCrowdloan(1),
						DexShare::ForeignAsset(2)
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::LiquidCrowdloan(1),
					DexShare::ForeignAsset(2)
				))
			);

			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::DexShare(
						DexShare::ForeignAsset(2),
						DexShare::LiquidCrowdloan(1),
					))
					.unwrap()
				),
				Some(CurrencyId::DexShare(
					DexShare::ForeignAsset(2),
					DexShare::LiquidCrowdloan(1)
				))
			);

			// StableAssetPoolToken
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::StableAssetPoolToken(1)).unwrap()
				),
				Some(CurrencyId::StableAssetPoolToken(1))
			);
			// LiquidCrowdloan
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::LiquidCrowdloan(1)).unwrap()
				),
				Some(CurrencyId::LiquidCrowdloan(1))
			);

			// ForeignAsset
			assert_eq!(
				EvmErc20InfoMapping::<Runtime>::decode_evm_address(
					EvmErc20InfoMapping::<Runtime>::encode_evm_address(CurrencyId::ForeignAsset(1)).unwrap()
				),
				Some(CurrencyId::ForeignAsset(1))
			);
		});
}
