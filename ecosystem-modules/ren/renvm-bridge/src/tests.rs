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

//! Unit tests for the renvm bridge module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, unsigned::ValidateUnsigned};
use hex_literal::hex;
use mock::{AccountId, Balances, ExtBuilder, Origin, RenVmBridge, Runtime, System};
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
		&Call::mint {
			who,
			p_hash,
			amount,
			n_hash,
			sig: sig.clone(),
		},
	)?;

	Ok(RenVmBridge::mint(Origin::none(), who, p_hash, amount, n_hash, sig))
}

fn rotate_key(new_key: PublicKey, sig: EcdsaSignature) -> Result<DispatchResult, TransactionValidityError> {
	<RenVmBridge as ValidateUnsigned>::validate_unsigned(
		TransactionSource::External,
		&Call::rotate_key {
			new_key,
			sig: sig.clone(),
		},
	)?;

	Ok(RenVmBridge::rotate_key(Origin::none(), new_key, sig))
}

#[test]
fn burn_works() {
	ExtBuilder::default().build().execute_with(|| {
		let issuer: H256 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
		assert_ok!(
			mint_ren_btc(
				issuer,
				hex!["4fe557069c2424260b9d0cca31049e70ede95c49964578044d80c74f3a118505"],
				93802,
				hex!["64c1212efd301721c9343fdf299f022778ea336608c1ae089136045b8d6f3e5c"],
				EcdsaSignature::from_slice(&hex!["5566d8eb9fec05a6636381302ad7dd6b28a0ec62e6e45038fbb095c6503ee08a69a450c566ce60ccca1233d32c24a366176d189bbe5613ae633ce3ae4b6b9a7e1b"]).unwrap(),
			)
		);
		assert_eq!(Balances::free_balance(issuer), 93802);

		let to: Vec<u8> = vec![2, 3, 4];
		assert_eq!(RenVmBridge::burn_events(0), None);
		assert_ok!(RenVmBridge::burn(Origin::signed(issuer), to.clone(), 1000));
		assert_eq!(Balances::free_balance(&issuer), 92802);
		assert_eq!(RenVmBridge::burn_events(0), Some((0, to.clone(), 1000)));
		assert_eq!(RenVmBridge::next_burn_event_id(), 1);

		System::set_block_number(15);

		assert_ok!(RenVmBridge::burn(Origin::signed(issuer), to.clone(), 2000));
		assert_eq!(Balances::free_balance(&issuer), 90802);
		assert_eq!(RenVmBridge::burn_events(1), Some((15, to, 2000)));
		assert_eq!(RenVmBridge::next_burn_event_id(), 2);
	});
}

#[test]
fn verify_mint_signature_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(
			RenVmBridge::verify_mint_signature(
				&hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
				93963,
				&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
				&hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
				&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"],
			)
		);

		assert_ok!(
			RenVmBridge::verify_mint_signature(
				&hex!["ad8fae51f70e3a013962934614201466076fec72eb60f74183f3059d6ad2c4c1"],
				86129,
				&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
				&hex!["1cdb2d4388e10ce8f89613f06a0d03a2d3fbcfd334d81d4564f7e1bfc5ebc9bb"],
				&hex!["87f068a20cfaf7752151320dcfde3994f2861cb4dd36aa73a947f23f92f135507607c997b450053914f2e9313ea2d1abf3326a7984341fdf47e4e21f33b54cda1b"],
			)
		);

		assert_ok!(
			RenVmBridge::verify_mint_signature(
				&hex!["1a98ccc4004f71c29c3ae3ee3a8fe51ece4a0eda383443aa8aaafeec4fd55247"],
				80411,
				&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
				&hex!["d45761c6d5123a10c5f707472613451de1e738b544acfbdd4b2680754ed2008a"],
				&hex!["1281893709fd7f4e1d65147a948d9884adf65bb9bcb587ea32e2f3b633fa1e1f2d82488ae89105004a301eda66ef8e5f036b705716f1df42d357647e09dd3e581c"],
			)
		);

		assert_ok!(
			RenVmBridge::verify_mint_signature(
				&hex!["425673f98610064b76dbd334783f45ea192f0e954db75ba2ae6b6058a8143d67"],
				87266,
				&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
				&hex!["fe125f912d2de05e3e34b96a0ce8a8e35d9ed883e830b978871f3e1f5d393726"],
				&hex!["acd463fa396c54995e444234e96d793d3977e75f445da219c10bc4947c22622f325f24dfc31e8e56ec21f04fc7669e91db861778a8367444bde6dfb5f95e15ed1b"],
			)
		);

		assert_ok!(
			RenVmBridge::verify_mint_signature(
				&hex!["046076abc0c7e2bd8cc15b9e22ed97deff2d8e2acfe3bec1ffbbd0255b2a094c"],
				87403,
				&hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"],
				&hex!["64962866cd5245005a06b8a10ac57626f176bc1c8e340a008c4a765a56aa4a6f"],
				&hex!["63f68adcda25db1de27b0edeb0439f7d971a22afeebb5ddb07ed05d4b07ac4fd1f78e5ecd4f2d6a21aabcc73027e8b977f9a688ae16db5aaf6c0d0021e85e3f41b"],
			)
		);
	});
}

