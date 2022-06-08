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

//! Cross-chain transfer tests within Polkadot network.

use crate::relaychain::polkadot_test_net::*;
use crate::setup::*;

use frame_support::assert_ok;
use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

#[test]
fn transfer_from_relay_chain() {
	PolkadotNet::execute_with(|| {
		assert_ok!(polkadot_runtime::XcmPallet::reserve_transfer_assets(
			polkadot_runtime::Origin::signed(ALICE.into()),
			Box::new(Parachain(2000).into().into()),
			Box::new(
				Junction::AccountId32 {
					id: BOB,
					network: NetworkId::Any
				}
				.into()
				.into()
			),
			Box::new((Here, dollar(DOT)).into()),
			0
		));
	});

	Acala::execute_with(|| {
		assert_eq!(9_998_135_200, Tokens::free_balance(DOT, &AccountId::from(BOB)));
	});
}

#[test]
fn transfer_to_relay_chain() {
	Acala::execute_with(|| {
		assert_ok!(XTokens::transfer(
			Origin::signed(ALICE.into()),
			DOT,
			5 * dollar(DOT),
			Box::new(
				MultiLocation::new(
					1,
					X1(Junction::AccountId32 {
						id: BOB,
						network: NetworkId::Any,
					})
				)
				.into()
			),
			4_000_000_000
		));
	});

	PolkadotNet::execute_with(|| {
		assert_eq!(
			// v0.9.19: 49_517_228_896
			// v0.9.22: 49_530_582_548
			49_530_582_548,
			polkadot_runtime::Balances::free_balance(&AccountId::from(BOB))
		);
		assert_eq!(
			5 * dollar(DOT),
			polkadot_runtime::Balances::free_balance(&ParaId::from(2000).into_account_truncating())
		);
	});
}
