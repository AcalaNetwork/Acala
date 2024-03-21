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

use crate::{AccountId, EvmAccounts, Runtime, RuntimeEvent, RuntimeOrigin, System, EVM};

use super::utils::{dollar, set_balance, NATIVE};
use frame_system::RawOrigin;
use module_evm::MaxCodeSize;
use module_support::AddressMapping;
use orml_benchmarking::{runtime_benchmarks, whitelist_account};
use sp_core::{H160, H256};
use sp_io::hashing::keccak_256;
use sp_runtime::DispatchError;
use sp_std::{str::FromStr, vec};

fn contract_addr() -> H160 {
	H160::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

fn alice() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

fn deploy_contract(caller: AccountId) -> Result<H160, DispatchError> {
	System::set_block_number(1);
	EVM::create(
		RuntimeOrigin::signed(caller.clone()),
		FACTORY_CONTRACT.to_vec(),
		0,
		1000000000,
		1000000000,
		vec![],
	)
	.map_or_else(|e| Err(e.error), |_| Ok(()))?;

	System::assert_last_event(RuntimeEvent::EVM(module_evm::Event::Created {
		from: module_evm_accounts::EvmAddressMapping::<Runtime>::get_evm_address(&caller).unwrap(),
		contract: contract_addr(),
		logs: vec![],
		used_gas: 132225,
		used_storage: 10367,
	}));
	Ok(contract_addr())
}

pub fn alice_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&alice());
	evm_to_account_id(address)
}

pub fn bob_account_id() -> AccountId {
	let address = EvmAccounts::eth_address(&bob());
	evm_to_account_id(address)
}

fn evm_to_account_id(address: H160) -> AccountId {
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	AccountId::from(Into::<[u8; 32]>::into(data))
}

// pragma solidity 0.8.2;
//
// contract Empty { }
const EMPTY_CONTRACT: [u8; 92] = hex_literal::hex!("6080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220e2641e5566296523edeafd776846b0e535aac278dfcf496804a865948b29646064736f6c63430008020033");

// pragma solidity 0.8.2;
//
// contract Storage {
//     uint256 public number;
//
//     function store(uint256 num) public {
//         number = num;
//     }
// }
const STORAGE_CONTRACT: [u8; 332] = hex_literal::hex!("608060405234801561001057600080fd5b5061012c806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d600480360381019060499190608c565b6069565b005b60556073565b6040516060919060bf565b60405180910390f35b8060008190555050565b60005481565b60008135905060868160e2565b92915050565b600060208284031215609d57600080fd5b600060a9848285016079565b91505092915050565b60b98160d8565b82525050565b600060208201905060d2600083018460b2565b92915050565b6000819050919050565b60e98160d8565b811460f357600080fd5b5056fea2646970667358221220b161a9e6cc3d4aac8bc0fd65e420da7555db59fefe6a1d4e8e7eea98e99b293b64736f6c63430008020033");

// pragma solidity ^0.5.0;
//
// contract Factory {
//     Contract[] newContracts;
//
//     function createContract () public payable {
//         Contract newContract = new Contract();
//         newContracts.push(newContract);
//     }
// }
//
// contract Contract {}
const FACTORY_CONTRACT: [u8; 399] = hex_literal::hex!("608060405234801561001057600080fd5b5061016f806100206000396000f3fe608060405260043610610041576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063412a5a6d14610046575b600080fd5b61004e610050565b005b600061005a6100e2565b604051809103906000f080158015610076573d6000803e3d6000fd5b50905060008190806001815401808255809150509060018203906000526020600020016000909192909190916101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055505050565b6040516052806100f28339019056fe6080604052348015600f57600080fd5b50603580601d6000396000f3fe6080604052600080fdfea165627a7a7230582092dc1966a8880ddf11e067f9dd56a632c11a78a4afd4a9f05924d427367958cc0029a165627a7a723058202b2cc7384e11c452cdbf39b68dada2d5e10a632cc0174a354b8b8c83237e28a40029");

