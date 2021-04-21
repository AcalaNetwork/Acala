#![cfg_attr(not(feature = "std"), no_std)]
#![allow(dead_code)]

#[cfg(feature = "std")]
pub use crate::mock::{for_bench::Block, wasm_binary_unwrap};

use crate::mock::for_bench::{alice, EvmAccountsModule, Runtime, ALICE};

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

// fn get_or_create_evm_address(b: &mut Bencher) {
//     b.bench("get_or_create_evm_address", || {
//         EvmAddressMapping::<Runtime>::get_or_create_evm_address(&ALICE);
//     });
// }

orml_bencher::bench!(eth_address, eth_sign, ethereum_signable_message, eth_public);
