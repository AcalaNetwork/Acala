//! Unit tests for the non-fungible-token module.

#![cfg(test)]

use super::*;
use frame_support::StorageMap;
use frame_support::{assert_noop, assert_ok};
use mock::{
	last_event, AccountId, ExtBuilder, NFTModule, Origin, Runtime, TestEvent, ALICE, BOB, CLASS_ID, CLASS_ID_NOT_EXIST,
	TOKEN_ID, TOKEN_ID_NOT_EXIST,
};

fn free_balance(who: &AccountId) -> Balance {
	<Runtime as Trait>::Currency::free_balance(who)
}

fn reserved_balance(who: &AccountId) -> Balance {
	<Runtime as Trait>::Currency::reserved_balance(who)
}

fn class_id_account() -> AccountId {
	<Runtime as Trait>::ModuleId::get().into_sub_account(CLASS_ID)
}

#[test]
fn create_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Default::default()
		));
		let event = TestEvent::nft(RawEvent::CreatedClass(ALICE, CLASS_ID));
		assert_eq!(last_event(), event);

		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Trait>::CreateClassDeposit::get()
		);
	});
}

#[test]
fn create_class_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			NFTModule::create_class(
				Origin::signed(BOB),
				vec![1],
				Properties(ClassProperty::Transferable | ClassProperty::Burnable)
			),
			pallet_balances::Error::<Runtime, _>::InsufficientBalance
		);
	});
}

#[test]
fn mint_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		let event = TestEvent::nft(RawEvent::CreatedClass(ALICE, CLASS_ID));
		assert_eq!(last_event(), event);

		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 2));
		let event = TestEvent::nft(RawEvent::MintedToken(ALICE, BOB, CLASS_ID, 2));
		assert_eq!(last_event(), event);

		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Trait>::CreateClassDeposit::get() + 2 * <Runtime as Trait>::CreateTokenDeposit::get()
		);
	});
}

#[test]
fn mint_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_noop!(
			NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID_NOT_EXIST, vec![1], 2),
			Error::<Runtime>::ClassIdNotFound
		);

		assert_noop!(
			NFTModule::mint(Origin::signed(BOB), BOB, CLASS_ID, vec![1], 0),
			Error::<Runtime>::InvalidQuantity
		);

		assert_noop!(
			NFTModule::mint(Origin::signed(BOB), BOB, CLASS_ID, vec![1], 2),
			Error::<Runtime>::NoPermission
		);

		orml_nft::NextTokenId::<Runtime>::mutate(CLASS_ID, |id| {
			*id = <Runtime as orml_nft::Trait>::TokenId::max_value()
		});
		assert_noop!(
			NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 2),
			orml_nft::Error::<Runtime>::NoAvailableTokenId
		);
	});
}

#[test]
fn transfer_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 2));

		assert_ok!(NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)));
		let event = TestEvent::nft(RawEvent::TransferedToken(BOB, ALICE, CLASS_ID, TOKEN_ID));
		assert_eq!(last_event(), event);

		assert_ok!(NFTModule::transfer(Origin::signed(ALICE), BOB, (CLASS_ID, TOKEN_ID)));
		let event = TestEvent::nft(RawEvent::TransferedToken(ALICE, BOB, CLASS_ID, TOKEN_ID));
		assert_eq!(last_event(), event);
	});
}

#[test]
fn transfer_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
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
			Error::<Runtime>::NoPermission
		);
	});

	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Default::default()
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
		assert_noop!(
			NFTModule::transfer(Origin::signed(BOB), ALICE, (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NonTransferable
		);
	});
}

#[test]
fn burn_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		let event = TestEvent::nft(RawEvent::BurnedToken(BOB, CLASS_ID, TOKEN_ID));
		assert_eq!(last_event(), event);

		assert_eq!(
			reserved_balance(&class_id_account()),
			<Runtime as Trait>::CreateClassDeposit::get()
		);
	});
}

#[test]
fn burn_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
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
			orml_nft::Error::<Runtime>::NumOverflow
		);
	});

	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Default::default()
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
		assert_noop!(
			NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)),
			Error::<Runtime>::NonBurnable
		);
	});
}

#[test]
fn destroy_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		assert_ok!(NFTModule::destroy_class(Origin::signed(ALICE), CLASS_ID, BOB));
		let event = TestEvent::nft(RawEvent::DestroyedClass(ALICE, CLASS_ID, BOB));
		assert_eq!(last_event(), event);

		assert_eq!(reserved_balance(&class_id_account()), 0);
		assert_eq!(free_balance(&ALICE), 99700);
		assert_eq!(free_balance(&BOB), 300);
	});
}

#[test]
fn destroy_class_should_fail() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(
			Origin::signed(ALICE),
			vec![1],
			Properties(ClassProperty::Transferable | ClassProperty::Burnable)
		));
		assert_ok!(NFTModule::mint(Origin::signed(ALICE), BOB, CLASS_ID, vec![1], 1));
		assert_noop!(
			NFTModule::destroy_class(Origin::signed(ALICE), CLASS_ID_NOT_EXIST, BOB),
			Error::<Runtime>::ClassIdNotFound
		);

		assert_noop!(
			NFTModule::destroy_class(Origin::signed(BOB), CLASS_ID, BOB),
			Error::<Runtime>::NoPermission
		);

		assert_noop!(
			NFTModule::destroy_class(Origin::signed(ALICE), CLASS_ID, BOB),
			Error::<Runtime>::CannotDestroyClass
		);

		assert_ok!(NFTModule::burn(Origin::signed(BOB), (CLASS_ID, TOKEN_ID)));
		assert_ok!(NFTModule::destroy_class(Origin::signed(ALICE), CLASS_ID, BOB));
	});
}
