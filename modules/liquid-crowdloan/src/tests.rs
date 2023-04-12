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

//! Unit tests for example module.

#![cfg(test)]

use super::*;
use crate::mock::*;
use frame_support::{assert_err, assert_ok};
use orml_traits::MultiCurrency;
use sp_runtime::traits::BadOrigin;

#[test]
fn redeem_works() {
	ExtBuilder::default()
		.balances(vec![(BOB, LDOT, 100), (LiquidCrowdloan::account_id(), DOT, 100)])
		.build()
		.execute_with(|| {
			assert_ok!(LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100));
			assert_eq!(Currencies::free_balance(LDOT, &BOB), 0);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 100);
			assert_eq!(Currencies::free_balance(DOT, &LiquidCrowdloan::account_id()), 0);
			System::assert_last_event(RuntimeEvent::LiquidCrowdloan(crate::Event::Redeemed { amount: 100 }));
		});
}

#[test]
fn redeem_fails_if_not_enough_liquid_crowdloan_token() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100),
			orml_tokens::Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn redeem_fails_if_not_enough_relay_chain_token() {
	ExtBuilder::default()
		.balances(vec![(BOB, LDOT, 100)])
		.build()
		.execute_with(|| {
			assert_err!(
				LiquidCrowdloan::redeem(RuntimeOrigin::signed(BOB), 100),
				orml_tokens::Error::<Runtime>::BalanceTooLow
			);
		});
}

#[test]
fn transfer_from_crowdloan_vault_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(LiquidCrowdloan::transfer_from_crowdloan_vault(
			RuntimeOrigin::signed(ALICE),
			100,
		));
		System::assert_last_event(RuntimeEvent::LiquidCrowdloan(
			crate::Event::TransferFromCrowdloanVaultRequested { amount: 100 },
		));
	});
}

#[test]
fn transfer_from_crowdloan_vault_fails_if_not_gov_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_err!(
			LiquidCrowdloan::transfer_from_crowdloan_vault(RuntimeOrigin::signed(BOB), 100,),
			BadOrigin
		);
	});
}

#[test]
fn transfer_from_crowdloan_vault_fails_if_sending_xcm_failed() {
	ExtBuilder::default().transfer_ok(false).build().execute_with(|| {
		assert_err!(
			LiquidCrowdloan::transfer_from_crowdloan_vault(RuntimeOrigin::signed(ALICE), 100,),
			DispatchError::Other("transfer failed")
		);
	})
}
