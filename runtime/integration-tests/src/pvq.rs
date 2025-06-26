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

use crate::setup::*;
use primitives::currency::AssetMetadata;
use pvq_primitives::PvqResult;
use runtime_common::RELAY_CHAIN_SLOT_DURATION_MILLIS;
use serde_json::de;
use sp_core::H160;
use xcm::{prelude::*, v4::Location};

#[test]
fn test_pvq_get_liquidity_pool() {
	use pvq_runtime_api::runtime_decl_for_pvq_api::PvqApi;
	ExtBuilder::default()
		.balances(vec![
			(
				MockAddressMapping::get_account_id(&H160::from_low_u64_be(0)),
				NATIVE_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				USD_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(
				AccountId::from(ALICE),
				RELAY_CHAIN_CURRENCY,
				1_000_000_000 * dollar(NATIVE_CURRENCY),
			),
			(AccountId::from(BOB), USD_CURRENCY, 1_000_000 * dollar(NATIVE_CURRENCY)),
			(
				AccountId::from(BOB),
				RELAY_CHAIN_CURRENCY,
				1_000_000 * dollar(NATIVE_CURRENCY),
			),
		])
		.build()
		.execute_with(|| {
			// First add liquidity to create a pool
			assert_ok!(Dex::add_liquidity(
				RuntimeOrigin::signed(AccountId::from(ALICE)),
				RELAY_CHAIN_CURRENCY,
				USD_CURRENCY,
				10_000 * dollar(RELAY_CHAIN_CURRENCY),
				10_000_000 * dollar(USD_CURRENCY),
				0,
				false,
			));

			assert_eq!(
				Dex::get_liquidity_pool(RELAY_CHAIN_CURRENCY, USD_CURRENCY),
				(10_000 * dollar(RELAY_CHAIN_CURRENCY), 10_000_000 * dollar(USD_CURRENCY))
			);

			let program = include_bytes!("../fixtures/guest-swap-info.polkavm");

			// test get_liquidity_pool
			let mut args = vec![2u8];
			args.extend_from_slice(&(RELAY_CHAIN_CURRENCY.encode(), USD_CURRENCY.encode()).encode());

			let result: Vec<u8> =
				Runtime::execute_query(program.to_vec(), args, None).expect("Failed to execute query");

			assert_eq!(
				result,
				Some((10_000 * dollar(RELAY_CHAIN_CURRENCY), 10_000_000 * dollar(USD_CURRENCY))).encode()
			);
		});
}
