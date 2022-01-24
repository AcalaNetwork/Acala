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

//! Crate used for testing with Cumulus.

#![warn(missing_docs)]

mod chain_spec;
mod genesis;

use std::{future::Future, time::Duration};

use cumulus_client_consensus_common::{ParachainCandidate, ParachainConsensus};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_local::RelayChainLocal;
use cumulus_test_runtime::{Hash, Header, NodeBlock as Block, RuntimeApi};

use frame_system_rpc_runtime_api::AccountNonceApi;
use parking_lot::Mutex;
use polkadot_primitives::v1::{CollatorPair, Hash as PHash, PersistedValidationData};
use polkadot_service::ProvideRuntimeApi;
use sc_client_api::execution_extensions::ExecutionStrategies;
use sc_network::{config::TransportConfig, multiaddr, NetworkService};
use sc_service::{
	config::{
		DatabaseSource, KeepBlocks, KeystoreConfig, MultiaddrWithPeerId, NetworkConfiguration, OffchainWorkerConfig,
		PruningMode, TransactionStorageMode, WasmExecutionMethod,
	},
	BasePath, ChainSpec, Configuration, Error as ServiceError, PartialComponents, Role, RpcHandlers, TFullBackend,
	TFullClient, TaskManager,
};
use sp_arithmetic::traits::SaturatedConversion;
use sp_blockchain::HeaderBackend;
use sp_core::{Pair, H256};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{codec::Encode, generic, traits::BlakeTwo256};
use sp_state_machine::BasicExternalities;
use sp_trie::PrefixedMemoryDB;
use std::sync::Arc;
use substrate_test_client::{BlockchainEventsExt, RpcHandlersExt, RpcTransactionError, RpcTransactionOutput};

pub use chain_spec::*;
pub use cumulus_test_runtime as runtime;
pub use genesis::*;
pub use sp_keyring::Sr25519Keyring as Keyring;

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
		cumulus_test_runtime::api::dispatch(method, data)
	}

	fn native_version() -> sc_executor::NativeVersion {
		cumulus_test_runtime::native_version()
	}
}

/// The client type being used by the test service.
pub type Client =
	TFullClient<runtime::NodeBlock, runtime::RuntimeApi, sc_executor::NativeElseWasmExecutor<RuntimeExecutor>>;

/// Transaction pool type used by the test service
pub type TransactionPool = Arc<sc_transaction_pool::FullPool<Block, Client>>;

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
pub fn new_partial(
	config: &mut Configuration,
) -> Result<
	PartialComponents<
		Client,
		TFullBackend<Block>,
		(),
		sc_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		sc_transaction_pool::FullPool<Block, Client>,
		(),
	>,
	sc_service::Error,
