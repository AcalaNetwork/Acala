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

pub mod mock;

use crate::{
	code_hash, evm::Runtime as EVMRuntime, module::*, runner::Runner, Context, StackExecutor, StackSubstateMetadata,
	SubstrateStackState,
};
use frame_support::{assert_ok, BoundedVec};
use hex::FromHex;
use mock::*;
use module_support::mocks::MockAddressMapping;
use module_support::AddressMapping;
use primitives::evm::Vicinity;
use serde_json::Value;
use sp_core::{H160, H256, U256};
use sp_std::{convert::TryInto, prelude::*, rc::Rc, str::FromStr};
use wasm_bencher::{benches, Bencher};

fn get_bench_info(name: &str) -> (Vec<u8>, H160, Vec<u8>, u64, Vec<u8>) {
	let benches_str = include_str!("../../../../evm-bench/build/benches.json");
	let evm_benches: Value = serde_json::from_str(benches_str).unwrap();
	let info = evm_benches[name].clone();

	let code_str = info["code"].as_str().unwrap();
	let input_str = info["input"].as_str().unwrap_or_default();
	let output_str = info["output"].as_str().unwrap_or_default();

	let code = Vec::from_hex(code_str).unwrap();
	let input = Vec::from_hex(input_str).unwrap();
	let output = Vec::from_hex(output_str).unwrap();

	let from = H160::from_str(info["from"].as_str().unwrap()).unwrap();
	let used_gas = info["used_gas"].as_u64().unwrap();

	(code, from, input, used_gas, output)
}

fn faucet(address: &H160) {
	let account_id = MockAddressMapping::get_account_id(&address);
	assert_ok!(Balances::force_set_balance(
		RuntimeOrigin::root(),
		account_id,
		1_000_000_000_000_000,
	));
}

fn whitelist_keys(b: &mut Bencher, from: H160, code: Vec<u8>) -> H160 {
	let address = H160::from_str("2000000000000000000000000000000000000001").unwrap();
	let vicinity = Vicinity {
		gas_price: U256::one(),
		..Default::default()
	};
	let context = Context {
		caller: from,
		address: address.clone(),
		apparent_value: Default::default(),
	};
	let config = <Runtime as Config>::config();
	let metadata = StackSubstateMetadata::new(21_000_000, 1_000_000, config);
	let state = SubstrateStackState::<Runtime>::new(&vicinity, metadata);
	let mut executor = StackExecutor::new_with_precompiles(state, config, &());

	let mut runtime = EVMRuntime::new(
		Rc::new(code.clone()),
		Rc::new(Vec::new()),
		context,
		config.stack_limit,
		config.memory_limit,
	);
	let reason = executor.execute(&mut runtime);

	assert!(reason.is_succeed(), "{:?}", reason);

	let out = runtime.machine().return_value();
	let bounded_code: BoundedVec<u8, MaxCodeSize> = out.try_into().unwrap();
	let code_hash = code_hash(bounded_code.as_slice());

	// unknown key
	b.whitelist(
		hex_literal::hex!("3a7472616e73616374696f6e5f6c6576656c3a").to_vec(),
		true,
		true,
	);

	// non-existent contract will end up reading this key
	b.whitelist(
		Codes::<Runtime>::hashed_key_for(&H256::from_slice(&hex_literal::hex!(
			"c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
		))),
		true,
		true,
	);
	b.whitelist(Codes::<Runtime>::hashed_key_for(&code_hash), true, true);
	b.whitelist(CodeInfos::<Runtime>::hashed_key_for(&code_hash), true, true);
	b.whitelist(Accounts::<Runtime>::hashed_key_for(&from), true, true);
	b.whitelist(Accounts::<Runtime>::hashed_key_for(&address), true, true);
	b.whitelist(ContractStorageSizes::<Runtime>::hashed_key_for(&address), true, true);
	let from_account = <Runtime as Config>::AddressMapping::get_account_id(&from);
	let address_account = <Runtime as Config>::AddressMapping::get_account_id(&address);
	b.whitelist(
		pallet_balances::Reserves::<Runtime>::hashed_key_for(&from_account),
		true,
		true,
	);
	b.whitelist(
		pallet_balances::Reserves::<Runtime>::hashed_key_for(&address_account),
		true,
		true,
	);
	b.whitelist(
		frame_system::Account::<Runtime>::hashed_key_for(&from_account),
		true,
		true,
	);
	b.whitelist(
		frame_system::Account::<Runtime>::hashed_key_for(&address_account),
		true,
		true,
	);

	// System::Number
	b.whitelist(
		hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec(),
		true,
		true,
	);

	address
}

macro_rules! evm_create {
	($name: ident) => {
		fn $name(b: &mut Bencher) {
			let (code, from, _, used_gas, _) = get_bench_info(stringify!($name));
			faucet(&from);
			let contract_address = whitelist_keys(b, from, code.clone());

			let result = b
				.bench(|| {
					// create contract
					<Runtime as Config>::Runner::create_at_address(
						from,
						contract_address,
						code.clone(),
						0,
						21_000_000,
						1_000_000,
						vec![],
						<Runtime as Config>::config(),
					)
				})
				.unwrap();
			assert!(
				result.exit_reason.is_succeed(),
				"CREATE: Deploy contract failed with: {:?}",
				result.exit_reason
			);
			assert_eq!(result.used_gas, used_gas.into());
		}
	};
}

macro_rules! evm_call {
	($name: ident) => {
		fn $name(b: &mut Bencher) {
			let (code, from, input, used_gas, output) = get_bench_info(stringify!($name));
			faucet(&from);
			let contract_address = whitelist_keys(b, from, code.clone());

			// create contract
			let result = <Runtime as Config>::Runner::create_at_address(
				from,
				contract_address,
				code.clone(),
				0,
				21_000_000,
				1_000_000,
				vec![],
				<Runtime as Config>::config(),
			)
			.unwrap();

			assert!(
				result.exit_reason.is_succeed(),
				"CALL: Deploy contract failed with: {:?}",
				result.exit_reason
			);
			assert_eq!(contract_address, result.value);

			let result = b
				.bench(|| {
					<Runtime as Config>::Runner::call(
						from,
						from,
						contract_address,
						input.clone(),
						0,
						21_000_000,
						1_000_000,
						vec![],
						<Runtime as Config>::config(),
					)
				})
				.unwrap();

			assert!(
				result.exit_reason.is_succeed(),
				"Call failed {:?}",
				result.exit_reason
			);
			assert_eq!(result.value, output);
			assert_eq!(result.used_gas, used_gas.into());
		}
	};
}

evm_create!(empty_deploy);
evm_call!(empty_noop);

evm_create!(erc20_deploy);
evm_call!(erc20_approve);
evm_call!(erc20_approve_many);
evm_call!(erc20_transfer);
evm_call!(erc20_transfer_many);

evm_create!(storage_deploy);
evm_call!(storage_store);
evm_call!(storage_store_many);

evm_create!(ballot_deploy);
evm_call!(ballot_delegate);
evm_call!(ballot_vote);

benches!(
	empty_deploy,
	empty_noop,
	erc20_deploy,
	erc20_approve,
	erc20_approve_many,
	erc20_transfer,
	erc20_transfer_many,
	storage_deploy,
	storage_store,
	storage_store_many,
	ballot_deploy,
	ballot_delegate,
	ballot_vote
);
