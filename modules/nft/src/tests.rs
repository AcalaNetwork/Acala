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

//! Unit tests for the non-fungible-token module.

#![cfg(test)]

use super::*;
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use orml_nft::TokenInfo;
use primitives::Balance;
use sp_runtime::{traits::BlakeTwo256, ArithmeticError};
use sp_std::convert::TryInto;

fn free_balance(who: &AccountId) -> Balance {
	<Runtime as pallet_proxy::Config>::Currency::free_balance(who)
}

fn reserved_balance(who: &AccountId) -> Balance {
	<Runtime as pallet_proxy::Config>::Currency::reserved_balance(who)
}

fn class_id_account() -> AccountId {
	<Runtime as Config>::PalletId::get().into_sub_account(CLASS_ID)
}

#[test]
fn create_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Default::default()
		));
		System::assert_last_event(Event::NFTModule(crate::Event::CreatedClass(
			class_id_account(),
			CLASS_ID,
		)));
		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Config>::CreateClassDeposit::get()
				+ Proxy::deposit(1u32)
				+ <Runtime as Config>::DataDepositPerByte::get() * (metadata.len() as u128)
		);
	});
}

#[test]
fn create_class_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_noop!(
			NFTModule::create_class(
				Origin::signed(BOB),
				metadata.clone(),
				Properties(ClassProperty::Transferable | ClassProperty::Burnable)
			),
			pallet_balances::Error::<Runtime, _>::InsufficientBalance
		);
	});
}

#[test]
fn mint_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		let metadata_2 = vec![2];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		System::assert_last_event(Event::NFTModule(crate::Event::CreatedClass(
			class_id_account(),
			CLASS_ID,
		)));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 2 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata_2.clone(),
			2
		));
		System::assert_last_event(Event::NFTModule(crate::Event::MintedToken(
			class_id_account(),
			BOB,
			CLASS_ID,
			2,
		)));
		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Config>::CreateClassDeposit::get()
				+ Proxy::deposit(1u32)
				+ <Runtime as Config>::DataDepositPerByte::get() * (metadata.len() as u128)
		);
		assert_eq!(
			reserved_balance(&BOB),
			2 * <Runtime as Config>::CreateTokenDeposit::get()
		);
		assert_eq!(
			orml_nft::Pallet::<Runtime>::tokens(0, 0).unwrap(),
			TokenInfo {
				metadata: metadata_2.clone().try_into().unwrap(),
				owner: BOB,
				data: TokenData {
					deposit: <Runtime as Config>::CreateTokenDeposit::get()
				}
			}
		);
		assert_eq!(
			orml_nft::Pallet::<Runtime>::tokens(0, 1).unwrap(),
			TokenInfo {
				metadata: metadata_2.clone().try_into().unwrap(),
				owner: BOB,
				data: TokenData {
					deposit: <Runtime as Config>::CreateTokenDeposit::get()
				}
			}
		);
	});
}

#[test]
fn mint_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_noop!(
			NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID_NOT_EXIST, metadata.clone(), 2),
			Error::<Runtime>::ClassIdNotFound
		);

		assert_noop!(
			NFTModule::mint(Origin::signed(BOB), BOB, CLASS_ID, metadata.clone(), 0),
			Error::<Runtime>::InvalidQuantity
		);

		assert_noop!(
			NFTModule::mint(Origin::signed(BOB), BOB, CLASS_ID, metadata.clone(), 2),
			Error::<Runtime>::NoPermission
		);

		orml_nft::NextTokenId::<Runtime>::mutate(CLASS_ID, |id| {
			*id = <Runtime as orml_nft::Config>::TokenId::max_value()
		});
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 2 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_noop!(
			NFTModule::mint(Origin::signed(class_id_account()), BOB, CLASS_ID, metadata.clone(), 2),
			orml_nft::Error::<Runtime>::NoAvailableTokenId
		);
	});
}

#[test]
fn transfer_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 2 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			2
		));

		assert_eq!(
			reserved_balance(&BOB),
			2 * <Runtime as Config>::CreateTokenDeposit::get()
		);

		assert_ok!(NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::TransferredToken(
			BOB, ALICE, CLASS_ID, TOKEN_ID,
		)));
		assert_eq!(
			reserved_balance(&BOB),
			1 * <Runtime as Config>::CreateTokenDeposit::get()
		);
		assert_eq!(
			reserved_balance(&ALICE),
			1 * <Runtime as Config>::CreateTokenDeposit::get()
		);

		assert_ok!(NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::TransferredToken(
			ALICE, BOB, CLASS_ID, TOKEN_ID,
		)));
		assert_eq!(
			reserved_balance(&BOB),
			2 * <Runtime as Config>::CreateTokenDeposit::get()
		);
		assert_eq!(reserved_balance(&ALICE), 0);
	});
}