runtime_benchmarks! {
	{ Runtime, module_evm }

	create {
		let alice_account = alice_account_id();
		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
	}: _(RawOrigin::Signed(alice_account), EMPTY_CONTRACT.to_vec(), 0, 21_000_000, 100_000, vec![])
	verify {
		// contract address when it gets deployed
		let contract_address = H160::from(hex_literal::hex!("5e0b4bfa0b55932a3587e648c3552a6515ba56b1"));
		let code_hash = EVM::code_hash_at_address(&contract_address);
		assert!(module_evm::Codes::<Runtime>::contains_key(code_hash));
	}

	create2 {
		let salt = H256::repeat_byte(1);
		let alice_account = alice_account_id();
		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
	}: _(RawOrigin::Signed(alice_account), EMPTY_CONTRACT.to_vec(), salt, 0, 21_000_000, 100_000, vec![])
	verify {
		// contract address when it gets deployed
		let contract_address = H160::from(hex_literal::hex!("f6930000a8679e0c96af73e73c02f163e34b9d70"));
		let code_hash = EVM::code_hash_at_address(&contract_address);
		assert!(module_evm::Codes::<Runtime>::contains_key(code_hash));
	}

	create_nft_contract {
		let account_id = <Runtime as module_evm::Config>::TreasuryAccount::get();
		set_balance(NATIVE, &account_id, 1_000_000 * dollar(NATIVE));
		let address = primitives::evm::MIRRORED_TOKENS_ADDRESS_START | H160::from_low_u64_be(EVM::network_contract_index());
	}: _(RawOrigin::Root, EMPTY_CONTRACT.to_vec(), 0, 2_100_000, 15_000, vec![])
	verify {
		let code_hash = EVM::code_hash_at_address(&address);
		assert!(module_evm::Codes::<Runtime>::contains_key(code_hash));
	}

	create_predeploy_contract {
		let account_id = <Runtime as module_evm::Config>::TreasuryAccount::get();
		set_balance(NATIVE, &account_id, 1_000_000 * dollar(NATIVE));
		let address = H160::from_low_u64_be(1);
	}: _(RawOrigin::Root, address, EMPTY_CONTRACT.to_vec(), 0, 2_100_000, 15_000, vec![])
	verify {
		let code_hash = EVM::code_hash_at_address(&address);
		assert!(module_evm::Codes::<Runtime>::contains_key(code_hash));
	}

	call {
		// Storage.store(1)
		let input = hex_literal::hex!("6057361d0000000000000000000000000000000000000000000000000000000000000001").to_vec();
		let alice_account = alice_account_id();
		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));

		// contract address when it gets deployed
		let contract_address = H160::from(hex_literal::hex!("5e0b4bfa0b55932a3587e648c3552a6515ba56b1"));

		frame_support::assert_ok!(EVM::create(RuntimeOrigin::signed(alice_account.clone()), STORAGE_CONTRACT.to_vec(), 0, 21_000_000, 100_000, vec![]));

		let code_hash = EVM::code_hash_at_address(&contract_address);
		assert!(module_evm::Codes::<Runtime>::contains_key(code_hash));

		// Storage::number
		let hashed_key = module_evm::AccountStorages::<Runtime>::hashed_key_for(&contract_address, H256::zero());
		frame_benchmarking::benchmarking::add_to_whitelist(hashed_key.into());

	}: _(RawOrigin::Signed(alice_account), contract_address, input, 0, 21_000_000, 100_000, vec![])
	verify {
		assert_eq!(module_evm::AccountStorages::<Runtime>::get(&contract_address, H256::zero()), H256::from_low_u64_be(1));
	}

	transfer_maintainer {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
		set_balance(NATIVE, &bob_account_id(), 1_000 * dollar(NATIVE));
		let contract = deploy_contract(alice_account_id())?;
		let bob_address = EvmAccounts::eth_address(&bob());

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract, bob_address)

	publish_contract {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000_000_000 * dollar(NATIVE));
		set_balance(NATIVE, &bob_account_id(), 1_000 * dollar(NATIVE));

		EVM::enable_contract_development(RuntimeOrigin::signed(alice_account_id()))?;

		let contract = deploy_contract(alice_account_id())?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract)

	publish_free {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
		set_balance(NATIVE, &bob_account_id(), 1_000 * dollar(NATIVE));

		EVM::enable_contract_development(RuntimeOrigin::signed(alice_account_id()))?;
		let contract = deploy_contract(alice_account_id())?;
	}: _(RawOrigin::Root, contract)

	enable_contract_development {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000 * dollar(NATIVE));

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()))

	disable_contract_development {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000 * dollar(NATIVE));
		EVM::enable_contract_development(RuntimeOrigin::signed(alice_account_id()))?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()))

	set_code {
		let c in 0..MaxCodeSize::get();
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));

		EVM::enable_contract_development(RuntimeOrigin::signed(alice_account_id()))?;
		let contract = deploy_contract(alice_account_id())?;

		let new_contract = vec![0; c as usize];

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract, new_contract)

	selfdestruct {
		let alice_account = alice_account_id();

		set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));

		EVM::enable_contract_development(RuntimeOrigin::signed(alice_account_id()))?;
		let contract = deploy_contract(alice_account_id())?;

		whitelist_account!(alice_account);
	}: _(RawOrigin::Signed(alice_account_id()), contract)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::benchmarking::utils::tests::new_test_ext;
	use module_evm::Runner;
	use orml_benchmarking::impl_benchmark_test_suite;

	impl_benchmark_test_suite!(new_test_ext(),);

	#[test]
	fn create_gas_usage() {
		new_test_ext().execute_with(|| {
			let alice_account = alice_account_id();
			set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
			let caller = module_evm_accounts::EvmAddressMapping::<Runtime>::get_or_create_evm_address(&alice_account);
			let config = <Runtime as module_evm::Config>::config();
			let result = <Runtime as module_evm::Config>::Runner::create(
				caller,
				EMPTY_CONTRACT.to_vec(),
				0,
				1_000_000,
				100_000,
				vec![],
				config,
			)
			.unwrap();
			assert!(result.exit_reason.is_succeed());
			assert_eq!(
				result.value,
				H160::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
			);
			assert_eq!(result.used_gas.as_u64(), module_evm::BASE_CREATE_GAS);
		});
	}

	#[test]
	fn call_gas_usage() {
		new_test_ext().execute_with(|| {
			let alice_account = alice_account_id();
			set_balance(NATIVE, &alice_account, 1_000_000 * dollar(NATIVE));
			let caller = module_evm_accounts::EvmAddressMapping::<Runtime>::get_or_create_evm_address(&alice_account);
			let config = <Runtime as module_evm::Config>::config();
			let result = <Runtime as module_evm::Config>::Runner::create(
				caller,
				STORAGE_CONTRACT.to_vec(),
				0,
				1_000_000,
				100_000,
				vec![],
				config,
			)
			.unwrap();
			let address = H160::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap();
			assert!(result.exit_reason.is_succeed());
			assert_eq!(result.value, address);

			let input =
				hex_literal::hex!("6057361d0000000000000000000000000000000000000000000000000000000000000001").to_vec();
			let result = <Runtime as module_evm::Config>::Runner::call(
				caller,
				caller,
				address,
				input,
				0,
				1_000_000,
				100_000,
				vec![],
				config,
			)
			.unwrap();
			assert!(result.exit_reason.is_succeed());
			assert_eq!(result.used_gas.as_u64(), module_evm::BASE_CALL_GAS);
		});
	}
}
