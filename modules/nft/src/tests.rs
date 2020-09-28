//! Unit tests for the non-fungible-token module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok};
use mock::{
	ExtBuilder, NFTModule, Origin, Runtime, ALICE, BOB, CLASS_ID, CLASS_ID_NOT_EXIST, TOKEN_ID, TOKEN_ID_NOT_EXIST,
};

#[test]
fn create_class_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(NFTModule::create_class(Origin::signed(ALICE), vec![1], vec![]));
	});
}

//#[test]
//fn create_class_should_fail() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_noop!(
//			NFTModule::create_class(&ALICE, vec![1], ()),
//			Error::<Runtime>::NoAvailableClassId
//		);
//	});
//}

////#[test]
//fn mint_should_work() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//	});
//}
//
////#[test]
//fn mint_should_fail() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_noop!(
//			NFTModule::mint(&BOB, CLASS_ID, vec![1], ()),
//			Error::<Runtime>::NumOverflow
//		);
//
//		//NextTokenId::<Runtime>::mutate(|id| *id = <Runtime as
// Trait>::TokenId::max_value()); 		assert_noop!(
//			NFTModule::mint(&BOB, CLASS_ID, vec![1], ()),
//			Error::<Runtime>::NoAvailableTokenId
//		);
//	});
//}
//
////#[test]
//fn transfer_should_work() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//		assert_ok!(NFTModule::transfer(&BOB, &BOB, (CLASS_ID, TOKEN_ID)));
//		assert_ok!(NFTModule::transfer(&BOB, &ALICE, (CLASS_ID, TOKEN_ID)));
//		assert_ok!(NFTModule::transfer(&ALICE, &BOB, (CLASS_ID, TOKEN_ID)));
//	});
//}
//
////#[test]
//fn transfer_should_fail() {
//	ExtBuilder::default().build().execute_with(|| {
//		//NextClassId::<Runtime>::mutate(|id| *id = <Runtime as
// Trait>::ClassId::max_value()); 		assert_ok!(NFTModule::mint(&BOB, CLASS_ID,
// vec![1], ())); 		assert_noop!(
//			NFTModule::transfer(&BOB, &ALICE, (CLASS_ID, TOKEN_ID_NOT_EXIST)),
//			Error::<Runtime>::TokenNotFound
//		);
//		assert_noop!(
//			NFTModule::transfer(&ALICE, &ALICE, (CLASS_ID, TOKEN_ID)),
//			Error::<Runtime>::NoPermission
//		);
//	});
//}
//
////#[test]
//fn burn_should_work() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//		assert_ok!(NFTModule::burn(&BOB, (CLASS_ID, TOKEN_ID)));
//	});
//}
//
////#[test]
//fn burn_should_fail() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//		assert_noop!(
//			NFTModule::burn(&BOB, (CLASS_ID, TOKEN_ID_NOT_EXIST)),
//			Error::<Runtime>::TokenNotFound
//		);
//
//		assert_noop!(
//			NFTModule::burn(&ALICE, (CLASS_ID, TOKEN_ID)),
//			Error::<Runtime>::NoPermission
//		);
//
//		assert_noop!(
//			NFTModule::burn(&BOB, (CLASS_ID, TOKEN_ID)),
//			Error::<Runtime>::NumOverflow
//		);
//	});
//}
//
////#[test]
//fn destroy_class_should_work() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//		assert_ok!(NFTModule::burn(&BOB, (CLASS_ID, TOKEN_ID)));
//		assert_ok!(NFTModule::destroy_class(&ALICE, CLASS_ID));
//	});
//}
//
////#[test]
//fn destroy_class_should_fail() {
//	ExtBuilder::default().build().execute_with(|| {
//		assert_ok!(NFTModule::create_class(&ALICE, vec![1], ()));
//		assert_ok!(NFTModule::mint(&BOB, CLASS_ID, vec![1], ()));
//		assert_noop!(
//			NFTModule::destroy_class(&ALICE, CLASS_ID_NOT_EXIST),
//			Error::<Runtime>::ClassNotFound
//		);
//
//		assert_noop!(
//			NFTModule::destroy_class(&BOB, CLASS_ID),
//			Error::<Runtime>::NoPermission
//		);
//
//		assert_noop!(
//			NFTModule::destroy_class(&ALICE, CLASS_ID),
//			Error::<Runtime>::CannotDestroyClass
//		);
//
//		assert_ok!(NFTModule::burn(&BOB, (CLASS_ID, TOKEN_ID)));
//		assert_ok!(NFTModule::destroy_class(&ALICE, CLASS_ID));
//	});
//}
