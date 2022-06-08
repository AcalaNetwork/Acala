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

use cumulus_primitives_core::ParaId;
use ecosystem_renvm_bridge::EcdsaSignature;
use hex_literal::hex;
use module_evm::AddressMapping;
use sc_transaction_pool_api::TransactionPool;
use sha3::{Digest, Keccak256};
use sp_core::{crypto::AccountId32, H160, H256};
use sp_keyring::Sr25519Keyring::*;
use sp_runtime::{traits::IdentifyAccount, MultiAddress, MultiSigner};
use test_service::{ensure_event, SealMode};

#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore] // TODO: Wasm binary must be built for testing, polkadot/node/test/service/src/chain_spec.rs:117:40
async fn simple_balances_dev_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	let bob = MultiSigner::from(Bob.public());
	let bob_account_id = bob.into_account();
	let amount = 1_000_000_000_000;

	type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

	// the function with_state allows us to read state, pretty cool right? :D
	let old_balance = node.with_state(|| Balances::free_balance(bob_account_id.clone()));

	node.transfer(Alice, Bob, amount, 0).await.unwrap();

	node.wait_for_blocks(1).await;

	// we can check the new state :D
	let new_balance = node.with_state(|| Balances::free_balance(bob_account_id));
	assert_eq!(old_balance + amount, new_balance);
}

#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn transaction_pool_priority_order_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	let bob = MultiSigner::from(Bob.public());
	let bob_account_id = bob.into_account();

	// send operational extrinsic
	let operational_tx_hash = node
		.submit_extrinsic(
			pallet_sudo::Call::sudo {
				call: Box::new(module_emergency_shutdown::Call::emergency_shutdown {}.into()),
			},
			Some(Alice),
			0,
		)
		.await
		.unwrap();

	// send normal extrinsic
	let normal_tx_hash = node
		.submit_extrinsic(
			pallet_balances::Call::transfer {
				dest: MultiAddress::from(bob_account_id.clone()),
				value: 80_000,
			},
			Some(Bob),
			0,
		)
		.await
		.unwrap();

	// send unsigned extrinsic
	let to: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
	let unsigned_tx_hash = node.submit_extrinsic(
		ecosystem_renvm_bridge::Call::mint {
			who: to,
			p_hash: hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
			amount: 93963,
			n_hash: hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
			sig: EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]).unwrap(),
		},
		None,
		0,
	).await.unwrap();

	assert_eq!(node.transaction_pool.ready().count(), 3);

	// Ensure tx priority order:
	// Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
	let mut txs = node.transaction_pool.ready();
	let tx1 = txs.next().unwrap();
	let tx2 = txs.next().unwrap();
	let tx3 = txs.next().unwrap();

	assert_eq!(tx1.hash, operational_tx_hash);
	assert_eq!(tx2.hash, unsigned_tx_hash);
	assert_eq!(tx3.hash, normal_tx_hash);

	assert!(tx1.priority > tx2.priority);
	assert!(tx2.priority > tx3.priority);
}

