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

//! Cross-chain transfer tests within Kusama network.

use crate::integration_tests::*;
use crate::kusama_test_net::*;

use frame_support::assert_ok;
use xcm::v0::{
	Junction::{self, Parachain, Parent},
	MultiAsset::*,
	MultiLocation::*,
};

use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn transfer_from_relay_chain() {
	Kusama::execute_with(|| {
		assert_ok!(kusama_runtime::XcmPallet::reserve_transfer_assets(
			kusama_runtime::Origin::signed(ALICE.into()),
			X1(Parachain(2000)),
			X1(Junction::AccountId32 {
				id: BOB,
				network: NetworkId::Any
			}),
			vec![ConcreteFungible {
				id: Null,
				amount: dollar(KSM)
			}],
			600_000_000
		));
	});

	Karura::execute_with(|| {
		assert_eq!(Tokens::free_balance(KSM, &AccountId::from(BOB)), 999_952_000_000);
	});
}

#[test]
fn transfer_to_relay_chain() {
	Karura::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			KSM,
			dollar(KSM),
			X2(
				Parent,
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any,
				}
			),
			3_000_000_000
		));
	});

	Kusama::execute_with(|| {
		assert_eq!(
			kusama_runtime::Balances::free_balance(&AccountId::from(BOB)),
			999_920_000_005
		);
	});
}
