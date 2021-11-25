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

#![cfg(feature = "bench")]
#![allow(dead_code)]

use crate::{bench_mock::*, module::*, runner::Runner};
use frame_support::assert_ok;
use hex::FromHex;
use module_support::mocks::MockAddressMapping;
use module_support::AddressMapping;
use orml_bencher::{benches, Bencher};
use serde_json::Value;
use sp_core::H160;
use sp_std::{prelude::*, str::FromStr};

fn get_bench_desc(name: &str) -> (Vec<u8>, H160, Vec<u8>, u64, Vec<u8>) {
	let benches_str = include_str!("../../../resources/evm-benches.json");
	let evm_benches: Value = serde_json::from_str(benches_str).unwrap();
	let desc = evm_benches["benches"][name].clone();

	let code_str = desc["code"].as_str().unwrap();
	let input_str = desc["input"].as_str().unwrap_or_default();
	let output_str = desc["output"].as_str().unwrap_or_default();

	let code = Vec::from_hex(code_str).unwrap();
	let input = Vec::from_hex(input_str).unwrap();
	let output = Vec::from_hex(output_str).unwrap();

	let from = H160::from_str(desc["from"].as_str().unwrap()).unwrap();
	let used_gas = desc["used_gas"].as_u64().unwrap();

	(code, from, input, used_gas, output)
}

fn whitelist_keys(b: &mut Bencher, contract: &H160) {
	b.whitelist(
		Codes::<Runtime>::hashed_key_for(EVM::code_hash_at_address(&contract)),
		true,
		true,
	);
	b.whitelist(ContractStorageSizes::<Runtime>::hashed_key_for(&contract), true, true);
	// System::Number
	b.whitelist(
		hex_literal::hex!("26aa394eea5630e07c48ae0c9558cef702a5c1b19ab7a04f536c519aca4983ac").to_vec(),
		true,
		true,
	);
}

macro_rules! evm_create {
	($name: ident) => {
		fn $name(b: &mut Bencher) {
			let (code, from, _, used_gas, _) = get_bench_desc(stringify!($name));

			let config = <Runtime as Config>::config();

			let acc = MockAddressMapping::get_account_id(&from);
			assert_ok!(Balances::set_balance(
				Origin::root(),
				acc,
				1_000_000_000_000_000,
				0
			));

			let result = b
				.bench(|| {
					// create contract
					<Runtime as Config>::Runner::create(from, code.clone(), 0, 21_000_000, 1_000_000, config)
				})
				.unwrap();

			assert_eq!(result.used_gas, used_gas.into());
		}
	};
}

macro_rules! evm_call {
	($name: ident) => {
		fn $name(b: &mut Bencher) {
			let (code, from, input, used_gas, output) = get_bench_desc(stringify!($name));

			let acc = MockAddressMapping::get_account_id(&from);
			assert_ok!(Balances::set_balance(
				Origin::root(),
				acc,
				1_000_000_000_000_000,
				0
			));

			let config = <Runtime as Config>::config();

			// create contract
			let contract_address =
				<Runtime as Config>::Runner::create(from, code.clone(), 0, 21_000_000, 1_000_000, config)
					.unwrap()
					.value;

			assert_ok!(EVM::deploy_free(
				Origin::signed(CouncilAccount::get()),
				contract_address
			));
			whitelist_keys(b, &contract_address);

			let result = b
				.bench(|| {
					<Runtime as Config>::Runner::call(
						from,
						from,
						contract_address,
						input.clone(),
						0,
						1000000,
						1000000,
						config,
					)
				})
				.unwrap();

			assert_eq!(result.value, output);
			assert_eq!(result.used_gas, used_gas.into());
		}
	};
}

evm_create!(erc20_deploy);
evm_call!(erc20_approve);
evm_call!(erc20_transfer);

evm_create!(storage_deploy);
evm_call!(storage_store);

evm_create!(ballot_deploy);
evm_call!(ballot_delegate);

benches!(
	erc20_deploy,
	erc20_approve,
	erc20_transfer,
	storage_deploy,
	storage_store,
	ballot_deploy,
	ballot_delegate
);
