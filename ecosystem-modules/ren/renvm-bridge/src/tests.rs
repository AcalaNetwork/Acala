//! Unit tests for the renvm bridge module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::OnFinalize, unsigned::ValidateUnsigned};
use hex_literal::hex;
use mock::{AccountId, Balances, ExtBuilder, Origin, RenVmBridge, RenvmBridgeCall};
use sp_core::H256;
use sp_runtime::transaction_validity::TransactionValidityError;

fn mint_ren_btc(
	who: AccountId,
	p_hash: [u8; 32],
	amount: Balance,
	n_hash: [u8; 32],
	sig: EcdsaSignature,
) -> Result<DispatchResult, TransactionValidityError> {
	<RenVmBridge as ValidateUnsigned>::validate_unsigned(
		TransactionSource::External,
		&RenvmBridgeCall::mint(who.clone(), p_hash, amount, n_hash, sig.clone()),
	)?;

	Ok(RenVmBridge::mint(Origin::none(), who, p_hash, amount, n_hash, sig))
}

#[test]
fn burn_works() {
	ExtBuilder::default().build().execute_with(|| {
		let issuer: H256 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
		assert_ok!(
			mint_ren_btc(
				issuer.clone(),
				hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
				5000,
				hex!["e96cc92771222bd8f674ddf4ef6a4264e38030e90380fb215cb145591ed803e9"],
				EcdsaSignature(hex!["1beaeea7cb5433659979ba0ba17bc0174c87b6208ea0fa82e1478a74b3ded5a27324239b8f0ef31f54cc56deb32bb8962803ecf399eac7ade08f291ae03f6a1f1c"]),
			)
		);
		assert_eq!(Balances::free_balance(issuer.clone()), 5000);

		let to: [u8; 20] = [0; 20];
		assert_eq!(RenVmBridge::burn_events(10), vec![]);
		assert_ok!(RenVmBridge::burn(Origin::signed(issuer.clone()), to.clone(), 1000));
		assert_eq!(Balances::free_balance(&issuer), 4000);
		assert_eq!(RenVmBridge::burn_events(10), vec![(to.clone(), 1000)]);

		assert_ok!(RenVmBridge::burn(Origin::signed(issuer.clone()), to.clone(), 2000));
		assert_eq!(Balances::free_balance(&issuer), 2000);
		assert_eq!(RenVmBridge::burn_events(10), vec![(to.clone(), 1000), (to.clone(), 2000)]);

		RenVmBridge::on_finalize(10);
		assert_eq!(RenVmBridge::burn_events(10), vec![]);
	});
}

#[test]
fn verify_signature_works() {
	assert_ok!(
		RenVmBridge::verify_signature(
			&hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
			5000,
			&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
			&hex!["e96cc92771222bd8f674ddf4ef6a4264e38030e90380fb215cb145591ed803e9"],
			&hex!["1beaeea7cb5433659979ba0ba17bc0174c87b6208ea0fa82e1478a74b3ded5a27324239b8f0ef31f54cc56deb32bb8962803ecf399eac7ade08f291ae03f6a1f1c"],
		)
	);

	assert_ok!(
		RenVmBridge::verify_signature(
			&hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
			5000,
			&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
			&hex!["6d9b77f6070c8dd4e6e6ad2217d6aa6ef48a06e27a3c4a189e0a9f2c59db409e"],
			&hex!["130bef45db4f2b7ccf2689cfd8214e7dbdeb4263de1c26bcd1c702ce4a4093b97d49c835f8225e52103047eef3feca2e41681ea5a27dc6ab84a26efc49f05f971b"],
		)
	);

	assert_ok!(
		RenVmBridge::verify_signature(
			&hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
			6000,
			&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
			&hex!["7c5e9fad22654694c5bbbce509c2003b10cf90798cd84b1fb1851cdfba58d52e"],
			&hex!["776abdea3287da906a5c72dd08f9be1b0a160374ae7045b028a17098f98970245d173aa73d1e8ae99adf23ccf92030e6c4a390c62952f1dffb37bbcfde4bef171b"],
		)
	);

	assert_ok!(
		RenVmBridge::verify_signature(
			&hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
			95000,
			&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
			&hex!["81e25aafbe2fb3ea02de043f5e13118c087a12d6871198cc97d160180fafcca2"],
			&hex!["09f05f67a282e483d7e064ad1f2382dfedf6df11f55d42c86a47e6f54e0dd004280b395a923a8a60a93b6986217bb67adb4cc066ad4444dc28ec92d1de23b5f11b"],
		)
	);

	assert_ok!(
		RenVmBridge::verify_signature(
			&hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
			5000,
			&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
			&hex!["6bed1f11a3d904a7e5b555a2524b2ce1a8bdbfa10f68dcb93f32b25c8df74c5a"],
			&hex!["0a8167a494b8e3e0e45e50f9650537ebecefb688bc870777c6ef5f3722d932a516c3cced274b6550384eba9c59556083312dd5f1fdebcfadf0cb04a372207e271c"],
		)
	);
}

#[test]
fn mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let to: H256 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();

		assert_ok!(
			mint_ren_btc(
				to.clone(),
				hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
				5000,
				hex!["e96cc92771222bd8f674ddf4ef6a4264e38030e90380fb215cb145591ed803e9"],
				EcdsaSignature(hex!["1beaeea7cb5433659979ba0ba17bc0174c87b6208ea0fa82e1478a74b3ded5a27324239b8f0ef31f54cc56deb32bb8962803ecf399eac7ade08f291ae03f6a1f1c"]),
			)
		);

		assert_eq!(Balances::free_balance(to.clone()), 5000);

		assert_ok!(
			mint_ren_btc(
				to.clone(),
				hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
				95000,
				hex!["81e25aafbe2fb3ea02de043f5e13118c087a12d6871198cc97d160180fafcca2"],
				EcdsaSignature(hex!["09f05f67a282e483d7e064ad1f2382dfedf6df11f55d42c86a47e6f54e0dd004280b395a923a8a60a93b6986217bb67adb4cc066ad4444dc28ec92d1de23b5f11b"]),
			)
		);

		assert_eq!(Balances::free_balance(to.clone()), 5000 + 95000);

		assert_noop!(
			mint_ren_btc(
				to.clone(),
				hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
				95000,
				hex!["81e25aafbe2fb3ea02de043f5e13118c087a12d6871198cc97d160180fafcca2"],
				EcdsaSignature(hex!["09f05f67a282e483d7e064ad1f2382dfedf6df11f55d42c86a47e6f54e0dd004280b395a923a8a60a93b6986217bb67adb4cc066ad4444dc28ec92d1de23b5f11b"]),
			),
			TransactionValidityError::Invalid(InvalidTransaction::Stale)
		);

		assert_noop!(
			mint_ren_btc(
				to.clone(),
				hex!["c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"],
				95000,
				hex!["81e25aafbe2fb3ea02de043f5e13118c087a12d6871198cc97d160180fafcca2"],
				EcdsaSignature(hex!["000000000000000000e064ad1f2382dfedf6df11f55d42c86a47e6f54e0dd004280b395a923a8a60a93b6986217bb67adb4cc066ad4444dc28ec92d1de23b5f11b"]),
			),
			TransactionValidityError::Invalid(InvalidTransaction::BadProof)
		);
	});
}