#[test]
fn mint_works() {
	ExtBuilder::default().build().execute_with(|| {
		let to: H256 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();

		assert_ok!(
			mint_ren_btc(
				to,
				hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
				93963,
				hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
				EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]).unwrap(),
			)
		);

		assert_eq!(Balances::free_balance(to), 93963);

		assert_ok!(
			mint_ren_btc(
				to,
				hex!["425673f98610064b76dbd334783f45ea192f0e954db75ba2ae6b6058a8143d67"],
				87266,
				hex!["fe125f912d2de05e3e34b96a0ce8a8e35d9ed883e830b978871f3e1f5d393726"],
				EcdsaSignature::from_slice(&hex!["acd463fa396c54995e444234e96d793d3977e75f445da219c10bc4947c22622f325f24dfc31e8e56ec21f04fc7669e91db861778a8367444bde6dfb5f95e15ed1b"]).unwrap(),
			)
		);

		assert_eq!(Balances::free_balance(to), 93963 + 87266);

		assert_noop!(
			mint_ren_btc(
				to,
				hex!["425673f98610064b76dbd334783f45ea192f0e954db75ba2ae6b6058a8143d67"],
				87266,
				hex!["fe125f912d2de05e3e34b96a0ce8a8e35d9ed883e830b978871f3e1f5d393726"],
				EcdsaSignature::from_slice(&hex!["acd463fa396c54995e444234e96d793d3977e75f445da219c10bc4947c22622f325f24dfc31e8e56ec21f04fc7669e91db861778a8367444bde6dfb5f95e15ed1b"]).unwrap(),
			),
			TransactionValidityError::Invalid(InvalidTransaction::Stale)
		);

		assert_noop!(
			mint_ren_btc(
				to,
				hex!["425673f98610064b76dbd334783f45ea192f0e954db75ba2ae6b6058a8143d67"],
				87266,
				hex!["fe125f912d2de05e3e34b96a0ce8a8e35d9ed883e830b978871f3e1f5d393726"],
				EcdsaSignature::from_slice(&hex!["000463fa396c54995e444234e96d793d3977e75f445da219c10bc4947c22622f325f24dfc31e8e56ec21f04fc7669e91db861778a8367444bde6dfb5f95e15ed1b"]).unwrap(),
			),
			TransactionValidityError::Invalid(InvalidTransaction::BadProof)
		);
	});
}

#[test]
fn rotate_key_works() {
	ExtBuilder::default().build().execute_with(|| {
		let new_key: PublicKey = hex!["4b939fc8ade87cb50b78987b1dda927460dc456a"];

		assert_noop!(
			rotate_key(
				new_key,
				EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]).unwrap(),
			),
			TransactionValidityError::Invalid(InvalidTransaction::BadProof)
		);
	});
}

#[test]
fn transaction_length_of_mint() {
	ExtBuilder::default().build().execute_with(|| {
		let to: H256 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
		let call = Call::<Runtime>::mint{
			who: to,
			p_hash: hex!["425673f98610064b76dbd334783f45ea192f0e954db75ba2ae6b6058a8143d67"],
			amount: 10000 * 10u128.saturating_pow(8),  // 10000 BTC
			n_hash: hex!["fe125f912d2de05e3e34b96a0ce8a8e35d9ed883e830b978871f3e1f5d393726"],
			sig: EcdsaSignature::from_slice(&hex!["000463fa396c54995e444234e96d793d3977e75f445da219c10bc4947c22622f325f24dfc31e8e56ec21f04fc7669e91db861778a8367444bde6dfb5f95e15ed1b"]).unwrap()
		};

		let call_len = call.using_encoded(|c| c.len());
		assert_eq!(call_len as u32, MINT_TX_LENGTH);
	});
}
