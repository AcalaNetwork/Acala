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

//! Unit tests for the non-fungible-token module.

#![cfg(test)]

use super::*;
use frame_support::traits::Currency;
use frame_support::{assert_noop, assert_ok};
use mock::{Event, *};
use orml_nft::TokenInfo;
use primitives::Balance;
use sp_runtime::{traits::BlakeTwo256, ArithmeticError};
use sp_std::collections::btree_map::BTreeMap;

fn free_balance(who: &AccountId) -> Balance {
	<Runtime as pallet_proxy::Config>::Currency::free_balance(who)
}

fn reserved_balance(who: &AccountId) -> Balance {
	<Runtime as pallet_proxy::Config>::Currency::reserved_balance(who)
}

fn class_id_account() -> AccountId {
	<Runtime as Config>::PalletId::get().into_sub_account_truncating(CLASS_ID)
}

fn test_attr(x: u8) -> Attributes {
	let mut attr: Attributes = BTreeMap::new();
	attr.insert(vec![x, x + 10], vec![x, x + 1, x + 2]);
	attr.insert(vec![x + 1], vec![11]);
	attr
}

const TEST_ATTR_LEN: Balance = 7;

#[test]
fn create_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];

		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Default::default(),
			test_attr(1),
		));
		System::assert_last_event(Event::NFTModule(crate::Event::CreatedClass {
			owner: class_id_account(),
			class_id: CLASS_ID,
		}));

		let cls_deposit = CREATE_CLASS_DEPOSIT + DATA_DEPOSIT_PER_BYTE * ((metadata.len() as u128) + TEST_ATTR_LEN);

		assert_eq!(
			reserved_balance(&class_id_account()),
			cls_deposit + Proxy::deposit(1u32),
		);

		assert_eq!(
			orml_nft::Pallet::<Runtime>::classes(0).unwrap().data,
			ClassData {
				deposit: cls_deposit,
				properties: Default::default(),
				attributes: test_attr(1),
			}
		)
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
				Properties(ClassProperty::Transferable | ClassProperty::Burnable),
				Default::default(),
			),
			pallet_balances::Error::<Runtime, _>::InsufficientBalance
		);

		let mut large_attr: Attributes = BTreeMap::new();
		large_attr.insert(vec![1, 2, 3, 4, 5], vec![6, 7, 8, 9, 10, 11]);

		assert_noop!(
			NFTModule::create_class(
				Origin::signed(ALICE),
				metadata,
				Properties(ClassProperty::Transferable | ClassProperty::Burnable),
				large_attr,
			),
			Error::<Runtime>::AttributesTooLarge
		);
	});
}

#[test]
fn mint_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		let metadata_2 = vec![2, 3];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			test_attr(1),
		));
		System::assert_last_event(Event::NFTModule(crate::Event::CreatedClass {
			owner: class_id_account(),
			class_id: CLASS_ID,
		}));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			2 * (CREATE_TOKEN_DEPOSIT + ((metadata_2.len() as u128 + TEST_ATTR_LEN) * DATA_DEPOSIT_PER_BYTE))
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata_2.clone(),
			test_attr(2),
			2
		));
		System::assert_last_event(Event::NFTModule(crate::Event::MintedToken {
			from: class_id_account(),
			to: BOB,
			class_id: CLASS_ID,
			quantity: 2,
		}));
		assert_eq!(
			reserved_balance(&class_id_account()),
			CREATE_CLASS_DEPOSIT
				+ Proxy::deposit(1u32)
				+ DATA_DEPOSIT_PER_BYTE * (metadata.len() as u128 + TEST_ATTR_LEN)
		);
		assert_eq!(
			reserved_balance(&BOB),
			2 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE * (metadata_2.len() as u128 + TEST_ATTR_LEN))
		);
		assert_eq!(
			orml_nft::Pallet::<Runtime>::tokens(0, 0).unwrap(),
			TokenInfo {
				metadata: metadata_2.clone().try_into().unwrap(),
				owner: BOB,
				data: TokenData {
					deposit: CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE * (metadata_2.len() as u128 + TEST_ATTR_LEN),
					attributes: test_attr(2),
				}
			}
		);
		assert_eq!(
			orml_nft::Pallet::<Runtime>::tokens(0, 1).unwrap(),
			TokenInfo {
				metadata: metadata_2.clone().try_into().unwrap(),
				owner: BOB,
				data: TokenData {
					deposit: CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE * (metadata_2.len() as u128 + TEST_ATTR_LEN),
					attributes: test_attr(2),
				}
			}
		);
		assert_eq!(
			orml_nft::TokensByOwner::<Runtime>::iter_prefix((BOB,)).collect::<Vec<_>>(),
			vec![((0, 1), ()), ((0, 0), ())]
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_noop!(
			NFTModule::mint(
				Origin::signed(ALICE),
				BOB,
				CLASS_ID_NOT_EXIST,
				metadata.clone(),
				Default::default(),
				2
			),
			Error::<Runtime>::ClassIdNotFound
		);

		assert_noop!(
			NFTModule::mint(
				Origin::signed(BOB),
				BOB,
				CLASS_ID,
				metadata.clone(),
				Default::default(),
				0
			),
			Error::<Runtime>::InvalidQuantity
		);

		assert_noop!(
			NFTModule::mint(
				Origin::signed(BOB),
				BOB,
				CLASS_ID,
				metadata.clone(),
				Default::default(),
				2
			),
			Error::<Runtime>::NoPermission
		);

		orml_nft::NextTokenId::<Runtime>::mutate(CLASS_ID, |id| {
			*id = <Runtime as orml_nft::Config>::TokenId::max_value()
		});
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			2 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
		));
		assert_noop!(
			NFTModule::mint(
				Origin::signed(class_id_account()),
				BOB,
				CLASS_ID,
				metadata,
				Default::default(),
				2
			),
			orml_nft::Error::<Runtime>::NoAvailableTokenId
		);
	});
}

