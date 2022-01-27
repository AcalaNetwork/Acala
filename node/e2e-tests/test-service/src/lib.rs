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
mod genesis;

use futures::channel::{mpsc, oneshot};
use std::{future::Future, time::Duration};

use cumulus_client_consensus_common::{ParachainCandidate, ParachainConsensus};
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_local::RelayChainLocal;
use node_runtime::{Block, BlockId, Hash, Header, Runtime, RuntimeApi, SignedExtra};

use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use frame_system_rpc_runtime_api::AccountNonceApi;
use futures::channel::mpsc::Sender;
use futures::SinkExt;
use parking_lot::Mutex;
use polkadot_primitives::v1::{CollatorPair, Hash as PHash, PersistedValidationData};
use polkadot_service::ProvideRuntimeApi;
use sc_client_api::execution_extensions::ExecutionStrategies;
use sc_client_api::{Backend, CallExecutor, ExecutorProvider};
use sc_consensus::LongestChain;
use sc_consensus_aura::{ImportQueueParams, StartAuraParams};
use sc_consensus_manual_seal::rpc::{ManualSeal, ManualSealApi};
use sc_consensus_manual_seal::EngineCommand;
use sc_executor::NativeElseWasmExecutor;
use sc_network::{config::TransportConfig, multiaddr, NetworkService};
use sc_service::{
	config::{
		DatabaseSource, KeepBlocks, KeystoreConfig, MultiaddrWithPeerId, NetworkConfiguration, OffchainWorkerConfig,
		PruningMode, TransactionStorageMode, WasmExecutionMethod,
	},
	BasePath, ChainSpec, Configuration, PartialComponents, Role, RpcHandlers, SpawnTasksParams, TFullBackend,
	TFullCallExecutor, TFullClient, TaskManager,
};
use sc_transaction_pool_api::TransactionPool;
use sp_arithmetic::traits::SaturatedConversion;
use sp_blockchain::HeaderBackend;
use sp_core::{ExecutionContext, Pair, H256};
use sp_keyring::Sr25519Keyring;
use sp_runtime::{
	codec::Encode,
	generic,
	traits::{BlakeTwo256, Extrinsic},
	MultiAddress,
};
use sp_state_machine::BasicExternalities;
use sp_trie::PrefixedMemoryDB;
use std::sync::Arc;
use substrate_test_client::{BlockchainEventsExt, RpcHandlersExt, RpcTransactionError, RpcTransactionOutput};

pub use genesis::*;
use node_primitives::signature::AcalaMultiSignature;
use node_primitives::{AccountId, Address, Balance, Signature};
pub use node_runtime as runtime;
use node_service::chain_spec::mandala::dev_testnet_config;
use sp_api::{OverlayedChanges, StorageTransactionCache};
pub use sp_keyring::Sr25519Keyring as Keyring;
use sp_runtime::generic::Era;
use sp_runtime::traits::IdentifyAccount;
use sp_runtime::transaction_validity::TransactionSource;
use sp_state_machine::Ext;
use substrate_test_client::sp_consensus::SlotData;

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

/// Starts a `ServiceBuilder` for a full service.
///
/// Use this macro if you don't actually need the full service, but just the builder in order to
/// be able to perform chain operations.
pub fn new_partial(
	config: &Configuration,
	seal_mode: SealMode,
) -> Result<
	PartialComponents<
		Client,
		TFullBackend<Block>,
		MaybeFullSelectChain,
		sc_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		sc_transaction_pool::FullPool<Block, Client>,
		(),
	>,
	sc_service::Error,
> {
	let executor = NativeElseWasmExecutor::<RuntimeExecutor>::new(
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

	let (import_queue, select_chain) = match seal_mode {
		SealMode::DevInstantSeal => {
			// instance sealing
			(
				sc_consensus_manual_seal::import_queue(
					Box::new(client.clone()),
					&task_manager.spawn_essential_handle(),
					registry,
				),
				Some(LongestChain::new(backend.clone())),
			)
		}
		SealMode::DevAuraSeal => {
			// aura import queue
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();

			(
				sc_consensus_aura::import_queue::<sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _>(
					ImportQueueParams {
						block_import: client.clone(),
						justification_import: None,
						client: client.clone(),
						create_inherent_data_providers: move |_, ()| async move {
							let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

							let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
								*timestamp,
								slot_duration,
							);

							Ok((
								timestamp,
								slot,
								node_service::default_mock_parachain_inherent_data_provider(),
							))
						},
						spawner: &task_manager.spawn_essential_handle(),
						registry,
						can_author_with: sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
						check_for_equivocation: Default::default(),
						telemetry: None,
					},
				)?,
				None,
			)
		}
		SealMode::ParaSeal => {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;
			let create_inherent_data_providers = Box::new(move |_, _| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
					*timestamp,
					slot_duration.slot_duration(),
				);

				Ok((timestamp, slot))
			});

			(
				cumulus_client_consensus_aura::import_queue::<
					sp_consensus_aura::sr25519::AuthorityPair,
					_,
					_,
					_,
					_,
					_,
					_,
				>(cumulus_client_consensus_aura::ImportQueueParams {
					block_import: client.clone(),
					client: client.clone(),
					create_inherent_data_providers,
					registry,
					can_author_with: sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
					spawner: &task_manager.spawn_essential_handle(),
					telemetry: None,
				})?,
				None,
			)
		}
	};

	let params = PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain,
		other: (),
	};

	Ok(params)
}

