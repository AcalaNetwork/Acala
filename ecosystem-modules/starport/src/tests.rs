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

//! Unit tests for the Starport Module

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	AccountId, Currencies, Event, ExtBuilder, Origin, Runtime, Starport, StarportPalletId, System, ACALA, ALICE, BOB,
	CASH, GATEWAY_ACCOUNT, INITIAL_BALANCE, KSM,
};

#[test]
fn mock_initialize_token_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Currencies::free_balance(KSM, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(CASH, &ALICE), INITIAL_BALANCE);
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), INITIAL_BALANCE);
	});
}

#[test]
fn lock_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup supply caps
		SupplyCaps::<Runtime>::insert(ACALA, INITIAL_BALANCE);
		SupplyCaps::<Runtime>::insert(CASH, INITIAL_BALANCE);

		// Lock some ACALA
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, INITIAL_BALANCE));

		// Locked ACALA are transferred from the user's account into Admin's account.
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), 0);
		assert_eq!(
			Currencies::free_balance(ACALA, &StarportPalletId::get().into_account_truncating()),
			INITIAL_BALANCE
		);

		// Supply caps are reduced accordingly.
		assert_eq!(SupplyCaps::<Runtime>::get(ACALA), 0);
		assert_eq!(SupplyCaps::<Runtime>::get(CASH), INITIAL_BALANCE);

		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: INITIAL_BALANCE,
				user: ALICE
			})
		);

		// Locked CASH assets are burned instead
		assert_ok!(Starport::lock(Origin::signed(ALICE), CASH, INITIAL_BALANCE));

		// Locked ACALA are transferred from the user's account into Admin's account.
		assert_eq!(Currencies::free_balance(CASH, &ALICE), 0);
		assert_eq!(
			Currencies::free_balance(CASH, &StarportPalletId::get().into_account_truncating()),
			0
		);

		// Supply caps are reduced accordingly.
		assert_eq!(SupplyCaps::<Runtime>::get(CASH), 0);

		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: CASH,
				amount: INITIAL_BALANCE,
				user: ALICE
			})
		)
	});
}

#[test]
fn lock_to_works() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup supply caps
		SupplyCaps::<Runtime>::insert(ACALA, INITIAL_BALANCE);

		// Lock some ACALA into BOB's account
		assert_ok!(Starport::lock_to(Origin::signed(ALICE), BOB, ACALA, INITIAL_BALANCE));

		// Locked ACALA are transferred from the user's account into Admin's account.
		assert_eq!(Currencies::free_balance(ACALA, &ALICE), 0);
		assert_eq!(
			Currencies::free_balance(ACALA, &StarportPalletId::get().into_account_truncating()),
			INITIAL_BALANCE
		);
		// Supply caps are reduced accordingly.
		assert_eq!(SupplyCaps::<Runtime>::get(ACALA), 0);

		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: INITIAL_BALANCE,
				user: BOB
			})
		);
	});
}

#[test]
fn lock_to_fails_with_insufficient_balance() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup supply caps
		SupplyCaps::<Runtime>::insert(ACALA, INITIAL_BALANCE);

		// Lock some ACALA into BOB's account
		assert_noop!(
			Starport::lock_to(Origin::signed(BOB), ALICE, ACALA, INITIAL_BALANCE),
			module_currencies::Error::<Runtime>::BalanceTooLow
		);
	});
}

#[test]
fn lock_to_fails_with_insufficient_supply_caps() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup supply caps
		SupplyCaps::<Runtime>::insert(ACALA, INITIAL_BALANCE);
		SupplyCaps::<Runtime>::insert(KSM, INITIAL_BALANCE - 1);

		// Lock works if the amount is below the market cap
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, INITIAL_BALANCE - 1));

		// Lock fails due to insufficient Market cap
		assert_noop!(
			Starport::lock(Origin::signed(ALICE), KSM, INITIAL_BALANCE),
			Error::<Runtime>::InsufficientAssetSupplyCap
		);
	});
}

#[test]
fn invoke_can_set_supply_cap() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup initial caps
		SupplyCaps::<Runtime>::insert(ACALA, 100);

		// Lock some ACALA so the supply cap is spent.
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, 100));
		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: 100,
				user: ALICE
			})
		);

		// Lock fails due to insufficient Market cap
		assert_noop!(
			Starport::lock(Origin::signed(ALICE), ACALA, 100),
			Error::<Runtime>::InsufficientAssetSupplyCap
		);

		// Increase the supply cap via Notice invoke.
		let notice = GatewayNotice::new(0, GatewayNoticePayload::SetSupplyCap(ACALA, 100));
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice,
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::SupplyCapSet {
				currency_id: ACALA,
				new_cap: 100
			})
		);

		// Lock will now work
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, 100));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: 100,
				user: ALICE
			})
		);
	});
}