#[test]
fn mint_should_fail_without_mintable() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Default::default(),
			Default::default(),
		));

		assert_noop!(
			NFTModule::mint(
				Origin::signed(class_id_account()),
				BOB,
				CLASS_ID,
				metadata,
				Default::default(),
				2
			),
			Error::<Runtime>::NonMintable
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			2 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
			2
		));

		assert_eq!(
			reserved_balance(&BOB),
			2 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
		);

		assert_ok!(NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::TransferredToken {
			from: BOB,
			to: ALICE,
			class_id: CLASS_ID,
			token_id: TOKEN_ID,
		}));
		assert_eq!(
			reserved_balance(&BOB),
			1 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
		);
		assert_eq!(
			reserved_balance(&ALICE),
			1 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
		);

		assert_ok!(NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::TransferredToken {
			from: ALICE,
			to: BOB,
			class_id: CLASS_ID,
			token_id: TOKEN_ID,
		}));
		assert_eq!(
			reserved_balance(&BOB),
			2 * (CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE)
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
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
			Properties(ClassProperty::Mintable.into()),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			Default::default(),
			1
		));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		System::assert_last_event(Event::NFTModule(crate::Event::BurnedToken {
			owner: BOB,
			class_id: CLASS_ID,
			token_id: TOKEN_ID,
		}));
		assert_eq!(
			reserved_balance(&class_id_account()),
			CREATE_CLASS_DEPOSIT + Proxy::deposit(1u32) + DATA_DEPOSIT_PER_BYTE * (metadata.len() as u128)
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
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
			Properties(ClassProperty::Mintable.into()),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			Default::default(),
			1
		));

		let remark = "remark info".as_bytes().to_vec();
		let remark_hash = BlakeTwo256::hash(&remark[..]);
		assert_ok!(NFTModule::burn_with_remark(
			Origin::signed(BOB),
			(CLASS_ID, TOKEN_ID),
			remark
		));
		System::assert_last_event(Event::NFTModule(crate::Event::BurnedTokenWithRemark {
			owner: BOB,
			class_id: CLASS_ID,
			token_id: TOKEN_ID,
			remark_hash,
		}));

		assert_eq!(
			reserved_balance(&class_id_account()),
			CREATE_CLASS_DEPOSIT + Proxy::deposit(1u32) + DATA_DEPOSIT_PER_BYTE * (metadata.len() as u128)
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));

		let deposit = Proxy::deposit(1u32) + CREATE_CLASS_DEPOSIT + DATA_DEPOSIT_PER_BYTE * (metadata.len() as u128);
		assert_eq!(free_balance(&ALICE), 100000 - deposit);
		assert_eq!(reserved_balance(&ALICE), 0);
		assert_eq!(free_balance(&class_id_account()), 0);
		assert_eq!(reserved_balance(&class_id_account()), deposit);
		assert_eq!(free_balance(&BOB), 0);
		assert_eq!(reserved_balance(&BOB), 0);
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
			1
		));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		assert_ok!(NFTModule::destroy_class(
			Origin::signed(class_id_account()),
			CLASS_ID,
			ALICE
		));
		System::assert_last_event(Event::NFTModule(crate::Event::DestroyedClass {
			owner: class_id_account(),
			class_id: CLASS_ID,
		}));
		assert_eq!(free_balance(&class_id_account()), 0);
		assert_eq!(reserved_balance(&class_id_account()), 0);
		assert_eq!(free_balance(&ALICE), 100000);
		assert_eq!(reserved_balance(&ALICE), 0);
		assert_eq!(free_balance(&BOB), CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE);
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
			Properties(ClassProperty::Transferable | ClassProperty::Burnable | ClassProperty::Mintable),
			Default::default(),
		));
		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			1 * CREATE_TOKEN_DEPOSIT + DATA_DEPOSIT_PER_BYTE
		));
		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata,
			Default::default(),
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

#[test]
fn update_class_properties_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let metadata = vec![1];

		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			metadata.clone(),
			Properties(ClassProperty::Transferable | ClassProperty::ClassPropertiesMutable | ClassProperty::Mintable),
			Default::default(),
		));

		assert_ok!(Balances::deposit_into_existing(
			&class_id_account(),
			CREATE_TOKEN_DEPOSIT + ((metadata.len() as u128 + TEST_ATTR_LEN) * DATA_DEPOSIT_PER_BYTE)
		));

		assert_ok!(NFTModule::mint(
			Origin::signed(class_id_account()),
			BOB,
			CLASS_ID,
			metadata.clone(),
			Default::default(),
			1
		));

		assert_ok!(NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)));

		assert_ok!(NFTModule::update_class_properties(
			Origin::signed(class_id_account()),
			CLASS_ID,
			Properties(ClassProperty::ClassPropertiesMutable.into())
		));

		assert_noop!(
			NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NonTransferable
		);

		assert_ok!(NFTModule::update_class_properties(
			Origin::signed(class_id_account()),
			CLASS_ID,
			Properties(ClassProperty::Transferable.into())
		));

		assert_ok!(NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)));

		assert_noop!(
			NFTModule::update_class_properties(Origin::signed(class_id_account()), CLASS_ID, Default::default()),
			Error::<Runtime>::Immutable
		);

		assert_noop!(
			NFTModule::mint(
				Origin::signed(class_id_account()),
				BOB,
				CLASS_ID,
				metadata,
				Default::default(),
				1
			),
			Error::<Runtime>::NonMintable
		);
	});
}
