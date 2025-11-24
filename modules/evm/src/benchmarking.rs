// This file is part of Acala.

// Copyright (C) 2020-2025 Acala Foundation.
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

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use module_support::EVMAccountsManager;
use sp_io::hashing::keccak_256;
use std::str::FromStr;

fn contract_addr() -> H160 {
	H160::from_str("0x5e0b4bfa0b55932a3587e648c3552a6515ba56b1").unwrap()
}

fn alice() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Alice")).unwrap()
}

fn bob() -> libsecp256k1::SecretKey {
	libsecp256k1::SecretKey::parse(&keccak_256(b"Bob")).unwrap()
}

fn deploy_contract<T>(caller: T::AccountId) -> H160
where
	T: Config + frame_system::Config + module_evm_accounts::Config,
	T::AccountId: From<[u8; 32]>,
{
	frame_system::Pallet::<T>::set_block_number(1u32.into());

	let _ = <T as Config>::Currency::deposit_creating(&caller, 1_000_000_000_000_000);

	assert_ok!(Pallet::<T>::create(
		RawOrigin::Signed(caller.clone()).into(),
		FACTORY_CONTRACT.to_vec(),
		0,
		1000000000,
		1000000000,
		vec![],
	));

	frame_system::Pallet::<T>::assert_last_event(
		Event::Created {
			from: module_evm_accounts::Pallet::<T>::get_evm_address(&caller).unwrap(),
			contract: contract_addr(),
			logs: vec![],
			used_gas: 132225,
			used_storage: 467,
		}
		.into(),
	);
	contract_addr()
}

pub fn alice_account_id<T>() -> T::AccountId
where
	T: Config + module_evm_accounts::Config,
	T::AccountId: From<[u8; 32]>,
{
	let address = module_evm_accounts::Pallet::<T>::eth_address(&alice());
	evm_to_account_id::<T>(address)
}

#[allow(dead_code)]
pub fn bob_account_id<T>() -> T::AccountId
where
	T: Config + module_evm_accounts::Config,
	T::AccountId: From<[u8; 32]>,
{
	let address = module_evm_accounts::Pallet::<T>::eth_address(&bob());
	evm_to_account_id::<T>(address)
}

fn evm_to_account_id<T>(address: H160) -> T::AccountId
where
	T: Config,
	T::AccountId: From<[u8; 32]>,
{
	let mut data = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);
	T::AccountId::from(Into::<[u8; 32]>::into(data))
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