async fn start_dev_node(
	config: Configuration,
	seal_mode: SealMode,
) -> sc_service::error::Result<(
	TaskManager,
	Arc<Client>,
	Arc<NetworkService<Block, H256>>,
	RpcHandlers,
	TxPool,
	Arc<TFullBackend<Block>>,
	Sender<EngineCommand<H256>>,
)> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain: maybe_select_chain,
		transaction_pool,
		other: (),
	} = new_partial(&config, SealMode::DevInstantSeal)?;

	let (network, system_rpc_tx, network_starter) = sc_service::build_network(sc_service::BuildNetworkParams {
		config: &config,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		spawn_handle: task_manager.spawn_handle(),
		import_queue,
		block_announce_validator_builder: None,
		warp_sync: None,
	})?;

	// offchain workers
	sc_service::build_offchain_workers(&config, task_manager.spawn_handle(), client.clone(), network.clone());

	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks: Option<()> = None;
	let select_chain =
		maybe_select_chain.expect("In mandala dev mode, `new_partial` will return some `select_chain`; qed");

	let proposer_factory = sc_basic_authorship::ProposerFactory::new(
		task_manager.spawn_handle(),
		client.clone(),
		transaction_pool.clone(),
		config.prometheus_registry(),
		None,
	);
	// Channel for the rpc handler to communicate with the authorship task.
	let (command_sink, commands_stream) = mpsc::channel(10);
	let rpc_sink = command_sink.clone();

	match seal_mode {
		SealMode::DevInstantSeal => {
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();
			let create_inherent_data_providers = Box::new(move |_, _| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
					*timestamp,
					slot_duration,
				);

				Ok((timestamp, slot))
				// Ok(timestamp)
			});
			let authorship_future =
				sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
					block_import: client.clone(),
					env: proposer_factory,
					client: client.clone(),
					pool: transaction_pool.clone(),
					commands_stream,
					select_chain,
					consensus_data_provider: None,
					create_inherent_data_providers,
				});
			// we spawn the future on a background thread managed by service.
			task_manager.spawn_essential_handle().spawn_blocking(
				"instant-seal",
				Some("block-authoring"),
				authorship_future,
			);
		}
		SealMode::DevAuraSeal => {
			// aura
			let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();
			let aura = sc_consensus_aura::start_aura::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
				_,
				_,
				_,
				_,
				_,
			>(StartAuraParams {
				slot_duration: sc_consensus_aura::slot_duration(&*client)?,
				client: client.clone(),
				select_chain,
				// block_import: instant_finalize::InstantFinalizeBlockImport::new(client.clone()),
				block_import: client.clone(),
				proposer_factory,
				create_inherent_data_providers: move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
						*timestamp,
						slot_duration,
					);

					Ok((
						timestamp,
						slot,
						node_service::default_mock_parachain_inherent_data_provider(),
					))
				},
				force_authoring,
				backoff_authoring_blocks,
				keystore: keystore_container.sync_keystore(),
				can_author_with,
				sync_oracle: network.clone(),
				justification_sync_link: network.clone(),
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry: None,
			})?;

			// the AURA authoring task is considered essential, i.e. if it
			// fails we take down the service with it.
			task_manager
				.spawn_essential_handle()
				.spawn_blocking("aura", Some("block-authoring"), aura);
		}
		_ => {
			panic!("dev mode do not support parachain consensus")
		}
	}

	let rpc_handlers = sc_service::spawn_tasks(SpawnTasksParams {
		config,
		client: client.clone(),
		backend: backend.clone(),
		task_manager: &mut task_manager,
		keystore: keystore_container.sync_keystore(),
		transaction_pool: transaction_pool.clone(),
		rpc_extensions_builder: Box::new(move |_, _| {
			let mut io = jsonrpc_core::IoHandler::default();
			io.extend_with(ManualSealApi::to_delegate(ManualSeal::new(rpc_sink.clone())));
			Ok(io)
		}),
		network: network.clone(),
		system_rpc_tx,
		telemetry: None,
	})?;

	network_starter.start_network();

	Ok((
		task_manager,
		client,
		network,
		rpc_handlers,
		transaction_pool,
		backend,
		command_sink,
	))
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
	seal_mode: SealMode,
) -> sc_service::error::Result<(
	TaskManager,
	Arc<Client>,
	Arc<NetworkService<Block, H256>>,
	RpcHandlers,
	TxPool,
	Arc<TFullBackend<Block>>,
	Sender<EngineCommand<H256>>,
)>
where
	RB: Fn(Arc<Client>) -> Result<jsonrpc_core::IoHandler<sc_rpc::Metadata>, sc_service::Error> + Send + 'static,
{
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config, seal_mode.clone())?;
	let keystore = params.keystore_container.sync_keystore();
	let force_authoring = parachain_config.force_authoring;

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
	let backend_for_node = backend.clone();

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
			Consensus::Aura => {
				let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

				let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
					task_manager.spawn_handle(),
					client.clone(),
					transaction_pool.clone(),
					prometheus_registry.as_ref(),
					None,
				);

				AuraConsensus::build::<sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _>(
					BuildAuraConsensusParams {
						proposer_factory,
						create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
							let parachain_inherent =
								cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
									relay_parent,
									&relay_chain_interface_for_closure,
									&validation_data,
									para_id,
								);
							async move {
								let time = sp_timestamp::InherentDataProvider::from_system_time();

								let slot =
									sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
										*time,
										slot_duration.slot_duration(),
									);

								let parachain_inherent = parachain_inherent.ok_or_else(|| {
									Box::<dyn std::error::Error + Send + Sync>::from(
										"Failed to create parachain inherent",
									)
								})?;
								Ok((time, slot, parachain_inherent))
							}
						},
						block_import: client.clone(),
						para_client: client.clone(),
						backoff_authoring_blocks: Option::<()>::None,
						sync_oracle: network.clone(),
						keystore,
						force_authoring,
						slot_duration,
						// We got around 500ms for proposing
						block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
						// And a maximum of 750ms if slots are skipped
						max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
						telemetry: None,
					},
				)
			}
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
	let (command_sink, _) = mpsc::channel(1);

	Ok((
		task_manager,
		client,
		network,
		rpc_handlers,
		transaction_pool,
		backend_for_node,
		command_sink,
	))
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
	pub transaction_pool: TxPool,
	/// Nodes' backend
	pub backend: Arc<TFullBackend<Block>>,
	/// manual instant seal sink command
	pub seal_sink: Sender<EngineCommand<H256>>,
}