> {
	let executor = sc_executor::NativeElseWasmExecutor::<RuntimeExecutor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, _>(&config, None, executor)?;
	let client = Arc::new(client);

	let registry = config.prometheus_registry();

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let import_queue = cumulus_client_consensus_relay_chain::import_queue(
		client.clone(),
		client.clone(),
		|_, _| async { Ok(sp_timestamp::InherentDataProvider::from_system_time()) },
		&task_manager.spawn_essential_handle(),
		registry.clone(),
	)?;

	let params = PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain: (),
		other: (),
	};

	Ok(params)
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[sc_tracing::logging::prefix_logs_with(parachain_config.network.node_name.as_str())]
async fn start_node_impl<RB>(
	parachain_config: Configuration,
	collator_key: Option<CollatorPair>,
	relay_chain_config: Configuration,
	para_id: ParaId,
	wrap_announce_block: Option<Box<dyn FnOnce(AnnounceBlockFn) -> AnnounceBlockFn>>,
	rpc_ext_builder: RB,
	consensus: Consensus,
) -> sc_service::error::Result<(
	TaskManager,
	Arc<Client>,
	Arc<NetworkService<Block, H256>>,
	RpcHandlers,
	TransactionPool,
)>
where
	RB: Fn(Arc<Client>) -> Result<jsonrpc_core::IoHandler<sc_rpc::Metadata>, sc_service::Error> + Send + 'static,
{
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let mut parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&mut parachain_config)?;

	let transaction_pool = params.transaction_pool.clone();
	let mut task_manager = params.task_manager;

	let relay_chain_full_node = polkadot_test_service::new_full(
		relay_chain_config,
		if let Some(ref key) = collator_key {
			polkadot_service::IsCollator::Yes(key.clone())
		} else {
			polkadot_service::IsCollator::Yes(CollatorPair::generate().0)
		},
		None,
	)
	.map_err(|e| match e {
		polkadot_service::Error::Sub(x) => x,
		s => s.to_string().into(),
	})?;

	let client = params.client.clone();
	let backend = params.backend.clone();

	let relay_chain_interface = Arc::new(RelayChainLocal::new(
		relay_chain_full_node.client.clone(),
		relay_chain_full_node.backend.clone(),
		Arc::new(Mutex::new(Box::new(relay_chain_full_node.network.clone()))),
		relay_chain_full_node.overseer_handle.clone(),
	));
	task_manager.add_child(relay_chain_full_node.task_manager);

	let block_announce_validator = BlockAnnounceValidator::new(relay_chain_interface.clone(), para_id);
	let block_announce_validator_builder = move |_| Box::new(block_announce_validator) as Box<_>;

	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let import_queue = cumulus_client_service::SharedImportQueue::new(params.import_queue);
	let (network, system_rpc_tx, start_network) = sc_service::build_network(sc_service::BuildNetworkParams {
		config: &parachain_config,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		spawn_handle: task_manager.spawn_handle(),
		import_queue: import_queue.clone(),
		block_announce_validator_builder: Some(Box::new(block_announce_validator_builder)),
		warp_sync: None,
	})?;

	let rpc_extensions_builder = {
		let client = client.clone();

		Box::new(move |_, _| rpc_ext_builder(client.clone()))
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend,
		network: network.clone(),
		system_rpc_tx,
		telemetry: None,
	})?;

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	let announce_block = wrap_announce_block
		.map(|w| (w)(announce_block.clone()))
		.unwrap_or_else(|| announce_block);

	let relay_chain_interface_for_closure = relay_chain_interface.clone();
	if let Some(collator_key) = collator_key {
		let parachain_consensus: Box<dyn ParachainConsensus<Block>> = match consensus {
			Consensus::RelayChain => {
				let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
					task_manager.spawn_handle(),
					client.clone(),
					transaction_pool.clone(),
					prometheus_registry.as_ref(),
					None,
				);
				let relay_chain_interface2 = relay_chain_interface_for_closure.clone();
				Box::new(cumulus_client_consensus_relay_chain::RelayChainConsensus::new(
					para_id,
					proposer_factory,
					move |_, (relay_parent, validation_data)| {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface_for_closure,
								&validation_data,
								para_id,
							);

						async move {
							let time = sp_timestamp::InherentDataProvider::from_system_time();

							let parachain_inherent = parachain_inherent.ok_or_else(|| {
								Box::<dyn std::error::Error + Send + Sync>::from(String::from("error"))
							})?;
							Ok((time, parachain_inherent))
						}
					},
					client.clone(),
					relay_chain_interface2,
				))
			}
			Consensus::Null => Box::new(NullConsensus),
		};

		let params = StartCollatorParams {
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			spawner: task_manager.spawn_handle(),
			task_manager: &mut task_manager,
			para_id,
			parachain_consensus,
			relay_chain_interface,
			collator_key,
			import_queue,
			relay_chain_slot_duration: Duration::from_secs(6),
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id,
			relay_chain_interface,
			import_queue,
			// The slot duration is currently used internally only to configure
			// the recovery delay of pov-recovery. We don't want to wait for too
			// long on the full node to recover, so we reduce this time here.
			relay_chain_slot_duration: Duration::from_millis(6),
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client, network, rpc_handlers, transaction_pool))
}

/// A Cumulus test node instance used for testing.
pub struct TestNode {
	/// TaskManager's instance.
	pub task_manager: TaskManager,
	/// Client's instance.
	pub client: Arc<Client>,
	/// Node's network.
	pub network: Arc<NetworkService<Block, H256>>,
	/// The `MultiaddrWithPeerId` to this node. This is useful if you want to pass it as "boot node"
	/// to other nodes.
	pub addr: MultiaddrWithPeerId,
	/// RPCHandlers to make RPC queries.
	pub rpc_handlers: RpcHandlers,
	/// Node's transaction pool
	pub transaction_pool: TransactionPool,
}