#[test]
fn invoke_can_set_authorities() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup initial caps
		SupplyCaps::<Runtime>::insert(ACALA, 1000);

		// Lock some ACALA so the supply cap is spent.
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, 100));
		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: 100,
				user: ALICE
			})
		);

		let new_authorities = vec![AccountId::new([0xA0; 32]), AccountId::new([0xA1; 32])];

		let mut notice = GatewayNotice::new(0, GatewayNoticePayload::ChangeAuthorities(new_authorities.clone()));
		let bad_notice = GatewayNotice::new(1, GatewayNoticePayload::ChangeAuthorities(vec![]));

		// Incorrect authority signatures will fail the Invoke call
		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), notice.clone(), new_authorities.clone()),
			Error::<Runtime>::InsufficientValidNoticeSignatures
		);

		// Empty authority will fail
		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), bad_notice, mock::get_mock_signatures()),
			Error::<Runtime>::AuthoritiesListCannotBeEmpty
		);

		// Change authority via Notice invoke.
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::GatewayAuthoritiesChanged)
		);

		// Notices now uses the new set of authority for verification.
		notice.id = 2;
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice,
			new_authorities.clone()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::GatewayAuthoritiesChanged)
		);

		// invocation fails with too many authorities
		notice = GatewayNotice::new(
			3,
			GatewayNoticePayload::ChangeAuthorities(vec![
				AccountId::new([0x00; 32]),
				AccountId::new([0x01; 32]),
				AccountId::new([0x02; 32]),
				AccountId::new([0x03; 32]),
				AccountId::new([0x04; 32]),
				AccountId::new([0x05; 32]),
			]),
		);
		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), notice, new_authorities),
			Error::<Runtime>::ExceededMaxNumberOfAuthorities
		);
	});
}

#[test]
fn invoke_can_unlock_asset() {
	ExtBuilder::default().build().execute_with(|| {
		// Setup initial caps
		SupplyCaps::<Runtime>::insert(ACALA, 1000);

		// Lock some ACALA so the supply cap is spent.
		assert_ok!(Starport::lock(Origin::signed(ALICE), ACALA, 500));
		// Verify the event deposited for Gateway is correct.
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetLockedTo {
				currency_id: ACALA,
				amount: 500,
				user: ALICE
			})
		);

		// Unlock the locked asset
		let mut notice = GatewayNotice::new(
			0,
			GatewayNoticePayload::Unlock {
				currency_id: ACALA,
				amount: 500,
				who: ALICE,
			},
		);
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetUnlocked {
				currency_id: ACALA,
				amount: 500,
				user: ALICE
			})
		);

		// Unlock will fail with insufficient asset
		notice.id = 1;
		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), notice, mock::get_mock_signatures()),
			Error::<Runtime>::InsufficientAssetToUnlock
		);

		let notice_fail = GatewayNotice::new(
			0,
			GatewayNoticePayload::Unlock {
				currency_id: KSM,
				amount: 100,
				who: ALICE,
			},
		);
		assert_noop!(
			Starport::invoke(
				Origin::signed(GATEWAY_ACCOUNT),
				notice_fail,
				mock::get_mock_signatures()
			),
			Error::<Runtime>::InsufficientAssetToUnlock
		);

		// CASH asset is Minted
		let notice_cash = GatewayNotice::new(
			0,
			GatewayNoticePayload::Unlock {
				currency_id: CASH,
				amount: 100000,
				who: ALICE,
			},
		);
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice_cash,
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::AssetUnlocked {
				currency_id: CASH,
				amount: 100000,
				user: ALICE
			})
		);
	});
}

#[test]
fn invoke_can_set_future_cash_yield() {
	ExtBuilder::default().build().execute_with(|| {
		let notice = GatewayNotice::new(
			0,
			GatewayNoticePayload::SetFutureYield {
				next_cash_yield: 1000,
				next_cash_yield_index: 0,
				next_cash_yield_start: 0,
			},
		);
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice,
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);
	});
}

#[test]
fn notices_cannot_be_invoked_twice() {
	ExtBuilder::default().build().execute_with(|| {
		let notice = GatewayNotice::new(
			0,
			GatewayNoticePayload::SetFutureYield {
				next_cash_yield: 1000,
				next_cash_yield_index: 0,
				next_cash_yield_start: 0,
			},
		);
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);

		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), notice, mock::get_mock_signatures()),
			Error::<Runtime>::NoticeAlreadyInvoked
		);
	});
}

#[test]
fn notices_are_invoked_by_any_account() {
	ExtBuilder::default().build().execute_with(|| {
		let mut notice = GatewayNotice::new(
			0,
			GatewayNoticePayload::SetFutureYield {
				next_cash_yield: 1000,
				next_cash_yield_index: 0,
				next_cash_yield_start: 0,
			},
		);
		assert_ok!(Starport::invoke(
			Origin::signed(ALICE),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);

		notice.id = 1;
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);

		notice.id = 2;
		assert_ok!(Starport::invoke(
			Origin::signed(BOB),
			notice,
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);
	});
}

#[test]
fn notices_can_only_be_invoked_with_enough_signatures() {
	ExtBuilder::default().build().execute_with(|| {
		let mut notice = GatewayNotice::new(
			0,
			GatewayNoticePayload::SetFutureYield {
				next_cash_yield: 1000,
				next_cash_yield_index: 0,
				next_cash_yield_start: 0,
			},
		);
		let mut signer = mock::get_mock_signatures();
		signer.pop();

		// Mock requires atleast 50% of the 3 signers - so 2 signatures is sufficient.
		assert_ok!(Starport::invoke(
			Origin::signed(GATEWAY_ACCOUNT),
			notice.clone(),
			mock::get_mock_signatures()
		));
		assert_eq!(
			System::events().iter().last().unwrap().event,
			Event::Starport(crate::Event::FutureYieldSet {
				yield_amount: 1000,
				index: 0,
				timestamp: 0
			})
		);

		// 1 signer is insufficient authorisation
		notice.id = 1;
		signer.pop();
		assert_noop!(
			Starport::invoke(Origin::signed(GATEWAY_ACCOUNT), notice, signer),
			Error::<Runtime>::InsufficientValidNoticeSignatures
		);
	});
}
