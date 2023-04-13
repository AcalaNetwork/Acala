// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

use crate::relaychain::polkadot_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use module_xcm_interface::XcmInterfaceOperation;
use sp_runtime::traits::StaticLookup;
use xcm_emulator::TestExt;

const ACALA_PARA_ID: u32 = 2000;

//TODO: Enable after Polkadot runtime allows XCM proxy calls:
// https://github.com/paritytech/polkadot/blob/5c554b95e223b507a9b7e420e2cdee06e0982ab0/runtime/polkadot/src/xcm_config.rs#L167
#[ignore = "polkadot runtime does not allow XCM proxy calls"]
#[test]
fn transfer_from_crowdloan_vault_works() {
	TestNet::reset();

	let vault = acala_runtime::CrowdloanVault::get();
	let module_liquid_crowdloan_account = acala_runtime::LiquidCrowdloan::account_id();
	let acala_sovereign_account: AccountId = ParaId::from(ACALA_PARA_ID).into_account_truncating();

	PolkadotNet::execute_with(|| {
		use polkadot_runtime::{Balances, Proxy, ProxyType, Runtime, RuntimeOrigin};

		let _ = Balances::deposit_creating(&vault, dollar(DOT) * 100);

		assert_ok!(Proxy::add_proxy(
			RuntimeOrigin::signed(vault.clone()),
			<Runtime as frame_system::Config>::Lookup::unlookup(acala_sovereign_account.clone()),
			ProxyType::Any,
			0
		));

		// NOTE: the following code is to help debugging via duplicating the XCM transact in
		// Polkadot runtime. Feel free to delete it after Polkadot runtime allows XCM proxy calls
		// and the test can be enabled.

		// let call = RuntimeCall::XcmPallet(pallet_xcm::Call::reserve_transfer_assets {
		// 	dest: Box::new(Parachain(2000).into_versioned()),
		// 	beneficiary: Box::new(
		// 		Junction::AccountId32 {
		// 			id: module_liquid_crowdloan_account.clone().into(),
		// 			network: None
		// 		}
		// 		.into_versioned()
		// 	),
		// 	assets: Box::new((Here, dollar(DOT)).into()),
		// 	fee_asset_item: 0,
		// });
		// assert_ok!(Proxy::proxy(
		// 	RuntimeOrigin::signed(acala_sovereign_account.clone()),
		// 	<Runtime as frame_system::Config>::Lookup::unlookup(vault.clone()),
		// 	None,
		// 	Box::new(call),
		// ));
	});

	Acala::execute_with(|| {
		use acala_runtime::{LiquidCrowdloan, RuntimeOrigin, XcmInterface};

		assert_ok!(XcmInterface::update_xcm_dest_weight_and_fee(
			RuntimeOrigin::root(),
			vec![(
				XcmInterfaceOperation::ProxyReserveTransferAssets,
				Some(XcmWeight::from_ref_time(20_000_000_000)),
				Some(100_000_000_000)
			)]
		));

		assert_ok!(LiquidCrowdloan::transfer_from_crowdloan_vault(
			RuntimeOrigin::root(),
			dollar(DOT),
		));

		assert_eq!(
			Tokens::free_balance(DOT, &module_liquid_crowdloan_account),
			9_998_397_440,
		);
	});
}
