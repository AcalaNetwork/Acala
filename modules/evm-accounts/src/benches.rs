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

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

#[cfg(feature = "std")]
pub use crate::mock::{for_bench::Block, wasm_binary_unwrap};

use crate::mock::{
	alice,
	for_bench::{EvmAccountsModule, Origin, Runtime},
	ALICE,
};
use module_support::AddressMapping;

fn ethereum_signable_message(b: &mut Bencher) {
	b.bench("ethereum_signable_message", || {
		EvmAccountsModule::ethereum_signable_message(&[0u8; 32], &[][..]);
	});
}

fn eth_recover(b: &mut Bencher) {
	let signature = EvmAccountsModule::eth_sign(&alice(), &[0u8; 32], &[][..]);
	b.bench("eth_recover", || {
		EvmAccountsModule::eth_recover(&signature, &[0u8; 32], &[][..]);
	});
}

fn eth_public(b: &mut Bencher) {
	let alice_secret = alice();
	b.bench("eth_public", || {
		EvmAccountsModule::eth_public(&alice_secret);
	});
}

fn eth_address(b: &mut Bencher) {
	let alice_secret = alice();
	b.bench("eth_address", || {
		EvmAccountsModule::eth_address(&alice_secret);
	});
}

fn eth_sign(b: &mut Bencher) {
	let alice_secret = alice();
	b.bench("eth_sign", || {
		EvmAccountsModule::eth_sign(&alice_secret, &[0u8; 32], &[][..]);
	});
}

fn get_account_id(b: &mut Bencher) {
	EvmAccountsModule::claim_default_account(Origin::signed(ALICE)).unwrap();
	let alice_evm_addr = crate::EvmAddressMapping::<Runtime>::get_evm_address(&ALICE).unwrap();
	b.bench("get_account_id", || {
		crate::EvmAddressMapping::<Runtime>::get_account_id(&alice_evm_addr);
	});
}

fn get_evm_address(b: &mut Bencher) {
	EvmAccountsModule::claim_default_account(Origin::signed(ALICE)).unwrap();
	b.bench("get_evm_address", || {
		crate::EvmAddressMapping::<Runtime>::get_evm_address(&ALICE);
	});
}

fn get_or_create_evm_address(b: &mut Bencher) {
	b.bench("get_or_create_evm_address", || {
		crate::EvmAddressMapping::<Runtime>::get_or_create_evm_address(&ALICE);
	});
}

fn get_default_evm_address(b: &mut Bencher) {
	b.bench("get_default_evm_address", || {
		crate::EvmAddressMapping::<Runtime>::get_default_evm_address(&ALICE);
	});
}

fn is_linked(b: &mut Bencher) {
	EvmAccountsModule::claim_default_account(Origin::signed(ALICE)).unwrap();
	let alice_evm_addr = crate::EvmAddressMapping::<Runtime>::get_evm_address(&ALICE).unwrap();

	b.bench("is_linked", || {
		crate::EvmAddressMapping::<Runtime>::is_linked(&ALICE, &alice_evm_addr);
	});
}

orml_bencher::bench!(
	ethereum_signable_message,
	eth_recover,
	eth_public,
	eth_address,
	eth_sign,
	get_account_id,
	get_evm_address,
	get_or_create_evm_address,
	get_default_evm_address,
	is_linked
);
