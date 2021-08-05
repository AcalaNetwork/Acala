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

use node_service::chain_spec::mandala::dev_testnet_config;
use node_service::default_mock_parachain_inherent_data_provider;
use pallet_balances::Call as BalancesCall;
use sc_consensus_manual_seal::ConsensusDataProvider;
use sc_service::{new_full_parts, Configuration, TFullBackend, TFullClient, TaskExecutor, TaskManager};
use sp_consensus::BlockImport;
use sp_consensus::SlotData;
use sp_inherents::CreateInherentDataProviders;
use sp_keyring::sr25519::Keyring::{Alice, Bob};
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::traits::NumberFor;
use sp_runtime::MultiAddress;
use sp_runtime::{generic::Era, traits::IdentifyAccount, MultiSigner};
use std::sync::Arc;
use test_runner::{default_config, ChainInfo, Node, SignatureVerificationOverride};

sc_executor::native_executor_instance!(
	pub Executor,
	node_runtime::api::dispatch,
	node_runtime::native_version,
	(
		frame_benchmarking::benchmarking::HostFunctions,
		SignatureVerificationOverride,
	)
);

/// ChainInfo implementation.
struct NodeTemplateChainInfo;

impl ChainInfo for NodeTemplateChainInfo {
	type Block = node_primitives::Block;
	type Executor = Executor;
	type Runtime = node_runtime::Runtime;
	type RuntimeApi = node_runtime::RuntimeApi;
	type SelectChain = sc_consensus::LongestChain<TFullBackend<Self::Block>, Self::Block>;
	type BlockImport = Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>;
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
			frame_system::CheckNonce::<Self::Runtime>::from(frame_system::Pallet::<Self::Runtime>::account_nonce(from)),
			frame_system::CheckWeight::<Self::Runtime>::new(),
			module_transaction_payment::ChargeTransactionPayment::<Self::Runtime>::from(0),
			module_evm::SetEvmOrigin::<Self::Runtime>::new(),
		)
	}

	fn config(task_executor: TaskExecutor) -> Configuration {
		default_config(task_executor, Box::new(dev_testnet_config().unwrap()))
	}

	fn create_client_parts(
		config: &Configuration,
	) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			SyncCryptoStorePtr,
			TaskManager,
			Box<dyn CreateInherentDataProviders<Self::Block, (), InherentDataProviders = Self::InherentDataProviders>>,
			Option<
				Box<
					dyn ConsensusDataProvider<
						Self::Block,
						Transaction = sp_api::TransactionFor<
							TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
							Self::Block,
						>,
					>,
				>,
			>,
			Self::SelectChain,
			Self::BlockImport,
		),
		sc_service::Error,
	> {
		let (client, backend, keystore, task_manager) =
			new_full_parts::<Self::Block, Self::RuntimeApi, Self::Executor>(config, None)?;
		let client = Arc::new(client);

		let select_chain = sc_consensus::LongestChain::new(backend.clone());

		Ok((
			client.clone(),
			backend,
			keystore.sync_keystore(),
			task_manager,
			Box::new(move |_, _| async move {
				Ok((
					sp_timestamp::InherentDataProvider::from_system_time(),
					default_mock_parachain_inherent_data_provider(),
				))
			}),
			None,
			select_chain,
			client.clone(),
		))
	}

	fn dispatch_with_root(call: <Self::Runtime as frame_system::Config>::Call, node: &mut Node<Self>) {
		let alice = MultiSigner::from(Alice.public()).into_account();
		let call = pallet_sudo::Call::sudo(Box::new(call));
		node.submit_extrinsic(call, alice);
		node.seal_blocks(1);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use log::LevelFilter;
	use test_runner::NodeConfig;

	#[test]
	fn test_runner() {
		let config = NodeConfig {
			log_targets: vec![
				("yamux", LevelFilter::Off),
				("multistream_select", LevelFilter::Off),
				("libp2p", LevelFilter::Off),
				("jsonrpc_client_transports", LevelFilter::Off),
				("sc_network", LevelFilter::Off),
				("tokio_reactor", LevelFilter::Off),
				("parity-db", LevelFilter::Off),
				("sub-libp2p", LevelFilter::Off),
				("sync", LevelFilter::Off),
				("peerset", LevelFilter::Off),
				("ws", LevelFilter::Off),
				("sc_network", LevelFilter::Off),
				("sc_service", LevelFilter::Off),
				("sc_basic_authorship", LevelFilter::Off),
				("telemetry-logger", LevelFilter::Off),
				("sc_peerset", LevelFilter::Off),
				("rpc", LevelFilter::Off),
				("runtime", LevelFilter::Trace),
				("aura", LevelFilter::Debug),
			],
		};
		let mut node = Node::<NodeTemplateChainInfo>::new(config).unwrap();
		// seals blocks
		node.seal_blocks(1);
		// submit extrinsics
		let alice = MultiSigner::from(Alice.public()).into_account();
		node.submit_extrinsic(frame_system::Call::remark((b"hello world").to_vec()), alice);

		// look ma, I can read state.
		let _events = node.with_state(|| frame_system::Pallet::<node_runtime::Runtime>::events());
		// get access to the underlying client.
		let _client = node.client();
	}

	#[test]
	fn simple_balances_test() {
		// given
		let config = NodeConfig { log_targets: vec![] };
		let mut node = Node::<NodeTemplateChainInfo>::new(config).unwrap();

		type Balances = pallet_balances::Pallet<node_runtime::Runtime>;

		let (alice, bob) = (MultiSigner::from(Alice.public()), MultiSigner::from(Bob.public()));
		let (alice_account_id, bob_acount_id) = (alice.into_account(), bob.into_account());

		// the function with_state allows us to read state, pretty cool right? :D
		let old_balance = node.with_state(|| Balances::free_balance(alice_account_id.clone()));

		// 70 dots
		let amount = 70_000_000_000_000;

		// Send extrinsic in action.
		node.submit_extrinsic(
			pallet_balances::Call::transfer(MultiAddress::from(bob_acount_id.clone()), amount),
			alice_account_id.clone(),
		);

		// Produce blocks in action, Powered by manual-seal™.
		node.seal_blocks(1);

		// we can check the new state :D
		let new_balance = node.with_state(|| Balances::free_balance(alice_account_id));
		let events = node.with_state(|| frame_system::Pallet::<node_runtime::Runtime>::events());
		println!("events1 = {:?}", events);

		// we can now make assertions on how state has changed.
		assert_eq!(old_balance - amount, new_balance);
	}
}