#[benchmarks(
	where T: Config + module_evm_accounts::Config,
	T::AccountId: From<[u8; 32]>
)]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn create() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(alice_account),
			EMPTY_CONTRACT.to_vec(),
			0,
			21_000_000,
			100_000,
			vec![],
		);

		// contract address when it gets deployed
		let code_hash = Pallet::<T>::code_hash_at_address(&contract_addr());
		assert!(Codes::<T>::contains_key(code_hash));
	}

	#[benchmark]
	fn create2() {
		let salt = H256::repeat_byte(1);
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		#[extrinsic_call]
		_(
			RawOrigin::Signed(alice_account),
			EMPTY_CONTRACT.to_vec(),
			salt,
			0,
			21_000_000,
			100_000,
			vec![],
		);

		// contract address when it gets deployed
		let contract_address = H160::from(hex_literal::hex!("f6930000a8679e0c96af73e73c02f163e34b9d70"));
		let code_hash = Pallet::<T>::code_hash_at_address(&contract_address);
		assert!(Codes::<T>::contains_key(code_hash));
	}

	#[benchmark]
	fn create_nft_contract() {
		let account_id = <T as Config>::TreasuryAccount::get();
		let _ = <T as Config>::Currency::deposit_creating(&account_id, 1_000_000_000_000_000);
		let address = primitives::evm::MIRRORED_TOKENS_ADDRESS_START
			| H160::from_low_u64_be(Pallet::<T>::network_contract_index());

		#[extrinsic_call]
		_(RawOrigin::Root, EMPTY_CONTRACT.to_vec(), 0, 2_100_000, 15_000, vec![]);

		let code_hash = Pallet::<T>::code_hash_at_address(&address);
		assert!(Codes::<T>::contains_key(code_hash));
	}

	#[benchmark]
	fn create_predeploy_contract() {
		let account_id = <T as Config>::TreasuryAccount::get();
		let _ = <T as Config>::Currency::deposit_creating(&account_id, 1_000_000_000_000_000);
		let address = H160::from_low_u64_be(1);

		#[extrinsic_call]
		_(
			RawOrigin::Root,
			address,
			EMPTY_CONTRACT.to_vec(),
			0,
			2_100_000,
			15_000,
			vec![],
		);

		let code_hash = Pallet::<T>::code_hash_at_address(&address);
		assert!(Codes::<T>::contains_key(code_hash));
	}

	#[benchmark]
	fn call() {
		// Storage.store(1)
		let input =
			hex_literal::hex!("6057361d0000000000000000000000000000000000000000000000000000000000000001").to_vec();

		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		// contract address when it gets deployed
		let contract_address = H160::from(hex_literal::hex!("5e0b4bfa0b55932a3587e648c3552a6515ba56b1"));

		assert_ok!(Pallet::<T>::create(
			RawOrigin::Signed(alice_account.clone()).into(),
			STORAGE_CONTRACT.to_vec(),
			0,
			21_000_000,
			100_000,
			vec![]
		));

		let code_hash = Pallet::<T>::code_hash_at_address(&contract_address);
		assert!(Codes::<T>::contains_key(code_hash));

		// Storage::number
		let hashed_key = AccountStorages::<T>::hashed_key_for(&contract_address, H256::zero());
		frame_benchmarking::benchmarking::add_to_whitelist(hashed_key.into());

		#[extrinsic_call]
		_(
			RawOrigin::Signed(alice_account),
			contract_address,
			input,
			0,
			21_000_000,
			100_000,
			vec![],
		);

		assert_eq!(
			AccountStorages::<T>::get(&contract_address, H256::zero()),
			H256::from_low_u64_be(1)
		);
	}

	#[benchmark]
	fn transfer_maintainer() {
		let alice_account = alice_account_id::<T>();

		let contract = deploy_contract::<T>(alice_account.clone());

		let bob_address = module_evm_accounts::Pallet::<T>::eth_address(&bob());

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account), contract, bob_address);
	}

	#[benchmark]
	fn publish_contract() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		assert_ok!(Pallet::<T>::enable_contract_development(
			RawOrigin::Signed(alice_account.clone()).into()
		));

		let contract = deploy_contract::<T>(alice_account.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account), contract);
	}

	#[benchmark]
	fn publish_free() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		assert_ok!(Pallet::<T>::enable_contract_development(
			RawOrigin::Signed(alice_account.clone()).into()
		));

		let contract = deploy_contract::<T>(alice_account.clone());

		#[extrinsic_call]
		_(RawOrigin::Root, contract);
	}

	#[benchmark]
	fn enable_contract_development() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account));
	}

	#[benchmark]
	fn disable_contract_development() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		assert_ok!(Pallet::<T>::enable_contract_development(
			RawOrigin::Signed(alice_account.clone()).into()
		));

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account));
	}

	#[benchmark]
	fn set_code(c: Liner<0, { MaxCodeSize::get() }>) {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		assert_ok!(Pallet::<T>::enable_contract_development(
			RawOrigin::Signed(alice_account.clone()).into()
		));

		let contract = deploy_contract::<T>(alice_account.clone());

		let new_contract = vec![0; c as usize];

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account), contract, new_contract);
	}

	#[benchmark]
	fn selfdestruct() {
		let alice_account = alice_account_id::<T>();
		let _ = <T as Config>::Currency::deposit_creating(&alice_account, 1_000_000_000_000_000);

		assert_ok!(Pallet::<T>::enable_contract_development(
			RawOrigin::Signed(alice_account.clone()).into()
		));

		let contract = deploy_contract::<T>(alice_account.clone());

		#[extrinsic_call]
		_(RawOrigin::Signed(alice_account), contract);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Runtime);
}
