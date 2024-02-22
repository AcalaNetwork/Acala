// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

#![cfg(feature = "wasm-bench")]
#![allow(dead_code)]

pub use crate::{precompile::mock::*, DEXPrecompile, EVMPrecompile, OraclePrecompile};
use frame_support::assert_ok;
use hex_literal::hex;
use module_evm::{
	precompiles::{tests::MockPrecompileHandle, Precompile},
	Context,
};
use module_support::AddressMapping;
use orml_traits::DataFeeder;
use primitives::currency::{AssetMetadata, TokenInfo};
use sp_core::{H160, H256};
use wasm_bencher::{benches, Bencher};

fn whitelist_keys(b: &mut Bencher, caller: Option<H160>) {
	if let Some(caller) = caller {
		b.whitelist(module_evm::Accounts::<Test>::hashed_key_for(&caller), true, true);
		let caller_account = <Test as module_evm::Config>::AddressMapping::get_account_id(&caller);
		b.whitelist(
			pallet_balances::Reserves::<Test>::hashed_key_for(&caller_account),
			true,
			true,
		);
		b.whitelist(
			frame_system::Account::<Test>::hashed_key_for(&caller_account),
			true,
			true,
		);
	}

	// unknown key
	b.whitelist(
		hex_literal::hex!("3a7472616e73616374696f6e5f6c6576656c3a").to_vec(),
		true,
		true,
	);

	// System::Number
	b.whitelist(
		hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec(),
		true,
		true,
	);
	b.whitelist(pallet_timestamp::Now::<Test>::hashed_key().to_vec(), true, false);
}

fn setup_liquidity() {
	// faucet alice
	assert_ok!(Currencies::update_balance(RuntimeOrigin::root(), ALICE, DOT, 1_000_000));
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		ALICE,
		AUSD,
		1_000_000_000
	));

	// enable DOT/AUSD
	assert_ok!(DexModule::enable_trading_pair(RuntimeOrigin::signed(ALICE), DOT, AUSD,));

	assert_ok!(DexModule::add_liquidity(
		RuntimeOrigin::signed(ALICE),
		DOT,
		AUSD,
		1_000,
		1_000_000,
		0,
		true
	));
}

fn oracle_get_price(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, Some(caller));

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	let price = Price::from(30_000);
	assert_ok!(Oracle::feed_value(Some(ALICE), DOT, price));

	assert_ok!(AssetRegistry::register_native_asset(
		RuntimeOrigin::signed(CouncilAccount::get()),
		DOT,
		sp_std::boxed::Box::new(AssetMetadata {
			name: DOT.name().unwrap().into(),
			symbol: DOT.symbol().unwrap().into(),
			decimals: DOT.decimals().unwrap(),
			minimal_balance: 0
		})
	));

	// getPrice(address) -> 0x41976e09
	// DOT
	let input = hex! {"
		41976e09
		000000000000000000000000 0000000000000000000100000000000000000002
	"};
	// returned price
	let expected_output = hex! {"
		00000000000000000000000000000000 000000000000065a4da25d3016c00000
	"};

	let resp = b
		.bench(|| OraclePrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();

	assert_eq!(resp.output, expected_output);
}

fn evm_query_new_contract_extra_bytes(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	// newContractExtraBytes() -> 0xa23e8b82
	let input = hex! {"
		a23e8b82
	"};

	// 100
	let expected_output = hex! {"
		00000000000000000000000000000000 00000000000000000000000000000064
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();

	assert_eq!(resp.output, expected_output);
}

fn evm_query_storage_deposit_per_byte(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	// storageDepositPerByte() -> 0x6e043998
	let input = hex! {"
		6e043998
	"};

	// 10_000_000
	let expected_output = hex! {"
		00000000000000000000000000000000 00000000000000000000000000989680
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();

	assert_eq!(resp.output, expected_output);
}

fn evm_query_maintainer(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	let contract_address = H160::from(hex!("2000000000000000000000000000000000000001"));
	module_evm::Accounts::<Test>::insert(
		contract_address,
		module_evm::AccountInfo {
			nonce: 1,
			contract_info: Some(module_evm::ContractInfo {
				code_hash: H256::default(),
				maintainer: H160::from(hex!("1000000000000000000000000000000000000002")),
				published: true,
			}),
		},
	);

	// maintainerOf(address) -> 0x06ad1355
	// contract_address
	let input = hex! {"
		06ad1355
		000000000000000000000000 2000000000000000000000000000000000000001
	"};

	let expected_output = hex! {"
		000000000000000000000000 1000000000000000000000000000000000000002
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();

	assert_eq!(resp.output, expected_output);
}

fn evm_query_developer_deposit(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	// developerDeposit() -> 0x68a18855
	let input = hex! {"
		68a18855
	"};

	// 1_000_000_000
	let expected_output = hex! {"
		00000000000000000000000000000000 0000000000000000000000003b9aca00
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();
	assert_eq!(resp.output, expected_output);
}

fn evm_query_publication_fee(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	// publicationFee() -> 0x6e0e540c
	let input = hex! {"
		6e0e540c
	"};

	// 200_000_000
	let expected_output = hex! {"
		00000000000000000000000000000000 0000000000000000000000000bebc200
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();
	assert_eq!(resp.output, expected_output);
}

fn evm_query_developer_status(b: &mut Bencher) {
	let caller = alice_evm_addr();
	whitelist_keys(b, None);

	let context = Context {
		address: Default::default(),
		caller,
		apparent_value: Default::default(),
	};

	// developerStatus(address) -> 0x710f50ff
	// who
	let input = hex! {"
		710f50ff
		000000000000000000000000 1000000000000000000000000000000000000001
	"};

	// expect output is false as alice has not put a deposit down
	let expected_output = hex! {"
		00000000000000000000000000000000 00000000000000000000000000000000
	"};

	let resp = b
		.bench(|| EVMPrecompile::<Test>::execute(&mut MockPrecompileHandle::new(&input, None, &context, false)))
		.unwrap();
	assert_eq!(resp.output, expected_output);
}

benches!(
	oracle_get_price,
	evm_query_new_contract_extra_bytes,
	evm_query_storage_deposit_per_byte,
	evm_query_maintainer,
	evm_query_developer_deposit,
	evm_query_publication_fee,
	evm_query_developer_status
);