/// this testcase will take too long to run, test with command:
/// cargo test --release --package test-service -- evm_fill_block_test --nocapture --include-ignored
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn evm_fill_block_test() {
	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	node.wait_for_blocks(1).await;

	type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

	let acc = node.with_state(|| {
		<node_runtime::Runtime as module_evm::Config>::AddressMapping::get_account_id(&H160::from(hex!(
			"1000000000000000000000000000000000000001"
		)))
	});

	let old_balance = node.with_state(|| Balances::free_balance(acc.clone()));

	let target = H160::from(hex!("0000000000000000000100000000000000000000")); // ACA

	// transfer
	// to
	// amount 100000000000
	let input = hex! {"
		a9059cbb
		000000000000000000000000 1000000000000000000000000000000000000001
		00000000000000000000000000000000 0000000000000000000000174876e800
	"};

	let functions = std::iter::repeat_with(|| {
		node_runtime::Call::EVM(module_evm::Call::call {
			target,
			input: input.to_vec(),
			value: 0,
			gas_limit: 100_000,
			storage_limit: 100_000,
			access_list: vec![],
		})
	})
	.take(1_000)
	.collect();

	frame_support::assert_ok!(node.submit_extrinsic_batch(functions, Some(Alice), 0).await);

	// wait for 6 blocks
	node.wait_for_blocks(6).await;

	let pending_tx = node.transaction_pool.status().ready as u128;

	let new_balance = node.with_state(|| Balances::free_balance(acc));
	assert_eq!(new_balance - old_balance, (1000 - pending_tx) * 100000000000);
}

/// this testcase will take too long to run, test with command:
/// cargo test --release --package test-service -- evm_create_fill_block_test --nocapture
/// --include-ignored
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn evm_create_fill_block_test() {
	/*
	   pragma solidity ^0.8.0;
	   contract Contract {}
	*/
	let contract = hex! {"
		6080604052348015600f57600080fd5b50603f80601d6000396000f3fe608060
		4052600080fdfea2646970667358221220b9cbc7f3d9528c236f2c6bdf64e25a
		c8ca17489f9b4e91a6d92bea793883d5d764736f6c63430008020033
	"}
	.to_vec();

	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	node.wait_for_blocks(1).await;

	let functions = std::iter::repeat_with(|| {
		node_runtime::Call::EVM(module_evm::Call::create {
			input: contract.clone(),
			value: 0,
			gas_limit: 2_000_000,
			storage_limit: 100_000,
			access_list: vec![],
		})
	})
	.take(1_000)
	.collect();

	frame_support::assert_ok!(node.submit_extrinsic_batch(functions, Some(Alice), 0).await);

	// wait for 5 blocks
	node.wait_for_blocks(5).await;
	println!(
		"{:#?}",
		ensure_event!(node, node_runtime::Event::EVM(module_evm::Event::Created { .. }))
	);
}

/// this testcase will take too long to run, test with command:
/// cargo test --release --package test-service -- evm_gas_limit_test --nocapture --include-ignored
#[substrate_test_utils::test(flavor = "multi_thread")]
#[ignore]
async fn evm_gas_limit_test() {
	/*
	   pragma solidity ^0.8.0;
	   contract Factory {
		   Contract[] newContracts;
		   uint value;
		   function createContractLoop (uint count) public {
			   for(uint i = 0; i < count; i++) {
				   Contract newContract = new Contract();
				   newContracts.push(newContract);
			   }
		   }
		   function incrementLoop (uint count) public {
			   for(uint i = 0; i < count; i++) {
				   value += 1;
			   }
		   }
	   }
	   contract Contract {}
	*/
	let contract = hex! {"
		608060405234801561001057600080fd5b50610335806100206000396000f3fe
		608060405234801561001057600080fd5b50600436106100365760003560e01c
		80633f8308e61461003b578063659aaab314610057575b600080fd5b61005560
		048036038101906100509190610182565b610073565b005b6100716004803603
		81019061006c9190610182565b6100ae565b005b60005b818110156100aa5760
		0180600082825461009091906101af565b9250508190555080806100a2906102
		0f565b915050610076565b5050565b60005b8181101561015d57600060405161
		00c790610161565b604051809103906000f0801580156100e3573d6000803e3d
		6000fd5b50905060008190806001815401808255809150506001900390600052
		60206000200160009091909190916101000a81548173ffffffffffffffffffff
		ffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffff
		ffffffff1602179055505080806101559061020f565b9150506100b1565b5050
		565b605c806102a483390190565b60008135905061017c8161028c565b929150
		50565b60006020828403121561019857610197610287565b5b60006101a68482
		850161016d565b91505092915050565b60006101ba82610205565b91506101c5
		83610205565b9250827fffffffffffffffffffffffffffffffffffffffffffff
		ffffffffffffffffffff038211156101fa576101f9610258565b5b8282019050
		92915050565b6000819050919050565b600061021a82610205565b91507fffff
		ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff8214
		1561024d5761024c610258565b5b600182019050919050565b7f4e487b710000
		0000000000000000000000000000000000000000000000000000600052601160
		045260246000fd5b600080fd5b61029581610205565b81146102a057600080fd
		5b5056fe6080604052348015600f57600080fd5b50603f80601d6000396000f3
		fe6080604052600080fdfea264697066735822122003981c658c4f81879e8a61
		dac66895b300ed8c1522a2d242522caddab6fe5b6464736f6c63430008070033
		a264697066735822122047d51951d1cde00ab7c772ef239b4d5614518dc10741
		4ff90f297239ff62848f64736f6c63430008070033
	"}
	.to_vec();

	let mut builder = sc_cli::LoggerBuilder::new("");
	builder.with_colors(true);
	let _ = builder.init();

	let para_id = ParaId::from(2000);
	let tokio_handle = tokio::runtime::Handle::current();

	let node = test_service::TestNodeBuilder::new(para_id, tokio_handle.clone(), Alice)
		.with_seal_mode(SealMode::DevAuraSeal)
		.enable_collator()
		.build()
		.await;

	type EVM = module_evm::Pallet<node_runtime::Runtime>;

	let function = node_runtime::Call::EVM(module_evm::Call::create {
		input: contract,
		value: 0,
		gas_limit: 2_000_000,
		storage_limit: 100_000,
		access_list: vec![],
	});
	frame_support::assert_ok!(node.submit_extrinsic(function, Some(Alice), 0).await);

	let alice_addr = node.with_state(|| {
		<node_runtime::Runtime as module_evm::Config>::AddressMapping::get_or_create_evm_address(&Alice.into())
	});

	let mut stream = rlp::RlpStream::new_list(2);
	stream.append(&alice_addr);
	stream.append(&0u32);
	let contract_address: H160 = H256::from_slice(Keccak256::digest(&stream.out()).as_slice()).into();

	frame_support::assert_ok!(
		node.submit_extrinsic(
			node_runtime::Call::EVM(module_evm::Call::publish_contract {
				contract: contract_address
			}),
			Some(Alice),
			1,
		)
		.await
	);

	node.wait_for_blocks(1).await;

	println!(
		"{:#?}",
		ensure_event!(node, node_runtime::Event::EVM(module_evm::Event::Created { .. }))
	);

	// make sure contract is deployed
	let contract_account = node.with_state(|| EVM::accounts(contract_address).unwrap());
	assert_eq!(contract_account.nonce, 1);
	assert_eq!(contract_account.contract_info.unwrap().published, true);

	// createContractLoop(uint256) 460 times
	let input = hex! {"
		659aaab3
		00000000000000000000000000000000 000000000000000000000000000001cc
	"}
	.to_vec();

	let function = node_runtime::Call::EVM(module_evm::Call::call {
		target: contract_address,
		input: input.clone(),
		value: 0,
		gas_limit: 33_000_000,
		storage_limit: 5_000_000,
		access_list: vec![],
	});

	println!("{:#?}", node.submit_extrinsic(function, Some(Alice), 2).await);

	node.wait_for_blocks(1).await;
	println!(
		"{:#?}",
		ensure_event!(node, node_runtime::Event::EVM(module_evm::Event::Executed { .. }))
	);

	node.wait_for_blocks(1).await;

	// incrementLoop(uint256) 9500 times
	let input = hex! {"
		3f8308e6
		00000000000000000000000000000000 0000000000000000000000000000251c
	"}
	.to_vec();

	let function = node_runtime::Call::EVM(module_evm::Call::call {
		target: contract_address,
		input: input.clone(),
		value: 0,
		gas_limit: 33_000_000,
		storage_limit: 5_000_000,
		access_list: vec![],
	});

	println!("{:#?}", node.submit_extrinsic(function, Some(Alice), 3).await);

	node.wait_for_blocks(1).await;
	println!(
		"{:#?}",
		ensure_event!(node, node_runtime::Event::EVM(module_evm::Event::Executed { .. }))
	);

	node.wait_for_blocks(1).await;
}
