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

//! Unit tests for the partner's module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, error::BadOrigin};
use mock::*;

#[test]
fn register_partner_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 10000);
		assert_ok!(Partners::register_partner(
			Origin::signed(ALICE),
			b"meta".to_vec().try_into().unwrap()
		));

		// Takes registration fee and proxy fee
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 9898);
		assert_eq!(NextId::<Runtime>::get(), 1_u32);
		assert_eq!(PartnerMetadata::<Runtime>::get(0).unwrap(), b"meta".to_vec());
	});
}

#[test]
fn set_referral_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Partners::set_referral(Origin::signed(ALICE), 0));
		assert_eq!(
			Referral::<Runtime>::get(ALICE).unwrap(),
			ReferralInfo {
				partner_id: 0,
				expiry: 2
			}
		);
		System::set_block_number(3);

		assert_ok!(Partners::set_referral(Origin::signed(ALICE), 1));
		assert_eq!(
			Referral::<Runtime>::get(ALICE).unwrap(),
			ReferralInfo {
				partner_id: 1,
				expiry: 5
			}
		);
	});
}

#[test]
fn update_partner_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Partners::update_partner_metadata(Origin::signed(BOB), 0, b"meta".to_vec().try_into().unwrap()),
			BadOrigin
		);
		assert_ok!(Partners::update_partner_metadata(
			Origin::signed(ALICE),
			0,
			b"meta".to_vec().try_into().unwrap()
		));

		assert_eq!(PartnerMetadata::<Runtime>::get(0).unwrap(), b"meta".to_vec());
	});
}

#[test]
fn feeless_register_partner_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 10000);
		assert_ok!(Partners::admin_register_partner(
			Origin::signed(ALICE),
			ALICE,
			b"meta".to_vec().try_into().unwrap()
		));

		// Takes only proxy fee
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 9998);
		assert_eq!(NextId::<Runtime>::get(), 1_u32);
		assert_eq!(PartnerMetadata::<Runtime>::get(0).unwrap(), b"meta".to_vec());
	});
}

#[test]
fn on_fee_deposited_works() {
	ExtBuilder::default().build().execute_with(|| {
		let treasury_account = TreasuryAccount::get();
		let sub_account = PartnersPalletId::get().try_into_sub_account(0).unwrap();
		assert_ok!(Partners::admin_register_partner(
			Origin::signed(ALICE),
			ALICE,
			b"meta".to_vec().try_into().unwrap()
		));
		assert_ok!(Partners::set_referral(Origin::signed(ALICE), 0));
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 9998);

		// sends fee to sub account
		Partners::on_fee_deposited(&ALICE, ACA, 100);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 9898);
		assert_eq!(Currencies::free_balance(ACA, &treasury_account), 0);
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 100);
		System::set_block_number(3);

		// sends fee to treasury when referral expires
		Partners::on_fee_deposited(&ALICE, ACA, 100);
		assert_eq!(Currencies::free_balance(ACA, &ALICE), 9798);
		assert_eq!(Currencies::free_balance(ACA, &treasury_account), 100);
		assert_eq!(Currencies::free_balance(ACA, &sub_account), 100);
	});
}
