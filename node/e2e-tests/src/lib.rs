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

//! End to end runtime tests.

#![allow(clippy::type_complexity)]

use node_runtime::Runtime;
use sc_executor::NativeElseWasmExecutor;
use sc_service::{TFullBackend, TFullClient};
use sp_runtime::generic::Era;
use std::sync::Arc;
use test_runner::{ChainInfo, SignatureVerificationOverride};

/// A unit struct which implements `NativeExecutionDispatch` feeding in the
/// hard-coded runtime.
pub struct ExecutorDispatch;

impl sc_executor::NativeExecutionDispatch for ExecutorDispatch {
	type ExtendHostFunctions = (
		frame_benchmarking::benchmarking::HostFunctions,
		SignatureVerificationOverride,
	);

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		node_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		node_runtime::native_version()
	}
}

/// ChainInfo implementation.
struct NodeTemplateChainInfo;

impl ChainInfo for NodeTemplateChainInfo {
	type Block = node_primitives::Block;
	type ExecutorDispatch = ExecutorDispatch;
	type Runtime = Runtime;
	type RuntimeApi = node_runtime::RuntimeApi;
	type SelectChain = sc_consensus::LongestChain<TFullBackend<Self::Block>, Self::Block>;
	type BlockImport = Arc<TFullClient<Self::Block, Self::RuntimeApi, NativeElseWasmExecutor<Self::ExecutorDispatch>>>;
	type SignedExtras = node_runtime::SignedExtra;
	type InherentDataProviders = (
		sp_timestamp::InherentDataProvider,
		cumulus_primitives_parachain_inherent::MockValidationDataInherentDataProvider,
	);