enum Consensus {
	/// Use the relay-chain provided consensus.
	RelayChain,
	/// Use the null consensus that will never produce any block.
	Null,
}

/// A builder to create a [`TestNode`].
pub struct TestNodeBuilder {
	para_id: ParaId,
	tokio_handle: tokio::runtime::Handle,
	key: Sr25519Keyring,
	collator_key: Option<CollatorPair>,
	parachain_nodes: Vec<MultiaddrWithPeerId>,
	parachain_nodes_exclusive: bool,
	relay_chain_nodes: Vec<MultiaddrWithPeerId>,
	wrap_announce_block: Option<Box<dyn FnOnce(AnnounceBlockFn) -> AnnounceBlockFn>>,
	storage_update_func_parachain: Option<Box<dyn Fn()>>,
	storage_update_func_relay_chain: Option<Box<dyn Fn()>>,
	consensus: Consensus,
}

impl TestNodeBuilder {
	/// Create a new instance of `Self`.
	///
	/// `para_id` - The parachain id this node is running for.
	/// `tokio_handle` - The tokio handler to use.
	/// `key` - The key that will be used to generate the name and that will be passed as
	/// `dev_seed`.
	pub fn new(para_id: ParaId, tokio_handle: tokio::runtime::Handle, key: Sr25519Keyring) -> Self {
		TestNodeBuilder {
			key,
			para_id,
			tokio_handle,
			collator_key: None,
			parachain_nodes: Vec::new(),
			parachain_nodes_exclusive: false,
			relay_chain_nodes: Vec::new(),
			wrap_announce_block: None,
			storage_update_func_parachain: None,
			storage_update_func_relay_chain: None,
			consensus: Consensus::RelayChain,
		}
	}

	/// Enable collator for this node.
	pub fn enable_collator(mut self) -> Self {
		let collator_key = CollatorPair::generate().0;
		self.collator_key = Some(collator_key);
		self
	}

	/// Instruct the node to exclusively connect to registered parachain nodes.
	///
	/// Parachain nodes can be registered using [`Self::connect_to_parachain_node`] and
	/// [`Self::connect_to_parachain_nodes`].
	pub fn exclusively_connect_to_registered_parachain_nodes(mut self) -> Self {
		self.parachain_nodes_exclusive = true;
		self
	}

	/// Make the node connect to the given parachain node.
	///
	/// By default the node will not be connected to any node or will be able to discover any other
	/// node.
	pub fn connect_to_parachain_node(mut self, node: &TestNode) -> Self {
		self.parachain_nodes.push(node.addr.clone());
		self
	}

