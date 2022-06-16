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

//! Crate used for testing with acala.
#[macro_use]

mod builder;
mod node;
mod rpc;
mod service;

use futures::channel::{mpsc, oneshot};
use std::{future::Future, sync::Arc, time::Duration};

use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::{ParachainCandidate, ParachainConsensus};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	genesis::generate_genesis_block, prepare_node_config, start_collator, start_full_node, StartCollatorParams,
	StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_inprocess_interface::RelayChainInProcessInterface;
use cumulus_relay_chain_interface::{RelayChainError, RelayChainInterface, RelayChainResult};
use cumulus_relay_chain_rpc_interface::RelayChainRPCInterface;

use frame_system_rpc_runtime_api::AccountNonceApi;
use futures::{channel::mpsc::Sender, SinkExt};
use jsonrpsee::RpcModule;
use parking_lot::Mutex;
use polkadot_primitives::v2::{CollatorPair, Hash as PHash, HeadData, PersistedValidationData};
use sc_client_api::{execution_extensions::ExecutionStrategies, Backend, CallExecutor, ExecutorProvider};
use sc_consensus::LongestChain;
use sc_consensus_aura::{ImportQueueParams, StartAuraParams};
use sc_consensus_manual_seal::{
	rpc::{ManualSeal, ManualSealApiServer},
	EngineCommand,
};
use sc_executor::{NativeElseWasmExecutor, WasmExecutionMethod, WasmtimeInstantiationStrategy};
use sc_network::{config::TransportConfig, multiaddr, NetworkService};
pub use sc_rpc::SubscriptionTaskExecutor;
use sc_service::{
	config::{
		DatabaseSource, KeepBlocks, KeystoreConfig, MultiaddrWithPeerId, NetworkConfiguration, OffchainWorkerConfig,
		PruningMode,
	},
	BasePath, ChainSpec, Configuration, PartialComponents, Role, RpcHandlers, SpawnTasksParams, TFullBackend,
	TFullCallExecutor, TFullClient, TaskManager,
};
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_api::{OverlayedChanges, StorageTransactionCache};
use sp_arithmetic::traits::SaturatedConversion;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_core::{ExecutionContext, Pair, H256};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{
	codec::Encode,
	generic,
	generic::Era,
	traits::{BlakeTwo256, Block as BlockT, Extrinsic, IdentifyAccount},
	transaction_validity::TransactionSource,
	MultiAddress,
};
use sp_state_machine::{BasicExternalities, Ext};
use sp_trie::PrefixedMemoryDB;
use substrate_test_client::{BlockchainEventsExt, RpcHandlersExt, RpcTransactionError, RpcTransactionOutput};
use url::Url;

use node_primitives::{signature::AcalaMultiSignature, AccountId, Address, Balance, Signature};
use node_runtime::{Block, BlockId, Hash, Header, Runtime, RuntimeApi, SignedExtra};
use node_service::chain_spec::mandala::dev_testnet_config;

pub use builder::TestNodeBuilder;
pub use node::TestNode;
pub use node_runtime as runtime;
pub use service::{new_partial, start_dev_node, start_node_impl};

/// A consensus that will never produce any block.
#[derive(Clone)]
struct NullConsensus;

#[async_trait::async_trait]
impl ParachainConsensus<Block> for NullConsensus {
	async fn produce_candidate(
		&mut self,
		_: &Header,
		_: PHash,
		_: &PersistedValidationData,
	) -> Option<ParachainCandidate<Block>> {
		None
	}
}

/// The signature of the announce block fn.
pub type AnnounceBlockFn = Arc<dyn Fn(Hash, Option<Vec<u8>>) + Send + Sync>;

/// Native executor instance.
pub struct RuntimeExecutor;

impl sc_executor::NativeExecutionDispatch for RuntimeExecutor {
	type ExtendHostFunctions = ();

	fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
		node_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		node_runtime::native_version()
	}
}

/// The client type being used by the test service.
pub type Client = TFullClient<runtime::Block, runtime::RuntimeApi, NativeElseWasmExecutor<RuntimeExecutor>>;

/// Transaction pool type used by the test service
pub type TxPool = Arc<sc_transaction_pool::FullPool<Block, Client>>;

