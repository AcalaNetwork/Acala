// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

#![cfg(feature = "runtime-benchmarks")]

use crate::{
	AssetIds, AssetMetadata, BalanceOf, Call, Config, CurrencyId, EvmAddress, Location, Pallet, Parachain,
	VersionedLocation,
};
use frame_benchmarking::{benchmarks, impl_benchmark_test_suite, BenchmarkError};
use frame_support::{assert_ok, traits::Currency};
use frame_system::RawOrigin;
use module_support::AddressMapping;
use sp_runtime::traits::One;
use sp_std::{boxed::Box, str::FromStr, vec};

pub fn alice<T: Config + module_evm::Config>() -> T::AccountId {
	<T as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}
pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
}

pub fn dollar<T: Config>(amount: u32) -> BalanceOf<T> {
	BalanceOf::<T>::one() * 1_000_000u32.into() * 1_000_000u32.into() * amount.into()
}

pub fn deploy_contract<T: Config + module_evm::Config>() {
	<T as Config>::Currency::make_free_balance_be(&alice::<T>(), dollar::<T>(1000));

	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(module_evm::Pallet::<T>::create(
		RawOrigin::Signed(alice::<T>()).into(),
		code,
		0,
		2_100_000,
		1_000_000,
		vec![]
	));
}

benchmarks! {
	where_clause { where T: Config + module_evm::Config }
	register_foreign_asset {
		let location = VersionedLocation::V4(Location::new(
			0,
			[Parachain(1000)],
		));
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};
		let v3_location = xcm::v3::Location::try_from(location.clone()).map_err(|()| BenchmarkError::Weightless)?;
		let foreign_asset_id = 0;
	}: _(RawOrigin::Root, Box::new(location), Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::location_to_currency_ids(v3_location), Some(CurrencyId::ForeignAsset(foreign_asset_id)));
		assert_eq!(Pallet::<T>::foreign_asset_locations(foreign_asset_id), Some(v3_location));
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::ForeignAssetId(foreign_asset_id)), Some(asset_metadata));
	}

	update_foreign_asset {
		let location = VersionedLocation::V4(Location::new(
			0,
			[Parachain(1000)],
		));
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};
		let v3_location = xcm::v3::Location::try_from(location.clone()).map_err(|()| BenchmarkError::Weightless)?;
		let foreign_asset_id = 0;

		Pallet::<T>::register_foreign_asset(RawOrigin::Root.into(), Box::new(location.clone()), Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, 0, Box::new(location), Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::location_to_currency_ids(v3_location), Some(CurrencyId::ForeignAsset(foreign_asset_id)));
		assert_eq!(Pallet::<T>::foreign_asset_locations(foreign_asset_id), Some(v3_location));
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::ForeignAssetId(foreign_asset_id)), Some(asset_metadata));
	}

	register_stable_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};
		let stable_asset_id = 0;
	}: _(RawOrigin::Root, Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::StableAssetId(stable_asset_id)), Some(asset_metadata));
	}

	update_stable_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};
		let stable_asset_id = 0;

		Pallet::<T>::register_stable_asset(RawOrigin::Root.into(), Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, 0, Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::StableAssetId(stable_asset_id)), Some(asset_metadata));
	}

	register_erc20_asset {
		deploy_contract::<T>();
	}: _(RawOrigin::Root, erc20_address(), BalanceOf::<T>::one())
	verify {
		assert!(Pallet::<T>::asset_metadatas(AssetIds::Erc20(erc20_address())).is_some());
	}

	update_erc20_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};

		deploy_contract::<T>();
		Pallet::<T>::register_erc20_asset(RawOrigin::Root.into(), erc20_address(), BalanceOf::<T>::one())?;
	}: _(RawOrigin::Root, erc20_address(), Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::Erc20(erc20_address())), Some(asset_metadata));
	}

	register_native_asset {
		let currency_id = CurrencyId::LiquidCrowdloan(0);
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};
	}: _(RawOrigin::Root, currency_id, Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::NativeAssetId(currency_id)), Some(asset_metadata));
	}

	update_native_asset {
		let currency_id = CurrencyId::LiquidCrowdloan(0);
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: BalanceOf::<T>::one(),
		};

		Pallet::<T>::register_native_asset(RawOrigin::Root.into(), currency_id, Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, currency_id, Box::new(asset_metadata.clone()))
	verify {
		assert_eq!(Pallet::<T>::asset_metadatas(AssetIds::NativeAssetId(currency_id)), Some(asset_metadata));
	}
}

#[cfg(test)]
mod tests {
	use crate::mock::Runtime;
	use sp_io::TestExternalities;
	use sp_runtime::BuildStorage;

	pub fn new_test_ext() -> TestExternalities {
		let t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();
		TestExternalities::new(t)
	}
}

impl_benchmark_test_suite!(Pallet, crate::benchmarks::tests::new_test_ext(), crate::mock::Runtime);