enum Consensus {
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
	seal_mode: SealMode,
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
			consensus: Consensus::Aura,
			seal_mode: SealMode::ParaSeal,
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

	/// Use the relay-chain consensus.
	pub fn use_relay_consensus(mut self) -> Self {
		self.consensus = Consensus::RelayChain;
		self
	}

	/// Enable collator for this node.
	pub fn with_seal_mode(mut self, seal_mode: SealMode) -> Self {
		self.seal_mode = seal_mode;
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
			self.collator_key.is_some(),
		)
		.expect("could not generate Configuration");

		// start relay-chain full node inside para-chain
		let mut relay_chain_config = polkadot_test_service::node_config(
			self.storage_update_func_relay_chain.unwrap_or_else(|| Box::new(|| ())),
			self.tokio_handle,
			self.key,
			self.relay_chain_nodes,
			false,
		);

		relay_chain_config.network.node_name = format!("{} (relay chain)", relay_chain_config.network.node_name);

		let multiaddr = parachain_config.network.listen_addresses[0].clone();
		let (task_manager, client, network, rpc_handlers, transaction_pool, backend, seal_sink) = match self.seal_mode {
			SealMode::DevInstantSeal | SealMode::DevAuraSeal => {
				log::info!("start as standalone dev node.");
				start_dev_node(parachain_config, self.seal_mode)
					.await
					.expect("could not start dev node!")
			}
			SealMode::ParaSeal => {
				log::info!("start as parachain node.");
				start_node_impl(
					parachain_config,
					self.collator_key,
					relay_chain_config,
					self.para_id,
					self.wrap_announce_block,
					|_| Ok(Default::default()),
					self.consensus,
					self.seal_mode,
				)
				.await
				.expect("could not create collator!")
			}
		};

		let peer_id = network.local_peer_id().clone();
		let addr = MultiaddrWithPeerId { multiaddr, peer_id };