	fn signed_extras(from: <Self::Runtime as frame_system::Config>::AccountId) -> Self::SignedExtras {
		(
			frame_system::CheckSpecVersion::<Self::Runtime>::new(),
			frame_system::CheckTxVersion::<Self::Runtime>::new(),
			frame_system::CheckGenesis::<Self::Runtime>::new(),
			frame_system::CheckMortality::<Self::Runtime>::from(Era::Immortal),
			runtime_common::CheckNonce::<Self::Runtime>::from(frame_system::Pallet::<Self::Runtime>::account_nonce(
				from,
			)),
			frame_system::CheckWeight::<Self::Runtime>::new(),
			module_transaction_payment::ChargeTransactionPayment::<Self::Runtime>::from(0),
			module_evm::SetEvmOrigin::<Self::Runtime>::new(),
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ecosystem_renvm_bridge::EcdsaSignature;
	use hex_literal::hex;
	use node_service::chain_spec::mandala::dev_testnet_config;
	use sp_keyring::sr25519::Keyring::{Alice, Bob};
	use sp_runtime::{traits::IdentifyAccount, AccountId32, MultiAddress, MultiSigner};
	use test_runner::*;

	#[test]
	#[ignore] // TODO: fix this after https://github.com/paritytech/substrate/issues/10039
	fn test_runner() {
		let tokio_runtime = build_runtime().unwrap();
		let (rpc, task_manager, client, pool, command_sink, backend) = client_parts::<NodeTemplateChainInfo>(
			ConfigOrChainSpec::ChainSpec(Box::new(dev_testnet_config().unwrap()), tokio_runtime.handle().clone()),
		)
		.unwrap();
		let node = Node::<NodeTemplateChainInfo>::new(rpc, task_manager, client, pool, command_sink, backend);

		tokio_runtime.block_on(async {
			// seals blocks
			node.seal_blocks(1).await;
			// submit extrinsics
			let alice = MultiSigner::from(Alice.public()).into_account();
			let _hash = node
				.submit_extrinsic(
					frame_system::Call::remark {
						remark: (b"hello world").to_vec(),
					},
					Some(alice),
				)
				.await
				.unwrap();

			// look ma, I can read state.
			let _events = node.with_state(|| frame_system::Pallet::<node_runtime::Runtime>::events());
			// get access to the underlying client.
			let _client = node.client();
		})
	}

	#[test]
	#[ignore] // TODO: fix this after https://github.com/paritytech/substrate/issues/10039
	fn simple_balances_test() {
		let tokio_runtime = build_runtime().unwrap();
		let (rpc, task_manager, client, pool, command_sink, backend) = client_parts::<NodeTemplateChainInfo>(
			ConfigOrChainSpec::ChainSpec(Box::new(dev_testnet_config().unwrap()), tokio_runtime.handle().clone()),
		)
		.unwrap();
		let node = Node::<NodeTemplateChainInfo>::new(rpc, task_manager, client, pool, command_sink, backend);

		tokio_runtime.block_on(async {
			// submit extrinsics
			let alice = MultiSigner::from(Alice.public()).into_account();
			let _hash = node
				.submit_extrinsic(
					frame_system::Call::remark {
						remark: (b"hello world").to_vec(),
					},
					Some(alice),
				)
				.await
				.unwrap();

			type Balances = pallet_balances::Pallet<Runtime>;

			let (alice, bob) = (MultiSigner::from(Alice.public()), MultiSigner::from(Bob.public()));
			let (alice_account_id, bob_account_id) = (alice.into_account(), bob.into_account());

			// the function with_state allows us to read state, pretty cool right? :D
			let old_balance = node.with_state(|| Balances::free_balance(bob_account_id.clone()));

			let amount = 70_000_000_000_000;

			// Send extrinsic in action.
			let tx = pallet_balances::Call::transfer {
				dest: MultiAddress::from(bob_account_id.clone()),
				value: amount,
			};
			node.submit_extrinsic(tx, Some(alice_account_id)).await.unwrap();

			// Produce blocks in action, Powered by manual-seal™.
			node.seal_blocks(1).await;

			// we can check the new state :D
			let new_balance = node.with_state(|| Balances::free_balance(bob_account_id.clone()));

			// we can now make assertions on how state has changed.
			assert_eq!(old_balance + amount, new_balance);
		})
	}

	#[test]
	#[ignore] // TODO: fix this after https://github.com/paritytech/substrate/issues/10039
	fn transaction_pool_priority_order_test() {
		let tokio_runtime = build_runtime().unwrap();
		let (rpc, task_manager, client, pool, command_sink, backend) = client_parts::<NodeTemplateChainInfo>(
			ConfigOrChainSpec::ChainSpec(Box::new(dev_testnet_config().unwrap()), tokio_runtime.handle().clone()),
		)
		.unwrap();
		let node = Node::<NodeTemplateChainInfo>::new(rpc, task_manager, client, pool, command_sink, backend);

		tokio_runtime.block_on(async {
			let (alice, bob) = (MultiSigner::from(Alice.public()), MultiSigner::from(Bob.public()));
			let (alice_account_id, bob_account_id) = (alice.into_account(), bob.into_account());

			// send operational extrinsic
			let operational_tx_hash = node.submit_extrinsic(
				pallet_sudo::Call::sudo { call: Box::new(module_emergency_shutdown::Call::emergency_shutdown { }.into()) },
				Some(alice_account_id),
			).await.unwrap();

			// send normal extrinsic
			let normal_tx_hash = node.submit_extrinsic(
				pallet_balances::Call::transfer { dest: MultiAddress::from(bob_account_id.clone()), value: 80_000 },
				Some(bob_account_id),
			).await.unwrap();

			// send unsigned extrinsic
			let to: AccountId32 = hex!["d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"].into();
			let unsigned_tx_hash = node.submit_extrinsic(
				ecosystem_renvm_bridge::Call::mint {
					who: to,
					p_hash: hex!["67028f26328144de6ef80b8cd3b05e0cefb488762c340d1574c0542f752996cb"],
					amount: 93963,
					n_hash: hex!["f6a75cc370a2dda6dfc8d016529766bb6099d7fa0d787d9fe5d3a7e60c9ac2a0"],
					sig: EcdsaSignature::from_slice(&hex!["defda6eef01da2e2a90ce30ba73e90d32204ae84cae782b485f01d16b69061e0381a69cafed3deb6112af044c42ed0f7c73ee0eec7b533334d31a06db50fc40e1b"]),
				},
				None,
			).await.unwrap();

			assert_eq!(node.pool().ready().count(), 3);

			// Ensure tx priority order:
			// Inherent -> Operational tx -> Unsigned tx -> Signed normal tx
			let mut txs = node.pool().ready();
			let tx1 = txs.next().unwrap();
			let tx2 = txs.next().unwrap();
			let tx3 = txs.next().unwrap();

			assert_eq!(tx1.hash, operational_tx_hash);
			assert_eq!(tx1.priority, 13835064928601523711);

			assert_eq!(tx2.hash, unsigned_tx_hash);
			assert_eq!(tx2.priority, 1844674407370965161);

			assert_eq!(tx3.hash, normal_tx_hash);
			assert_eq!(tx3.priority, 42785501349000);
		})
	}
}