	/// Make the node connect to the given parachain nodes.
	///
	/// By default the node will not be connected to any node or will be able to discover any other
	/// node.
	pub fn connect_to_parachain_nodes<'a>(mut self, nodes: impl Iterator<Item = &'a TestNode>) -> Self {
		self.parachain_nodes.extend(nodes.map(|n| n.addr.clone()));
		self
	}

	/// Make the node connect to the given relay chain node.
	///
	/// By default the node will not be connected to any node or will be able to discover any other
	/// node.
	pub fn connect_to_relay_chain_node(mut self, node: &polkadot_test_service::PolkadotTestNode) -> Self {
		self.relay_chain_nodes.push(node.addr.clone());
		self
	}

	/// Make the node connect to the given relay chain nodes.
	///
	/// By default the node will not be connected to any node or will be able to discover any other
	/// node.
	pub fn connect_to_relay_chain_nodes<'a>(
		mut self,
		nodes: impl IntoIterator<Item = &'a polkadot_test_service::PolkadotTestNode>,
	) -> Self {
		self.relay_chain_nodes.extend(nodes.into_iter().map(|n| n.addr.clone()));
		self
	}

	/// Wrap the announce block function of this node.
	pub fn wrap_announce_block(mut self, wrap: impl FnOnce(AnnounceBlockFn) -> AnnounceBlockFn + 'static) -> Self {
		self.wrap_announce_block = Some(Box::new(wrap));
		self
	}

	/// Allows accessing the parachain storage before the test node is built.
	pub fn update_storage_parachain(mut self, updater: impl Fn() + 'static) -> Self {
		self.storage_update_func_parachain = Some(Box::new(updater));
		self
	}

	/// Allows accessing the relay chain storage before the test node is built.
	pub fn update_storage_relay_chain(mut self, updater: impl Fn() + 'static) -> Self {
		self.storage_update_func_relay_chain = Some(Box::new(updater));
		self
	}

	/// Use the null consensus that will never author any block.
	pub fn use_null_consensus(mut self) -> Self {
		self.consensus = Consensus::Null;
		self
	}

	/// Build the [`TestNode`].
	pub async fn build(self) -> TestNode {
		let parachain_config = node_config(
			self.storage_update_func_parachain.unwrap_or_else(|| Box::new(|| ())),
			self.tokio_handle.clone(),
			self.key.clone(),
			self.parachain_nodes,
			self.parachain_nodes_exclusive,
			self.para_id,
			self.collator_key.is_some(),
		)
		.expect("could not generate Configuration");
		let mut relay_chain_config = polkadot_test_service::node_config(
			self.storage_update_func_relay_chain.unwrap_or_else(|| Box::new(|| ())),
			self.tokio_handle,
			self.key,
			self.relay_chain_nodes,
			false,
		);

		relay_chain_config.network.node_name = format!("{} (relay chain)", relay_chain_config.network.node_name);

		let multiaddr = parachain_config.network.listen_addresses[0].clone();
		let (task_manager, client, network, rpc_handlers, transaction_pool) = start_node_impl(
			parachain_config,
			self.collator_key,
			relay_chain_config,
			self.para_id,
			self.wrap_announce_block,
			|_| Ok(Default::default()),
			self.consensus,
		)
		.await
		.expect("could not create Cumulus test service");

		let peer_id = network.local_peer_id().clone();
		let addr = MultiaddrWithPeerId { multiaddr, peer_id };

		TestNode {
			task_manager,
			client,
			network,
			addr,
			rpc_handlers,
			transaction_pool,
		}
	}
}

/// Create a Cumulus `Configuration`.
///
/// By default an in-memory socket will be used, therefore you need to provide nodes if you want the
/// node to be connected to other nodes. If `nodes_exclusive` is `true`, the node will only connect
/// to the given `nodes` and not to any other node. The `storage_update_func` can be used to make
/// adjustments to the runtime genesis.
pub fn node_config(
	storage_update_func: impl Fn(),
	tokio_handle: tokio::runtime::Handle,
	key: Sr25519Keyring,
	nodes: Vec<MultiaddrWithPeerId>,
	nodes_exlusive: bool,
	para_id: ParaId,
	is_collator: bool,
) -> Result<Configuration, ServiceError> {
	let base_path = BasePath::new_temp_dir()?;
	let root = base_path.path().to_path_buf();
	let role = if is_collator { Role::Authority } else { Role::Full };
	let key_seed = key.to_seed();
	let mut spec = Box::new(chain_spec::get_chain_spec(para_id));

	let mut storage = spec
		.as_storage_builder()
		.build_storage()
		.expect("could not build storage");

	BasicExternalities::execute_with_storage(&mut storage, storage_update_func);
	spec.set_storage(storage);

	let mut network_config = NetworkConfiguration::new(
		format!("{} (parachain)", key_seed.to_string()),
		"network/test/0.1",
		Default::default(),
		None,
	);

	if nodes_exlusive {
		network_config.default_peers_set.reserved_nodes = nodes;
		network_config.default_peers_set.non_reserved_mode = sc_network::config::NonReservedPeerMode::Deny;
	} else {
		network_config.boot_nodes = nodes;
	}

	network_config.allow_non_globals_in_dht = true;

	network_config
		.listen_addresses
		.push(multiaddr::Protocol::Memory(rand::random()).into());

	network_config.transport = TransportConfig::MemoryOnly;

	Ok(Configuration {
		impl_name: "cumulus-test-node".to_string(),
		impl_version: "0.1".to_string(),
		role,
		tokio_handle,
		transaction_pool: Default::default(),
		network: network_config,
		keystore: KeystoreConfig::InMemory,
		keystore_remote: Default::default(),
		database: DatabaseSource::RocksDb {
			path: root.join("db"),
			cache_size: 128,
		},
		state_cache_size: 67108864,
		state_cache_child_ratio: None,
		state_pruning: PruningMode::ArchiveAll,
		keep_blocks: KeepBlocks::All,
		transaction_storage: TransactionStorageMode::BlockBody,
		chain_spec: spec,
		wasm_method: WasmExecutionMethod::Interpreted,
		// NOTE: we enforce the use of the native runtime to make the errors more debuggable
		execution_strategies: ExecutionStrategies {
			syncing: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			importing: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			block_construction: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			offchain_worker: sc_client_api::ExecutionStrategy::NativeWhenPossible,
			other: sc_client_api::ExecutionStrategy::NativeWhenPossible,
		},
		rpc_http: None,
		rpc_ws: None,
		rpc_ipc: None,
		rpc_ws_max_connections: None,
		rpc_cors: None,
		rpc_methods: Default::default(),
		rpc_max_payload: None,
		ws_max_out_buffer_capacity: None,
		prometheus_config: None,
		telemetry_endpoints: None,
		default_heap_pages: None,
		offchain_worker: OffchainWorkerConfig {
			enabled: true,
			indexing_enabled: false,
		},
		force_authoring: false,
		disable_grandpa: false,
		dev_key_seed: Some(key_seed),
		tracing_targets: None,
		tracing_receiver: Default::default(),
		max_runtime_instances: 8,
		announce_block: true,
		base_path: Some(base_path),
		informant_output_format: Default::default(),
		wasm_runtime_overrides: None,
		runtime_cache_size: 2,
	})
}