		TestNode {
			task_manager,
			client,
			network,
			addr,
			rpc_handlers,
			transaction_pool,
			backend,
			seal_sink,
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
	is_collator: bool,
) -> Result<Configuration, sc_service::Error> {
	let base_path = BasePath::new_temp_dir()?;
	let root = base_path.path().to_path_buf();
	let role = if is_collator { Role::Authority } else { Role::Full };
	let key_seed = key.to_seed();
	let mut spec = Box::new(dev_testnet_config(None).unwrap());

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

	/// Instructs manual seal to seal new, possibly empty blocks.
	pub async fn seal_blocks(&self, num: usize) {
		let mut sink = self.seal_sink.clone();

		for count in 0..num {
			let (sender, future_block) = oneshot::channel();
			let future = sink.send(EngineCommand::SealNewBlock {
				create_empty: true,
				finalize: false,
				parent_hash: None,
				sender: Some(sender),
			});

			const ERROR: &'static str = "manual-seal authorship task is shutting down";
			future.await.expect(ERROR);

			match future_block.await.expect(ERROR) {
				Ok(block) => {
					log::info!("sealed {} (hash: {}) of {} blocks", count + 1, block.hash, num)
				}
				Err(err) => {
					log::error!("failed to seal block {} of {}, error: {:?}", count + 1, num, err)
				}
			}
		}
	}

	/// Submit an extrinsic to transaction pool.
	pub async fn submit_extrinsic(
		&self,
		function: impl Into<runtime::Call>,
		caller: Option<Sr25519Keyring>,
	) -> Result<H256, sc_transaction_pool::error::Error> {
		let extrinsic = match caller {
			Some(caller) => construct_extrinsic(&*self.client, function, caller.pair(), Some(0)),
			None => runtime::UncheckedExtrinsic::new(function.into(), None).unwrap(),
		};
		let at = self.client.info().best_hash;

		self.transaction_pool
			.submit_one(&BlockId::Hash(at), TransactionSource::Local, extrinsic.into())
			.await
	}

	/// Executes closure in an externalities provided environment.
	pub fn with_state<R>(&self, closure: impl FnOnce() -> R) -> R
	where
		<TFullCallExecutor<Block, NativeElseWasmExecutor<RuntimeExecutor>> as CallExecutor<Block>>::Error:
			std::fmt::Debug,
	{
		let id = BlockId::Hash(self.client.info().best_hash);
		let mut overlay = OverlayedChanges::default();
		let mut cache = StorageTransactionCache::<Block, <TFullBackend<Block> as Backend<Block>>::State>::default();
		let mut extensions = self
			.client
			.execution_extensions()
			.extensions(&id, ExecutionContext::BlockConstruction);
		let state_backend = self
			.backend
			.state_at(id.clone())
			.expect(&format!("State at block {} not found", id));

		let mut ext = Ext::new(&mut overlay, &mut cache, &state_backend, Some(&mut extensions));
		sp_externalities::set_and_run_with_externalities(&mut ext, closure)
	}

	/// Send an extrinsic to this node.
	pub async fn send_extrinsic(
		&self,
		function: impl Into<runtime::Call>,
		caller: Sr25519Keyring,
	) -> Result<RpcTransactionOutput, RpcTransactionError> {
		let extrinsic = construct_extrinsic(&*self.client, function, caller.pair(), Some(0));

		self.rpc_handlers.send_transaction(extrinsic.0.into()).await
	}

	/// Register a parachain at this relay chain.
	pub async fn schedule_upgrade(&self, validation: Vec<u8>) -> Result<(), RpcTransactionError> {
		let call = frame_system::Call::set_code { code: validation };

		self.send_extrinsic(
			pallet_sudo::Call::sudo_unchecked_weight {
				call: Box::new(call.into()),
				weight: 1_000,
			},
			Sr25519Keyring::Alice,
		)
		.await
		.map(drop)
	}

	/// Transfer some token from one account to another using a provided test [`Client`].
	pub async fn transfer(
		&self,
		origin: sp_keyring::AccountKeyring,
		dest: sp_keyring::AccountKeyring,
		value: Balance,
	) -> Result<(), RpcTransactionError> {
		let function = node_runtime::Call::Balances(pallet_balances::Call::transfer_keep_alive {
			dest: MultiAddress::Id(dest.public().into_account().into()),
			value,
		});

		self.send_extrinsic(function, origin).await.map(drop)
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
	let signed_data: (Address, AcalaMultiSignature, SignedExtra) =
		(address, Signature::Sr25519(signature.clone()), extra.clone());
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