/// Maybe Mandala Dev full select chain.
type MaybeFullSelectChain = Option<LongestChain<TFullBackend<Block>, Block>>;

pub enum Consensus {
	/// Use the relay-chain provided consensus.
	RelayChain,
	/// Use the null consensus that will never produce any block.
	Null,
	/// Use Aura consensus
	Aura,
}

#[derive(Clone, Copy)]
pub enum SealMode {
	/// Dev instant seal
	DevInstantSeal,
	/// Dev aura seal
	DevAuraSeal,
	/// Parachain aura seal
	ParaSeal,
}

/// Fetch account nonce for key pair
pub fn fetch_nonce(client: &Client, account: sp_core::sr25519::Public) -> u32 {
	let best_hash = client.chain_info().best_hash;
	client
		.runtime_api()
		.account_nonce(&generic::BlockId::Hash(best_hash), account.into())
		.expect("Fetching account nonce works; qed")
}

/// Construct an extrinsic that can be applied to the test runtime.
pub fn construct_extrinsic(
	client: &Client,
	function: impl Into<runtime::Call>,
	caller: sp_core::sr25519::Pair,
	nonce: Option<u32>,
) -> runtime::UncheckedExtrinsic {
	let function = function.into();
	let current_block = client.info().best_number.saturated_into();
	let genesis_block = client.hash(0).unwrap().unwrap();
	let current_block_hash = client.info().best_hash;
	let nonce = nonce.unwrap_or_else(|| fetch_nonce(client, caller.public()));
	let period = runtime::BlockHashCount::get()
		.checked_next_power_of_two()
		.map(|c| c / 2)
		.unwrap_or(2) as u64;
	let tip = 0;
	let extra: runtime::SignedExtra = (
		frame_system::CheckNonZeroSender::<Runtime>::new(),
		frame_system::CheckSpecVersion::<Runtime>::new(),
		frame_system::CheckTxVersion::<Runtime>::new(),
		frame_system::CheckGenesis::<Runtime>::new(),
		frame_system::CheckEra::<Runtime>::from(Era::mortal(period, current_block)),
		runtime_common::CheckNonce::<Runtime>::from(nonce),
		frame_system::CheckWeight::<Runtime>::new(),
		module_transaction_payment::ChargeTransactionPayment::<Runtime>::from(tip),
		module_evm::SetEvmOrigin::<Runtime>::new(),
	);
	let raw_payload = runtime::SignedPayload::from_raw(
		function,
		extra,
		(
			(),
			runtime::VERSION.spec_version,
			runtime::VERSION.transaction_version,
			genesis_block,
			current_block_hash,
			(),
			(),
			(),
			(),
		),
	);
	let signature = raw_payload.using_encoded(|e| caller.sign(e));
	let account: AccountId = caller.public().into();
	let address: Address = account.into();
	let (call, extra, _) = raw_payload.deconstruct();
	let signed_data: (Address, AcalaMultiSignature, SignedExtra) = (address, Signature::Sr25519(signature), extra);
	runtime::UncheckedExtrinsic::new(call, Some(signed_data)).unwrap()
}

/// Run a relay-chain validator node.
///
/// This is essentially a wrapper around
/// [`run_validator_node`](polkadot_test_service::run_validator_node).
pub fn run_relay_chain_validator_node(
	tokio_handle: tokio::runtime::Handle,
	key: Sr25519Keyring,
	storage_update_func: impl Fn(),
	boot_nodes: Vec<MultiaddrWithPeerId>,
) -> polkadot_test_service::PolkadotTestNode {
	let config = polkadot_test_service::node_config(storage_update_func, tokio_handle, key, boot_nodes, true);

	polkadot_test_service::run_validator_node(
		config,
		Some(cumulus_test_relay_validation_worker_provider::VALIDATION_WORKER.into()),
	)
}

/// Returns the initial head data for a parachain ID.
pub fn initial_head_data() -> HeadData {
	let spec = Box::new(dev_testnet_config(None).unwrap());
	let block: Block = generate_genesis_block(&(spec as Box<_>), sp_runtime::StateVersion::V1).unwrap();
	let genesis_state = block.header().encode();
	genesis_state.into()
}