#[test]
fn transfer_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_noop!(
			NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID_NOT_EXIST, TOKEN_ID)),
			Error::<Runtime>::ClassIdNotFound
		);
		assert_noop!(
			NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID_NOT_EXIST)),
			Error::<Runtime>::TokenIdNotFound
		);
		assert_noop!(
			NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)),
			orml_nft::Error::<Runtime>::NoPermission
		);
	});

	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Default::default()
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_noop!(
			NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NonTransferable
		);
	});
}

#[test]
fn burn_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::BurnedToken(BOB, CLASS_ID, TOKEN_ID)));
		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Config>::CreateClassDeposit::get()
				+ Proxy::deposit(1u32)
				+ <Runtime as Config>::DataDepositPerByte::get() * (metadata.len() as u128)
		);
	});
}

#[test]
fn burn_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_noop!(
			NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID_NOT_EXIST)),
			Error::<Runtime>::TokenIdNotFound
		);

		assert_noop!(
			NFTModule::burn(Origin::signed(ALICE), (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NoPermission
		);

		orml_nft::Classes::<Runtime>::mutate(CLASS_ID, |class_info| {
			class_info.as_mut().unwrap().total_issuance = 0;
		});
		assert_noop!(
			NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)),
			ArithmeticError::Overflow,
		);
	});

	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Default::default()
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_noop!(
			NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NonBurnable
		);
	});
}

#[test]
fn burn_with_remark_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));

		let remark = "remark info".as_bytes().to_vec();
		let remark_hash = BlakeTwo256::hash(&remark[..]);
		assert_ok!(NFTModule::burn_with_remark(
			Origin::signed(BOB),
			(CLASS_ID, TOKEN_ID),
			remark
		));
		System::assert_last_event(Event::NFTModule(crate::Event::BurnedTokenWithRemark(
			BOB,
			CLASS_ID,
			TOKEN_ID,
			remark_hash,
		)));

		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Config>::CreateClassDeposit::get()
				+ Proxy::deposit(1u32)
				+ <Runtime as Config>::DataDepositPerByte::get() * (metadata.len() as u128)
		);
	});
}

#[test]
fn destroy_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));

		let deposit = Proxy::deposit(1u32)
			+ <Runtime as Config>::CreateClassDeposit::get()
			+ <Runtime as Config>::DataDepositPerByte::get() * (metadata.len() as u128);
		assert_eq!(free_balance(&ALICE), 100000 - deposit);
		assert_eq!(reserved_balance(&ALICE), 0);
		assert_eq!(free_balance(&class_id_account()), 0);
		assert_eq!(reserved_balance(&class_id_account()), deposit);
		assert_eq!(free_balance(&BOB), 0);
		assert_eq!(reserved_balance(&BOB), 0);
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * <Runtime as Config>::CreateTokenDeposit::get()
		)); // + 100
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		assert_ok!(NFTModule::destroy_class(
			Origin::signed(class_id_account()),
			CLASS_ID,
			ALICE
		));
		System::assert_last_event(Event::NFTModule(crate::Event::DestroyedClass(
			class_id_account(),
			CLASS_ID,
		)));
		assert_eq!(free_balance(&class_id_account()), 0);
		assert_eq!(reserved_balance(&class_id_account()), 0);
		assert_eq!(free_balance(&ALICE), 100000);
		assert_eq!(reserved_balance(&ALICE), 0);
		assert_eq!(free_balance(&BOB), <Runtime as Config>::CreateTokenDeposit::get());
		assert_eq!(reserved_balance(&BOB), 0);
	});
}

#[test]
fn destroy_class_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_eq!(
			Balances::deposit_into_existing(&class_id_account(), 1 * <Runtime as Config>::CreateTokenDeposit::get())
				.is_ok(),
			true
		);
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			1
		));
		assert_noop!(
			NFTModule::destroy_class(Origin::signed(class_id_account()), CLASS_ID_NOT_EXIST, BOB),
			Error::<Runtime>::ClassIdNotFound
		);

		assert_noop!(
			NFTModule::destroy_class(Origin::signed(BOB), CLASS_ID, BOB),
			Error::<Runtime>::NoPermission
		);

		assert_noop!(
			NFTModule::destroy_class(Origin::signed(class_id_account()), CLASS_ID, BOB),
			Error::<Runtime>::CannotDestroyClass
		);

		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));

		assert_noop!(
			NFTModule::destroy_class(Origin::signed(class_id_account()), CLASS_ID, BOB),
			pallet_proxy::Error::<Runtime>::NotFound
		);

		assert_ok!(NFTModule::destroy_class(
			Origin::signed(class_id_account()),
			CLASS_ID,
			ALICE
		));
	});
}
