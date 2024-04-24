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

use crate::{AccountId, AssetRegistry, CurrencyId, Runtime, RuntimeOrigin, EVM};

use super::utils::{dollar, set_balance, NATIVE};
use frame_support::assert_ok;
use frame_system::RawOrigin;
use module_evm::EvmAddress;
use module_support::AddressMapping;
use orml_benchmarking::runtime_benchmarks;
use primitives::currency::AssetMetadata;
use sp_std::{boxed::Box, str::FromStr, vec};
use xcm::{prelude::*, v4::Location};

pub fn alice() -> AccountId {
	<Runtime as module_evm::Config>::AddressMapping::get_account_id(&alice_evm_addr())
}
pub fn alice_evm_addr() -> EvmAddress {
	EvmAddress::from_str("1000000000000000000000000000000000000001").unwrap()
}

pub fn erc20_address() -> EvmAddress {
	EvmAddress::from_str("0x5dddfce53ee040d9eb21afbc0ae1bb4dbb0ba643").unwrap()
}

pub fn deploy_contract() {
	//let alice_account = alice_account_id();
	set_balance(NATIVE, &alice(), 1_000_000 * dollar(NATIVE));

	let json: serde_json::Value =
		serde_json::from_str(include_str!("../../../../ts-tests/build/Erc20DemoContract2.json")).unwrap();
	let code = hex::decode(json.get("bytecode").unwrap().as_str().unwrap()).unwrap();

	assert_ok!(EVM::create(
		RuntimeOrigin::signed(alice()),
		code,
		0,
		2_100_000,
		1_000_000,
		vec![]
	));
}

runtime_benchmarks! {
	{ Runtime, module_asset_registry }

	register_foreign_asset {
		let location = VersionedLocation::V4(Location::new(
			0,
			[Parachain(1000)],
		));
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};
	}: _(RawOrigin::Root, Box::new(location), Box::new(asset_metadata))

	update_foreign_asset {
		let location = VersionedLocation::V4(Location::new(
			0,
			[Parachain(1000)],
		));
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

	register_erc20_asset {
		deploy_contract();
	}: _(RawOrigin::Root, erc20_address(), 1)

	update_erc20_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};

		deploy_contract();
		AssetRegistry::register_erc20_asset(RawOrigin::Root.into(), erc20_address(), 1)?;
	}: _(RawOrigin::Root, erc20_address(), Box::new(asset_metadata))

	register_native_asset {
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};
	}: _(RawOrigin::Root, CurrencyId::LiquidCrowdloan(0), Box::new(asset_metadata))

	update_native_asset {
		let currency_id = CurrencyId::LiquidCrowdloan(0);
		let asset_metadata = AssetMetadata {
			name: b"Token Name".to_vec(),
			symbol: b"TN".to_vec(),
			decimals: 12,
			minimal_balance: 1,
		};

		AssetRegistry::register_native_asset(RawOrigin::Root.into(), currency_id, Box::new(asset_metadata.clone()))?;
	}: _(RawOrigin::Root, currency_id, Box::new(asset_metadata))
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);
}
