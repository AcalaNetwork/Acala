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

use crate::mock::for_bench::{alice, EvmAccountsModule, Runtime, ALICE};
use module_support::AddressMapping;

fn eth_address(b: &mut Bencher) {
	b.bench("eth_address", || {
		EvmAccountsModule::eth_address(&alice());
	});
}

fn eth_sign(b: &mut Bencher) {
	b.bench("eth_sign", || {
		EvmAccountsModule::eth_sign(&alice(), &[0u8; 32], &[][..]);
	});
}

fn ethereum_signable_message(b: &mut Bencher) {
	b.bench("ethereum_signable_message", || {
		EvmAccountsModule::ethereum_signable_message(&[0u8; 32], &[][..]);
	});
}

fn eth_public(b: &mut Bencher) {
	b.bench("eth_public", || {
		EvmAccountsModule::eth_public(&alice());
	});
}

fn get_or_create_evm_address(b: &mut Bencher) {
	b.bench("get_or_create_evm_address", || {
		crate::EvmAddressMapping::<Runtime>::get_or_create_evm_address(&ALICE);
	});
}

orml_bencher::bench!(eth_address, eth_sign, ethereum_signable_message, eth_public);