impl TestNode {
	/// Wait for `count` blocks to be imported in the node and then exit. This function will not
	/// return if no blocks are ever created, thus you should restrict the maximum amount of time of
	/// the test execution.
	pub fn wait_for_blocks(&self, count: usize) -> impl Future<Output = ()> {
		self.client.wait_for_blocks(count)
	}

	/// Send an extrinsic to this node.
	pub async fn send_extrinsic(
		&self,
		function: impl Into<runtime::Call>,
		caller: Sr25519Keyring,
	) -> Result<RpcTransactionOutput, RpcTransactionError> {
		let extrinsic = construct_extrinsic(&*self.client, function, caller.pair(), Some(0));

		self.rpc_handlers.send_transaction(extrinsic.into()).await
	}

	/// Register a parachain at this relay chain.
	pub async fn schedule_upgrade(&self, validation: Vec<u8>) -> Result<(), RpcTransactionError> {
		let call = frame_system::Call::set_code { code: validation };

		self.send_extrinsic(
			runtime::SudoCall::sudo_unchecked_weight {
				call: Box::new(call.into()),
				weight: 1_000,
			},
			Sr25519Keyring::Alice,
		)
		.await
		.map(drop)
	}
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
	let current_block_hash = client.info().best_hash;
	let current_block = client.info().best_number.saturated_into();
	let genesis_block = client.hash(0).unwrap().unwrap();
	let nonce = nonce.unwrap_or_else(|| fetch_nonce(client, caller.public()));
	let period = runtime::BlockHashCount::get()
		.checked_next_power_of_two()
		.map(|c| c / 2)
		.unwrap_or(2) as u64;
	let tip = 0;
	let extra: runtime::SignedExtra = (
		frame_system::CheckNonZeroSender::<runtime::Runtime>::new(),
		frame_system::CheckSpecVersion::<runtime::Runtime>::new(),
		frame_system::CheckGenesis::<runtime::Runtime>::new(),
		frame_system::CheckEra::<runtime::Runtime>::from(generic::Era::mortal(period, current_block)),
		frame_system::CheckNonce::<runtime::Runtime>::from(nonce),
		frame_system::CheckWeight::<runtime::Runtime>::new(),
		pallet_transaction_payment::ChargeTransactionPayment::<runtime::Runtime>::from(tip),
	);
	let raw_payload = runtime::SignedPayload::from_raw(
		function.clone(),
		extra.clone(),
		(
			(),
			runtime::VERSION.spec_version,
			genesis_block,
			current_block_hash,
			(),
			(),
			(),
		),
	);
	let signature = raw_payload.using_encoded(|e| caller.sign(e));
	runtime::UncheckedExtrinsic::new_signed(
		function.clone(),
		caller.public().into(),
		runtime::Signature::Sr25519(signature.clone()),
		extra.clone(),
	)
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
