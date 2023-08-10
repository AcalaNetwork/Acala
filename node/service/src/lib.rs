// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

// Disable the following lints
#![allow(clippy::type_complexity)]

//! Acala service. Specialized wrapper over substrate service.

pub use acala_primitives::{Block, Hash};
use cumulus_client_cli::CollatorOptions;
use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::{ParachainBlockImport as TParachainBlockImport, ParachainConsensus};
use cumulus_client_network::RequireSecondedInBlockAnnounce;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_primitives_parachain_inherent::{MockValidationDataInherentDataProvider, MockXcmConfig};
use cumulus_relay_chain_inprocess_interface::build_inprocess_relay_chain;
use cumulus_relay_chain_interface::{RelayChainInterface, RelayChainResult};
use cumulus_relay_chain_minimal_node::build_minimal_relay_chain_node;
use futures::{channel::oneshot, FutureExt, StreamExt};
use jsonrpsee::RpcModule;
use polkadot_primitives::{CollatorPair, OccupiedCoreAssumption};
use sc_client_api::Backend;
use sc_consensus::{ImportQueue, LongestChain};
use sc_consensus_aura::{ImportQueueParams, StartAuraParams};
use sc_executor::{HeapAllocStrategy, WasmExecutor, DEFAULT_HEAP_ALLOC_STRATEGY};
use sc_network::{config::SyncMode, NetworkBlock};
use sc_network_sync::SyncingService;
pub use sc_service::{
	config::{DatabaseSource, PrometheusConfig},
	ChainSpec, SpawnTaskHandle, WarpSyncParams,
};
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, TFullBackend, TFullClient, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sc_transaction_pool_api::OffchainTransactionPoolFactory;
pub use sp_api::ConstructRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_consensus_aura::sr25519::{AuthorityId as AuraId, AuthorityPair as AuraPair};
use sp_core::Decode;
use sp_keystore::KeystorePtr;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};
use sp_trie::PrefixedMemoryDB;
use std::{sync::Arc, time::Duration};
use substrate_prometheus_endpoint::Registry;

pub use client::*;

pub mod chain_spec;
mod client;
pub mod instant_finalize;

const LOG_TARGET_SYNC: &str = "sync::cumulus";

#[cfg(not(feature = "runtime-benchmarks"))]
type HostFunctions = sp_io::SubstrateHostFunctions;

#[cfg(feature = "runtime-benchmarks")]
type HostFunctions = (
	sp_io::SubstrateHostFunctions,
	frame_benchmarking::benchmarking::HostFunctions,
);

#[cfg(feature = "with-mandala-runtime")]
mod mandala_executor {
	pub use mandala_runtime;

	pub struct MandalaExecutorDispatch;
	impl sc_executor::NativeExecutionDispatch for MandalaExecutorDispatch {
		type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

		fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
			mandala_runtime::api::dispatch(method, data)
		}

		fn native_version() -> sc_executor::NativeVersion {
			mandala_runtime::native_version()
		}
	}
}

#[cfg(feature = "with-karura-runtime")]
mod karura_executor {
	pub use karura_runtime;

	pub struct KaruraExecutorDispatch;
	impl sc_executor::NativeExecutionDispatch for KaruraExecutorDispatch {
		type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

		fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
			karura_runtime::api::dispatch(method, data)
		}

		fn native_version() -> sc_executor::NativeVersion {
			karura_runtime::native_version()
		}
	}
}

#[cfg(feature = "with-acala-runtime")]
mod acala_executor {
	pub use acala_runtime;

	pub struct AcalaExecutorDispatch;
	impl sc_executor::NativeExecutionDispatch for AcalaExecutorDispatch {
		type ExtendHostFunctions = frame_benchmarking::benchmarking::HostFunctions;

		fn dispatch(method: &str, data: &[u8]) -> Option<Vec<u8>> {
			acala_runtime::api::dispatch(method, data)
		}

		fn native_version() -> sc_executor::NativeVersion {
			acala_runtime::native_version()
		}
	}
}

#[cfg(feature = "with-acala-runtime")]
pub use acala_executor::*;
#[cfg(feature = "with-karura-runtime")]
pub use karura_executor::*;
#[cfg(feature = "with-mandala-runtime")]
pub use mandala_executor::*;

/// Can be called for a `Configuration` to check if it is a configuration for
/// the `Acala` network.
pub trait IdentifyVariant {
	/// Returns `true` if this is a configuration for the `Acala` network.
	fn is_acala(&self) -> bool;

	/// Returns `true` if this is a configuration for the `Karura` network.
	fn is_karura(&self) -> bool;

	/// Returns `true` if this is a configuration for the `Mandala` network.
	fn is_mandala(&self) -> bool;

	/// Returns `true` if this is a configuration for the dev network.
	fn is_dev(&self) -> bool;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
	fn is_acala(&self) -> bool {
		self.id().starts_with("acala")
	}

	fn is_karura(&self) -> bool {
		self.id().starts_with("karura")
	}

	fn is_mandala(&self) -> bool {
		self.id().starts_with("mandala")
	}

	fn is_dev(&self) -> bool {
		self.id().ends_with("dev")
	}
}

/// Acala's full backend.
type FullBackend = TFullBackend<Block>;

/// Acala's full client.
type FullClient<RuntimeApi> = TFullClient<Block, RuntimeApi, WasmExecutor<HostFunctions>>;

type ParachainBlockImport<RuntimeApi> = TParachainBlockImport<Block, Arc<FullClient<RuntimeApi>>, FullBackend>;

/// Maybe Mandala Dev full select chain.
type MaybeFullSelectChain = Option<LongestChain<FullBackend, Block>>;

pub fn new_partial<RuntimeApi>(
	config: &Configuration,
	dev: bool,
	instant_sealing: bool,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi>,
		FullBackend,
		MaybeFullSelectChain,
		sc_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>,
		(
			ParachainBlockImport<RuntimeApi>,
			Option<Telemetry>,
			Option<TelemetryWorkerHandle>,
		),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
{
	let telemetry = config
		.telemetry_endpoints
		.clone()
		.filter(|x| !x.is_empty())
		.map(|endpoints| -> Result<_, sc_telemetry::Error> {
			let worker = TelemetryWorker::new(16)?;
			let telemetry = worker.handle().new_telemetry(endpoints);
			Ok((worker, telemetry))
		})
		.transpose()?;

	let heap_pages = config
		.default_heap_pages
		.map_or(DEFAULT_HEAP_ALLOC_STRATEGY, |h| HeapAllocStrategy::Static {
			extra_pages: h as _,
		});

	let executor = WasmExecutor::<HostFunctions>::builder()
		.with_execution_method(config.wasm_method)
		.with_onchain_heap_alloc_strategy(heap_pages)
		.with_offchain_heap_alloc_strategy(heap_pages)
		.with_max_runtime_instances(config.max_runtime_instances)
		.with_runtime_cache_size(config.runtime_cache_size)
		.build();

	let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, _>(
		config,
		telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
		executor,
	)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", None, worker.run());
		telemetry
	});

	let registry = config.prometheus_registry();

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		registry,
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let block_import = ParachainBlockImport::new(client.clone(), backend.clone());

	let select_chain = if dev {
		Some(LongestChain::new(backend.clone()))
	} else {
		None
	};

	let import_queue = if dev {
		if instant_sealing {
			// instance sealing
			sc_consensus_manual_seal::import_queue(
				Box::new(client.clone()),
				&task_manager.spawn_essential_handle(),
				registry,
			)
		} else {
			// aura import queue
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
			let client_for_cidp = client.clone();

			sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(ImportQueueParams {
				block_import: block_import.clone(),
				justification_import: None,
				client: client.clone(),
				create_inherent_data_providers: move |block: Hash, ()| {
					let current_para_block = client_for_cidp
						.number(block)
						.expect("Header lookup should succeed")
						.expect("Header passed in as parent should be present in backend.");
					let client_for_xcm = client_for_cidp.clone();

					async move {
						let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

						let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

						let mocked_parachain = MockValidationDataInherentDataProvider {
							current_para_block,
							relay_offset: 1000,
							relay_blocks_per_para_block: 2,
							para_blocks_per_relay_epoch: 0,
							relay_randomness_config: (),
							xcm_config: MockXcmConfig::new(
								&*client_for_xcm,
								block,
								Default::default(),
								Default::default(),
							),
							raw_downward_messages: vec![],
							raw_horizontal_messages: vec![],
						};

						Ok((slot, timestamp, mocked_parachain))
					}
				},
				spawner: &task_manager.spawn_essential_handle(),
				registry,
				check_for_equivocation: Default::default(),
				telemetry: telemetry.as_ref().map(|x| x.handle()),
				compatibility_mode: Default::default(),
			})?
		}
	} else {
		let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

		cumulus_client_consensus_aura::import_queue::<AuraPair, _, _, _, _, _>(
			cumulus_client_consensus_aura::ImportQueueParams {
				block_import: block_import.clone(),
				client: client.clone(),
				create_inherent_data_providers: move |_, _| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
						*timestamp,
						slot_duration,
					);

					Ok((slot, timestamp))
				},
				registry,
				spawner: &task_manager.spawn_essential_handle(),
				telemetry: telemetry.as_ref().map(|telemetry| telemetry.handle()),
			},
		)?
	};

	Ok(PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		select_chain,
		other: (block_import, telemetry, telemetry_worker_handle),
	})
}

/// Build a relay chain interface.
/// Will return a minimal relay chain node with RPC
/// client or an inprocess node, based on the [`CollatorOptions`] passed in.
async fn build_relay_chain_interface(
	polkadot_config: Configuration,
	parachain_config: &Configuration,
	telemetry_worker_handle: Option<TelemetryWorkerHandle>,
	task_manager: &mut TaskManager,
	collator_options: CollatorOptions,
) -> RelayChainResult<(Arc<(dyn RelayChainInterface + 'static)>, Option<CollatorPair>)> {
	if !collator_options.relay_chain_rpc_urls.is_empty() {
		build_minimal_relay_chain_node(polkadot_config, task_manager, collator_options.relay_chain_rpc_urls).await
	} else {
		build_inprocess_relay_chain(
			polkadot_config,
			parachain_config,
			telemetry_worker_handle,
			task_manager,
			None,
		)
	}
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RB, RuntimeApi, BIC>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
	_rpc_ext_builder: RB,
	build_consensus: BIC,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RB: Fn(Arc<FullClient<RuntimeApi>>) -> Result<RpcModule<()>, sc_service::Error> + Send + 'static,
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
	BIC: FnOnce(
		Arc<FullClient<RuntimeApi>>,
		ParachainBlockImport<RuntimeApi>,
		Option<&Registry>,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<dyn RelayChainInterface>,
		Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi>>>,
		Arc<SyncingService<Block>>,
		KeystorePtr,
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config, false, false)?;
	let (block_import, mut telemetry, telemetry_worker_handle) = params.other;
	let net_config = sc_network::config::FullNetworkConfiguration::new(&parachain_config.network);

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) = build_relay_chain_interface(
		polkadot_config,
		&parachain_config,
		telemetry_worker_handle,
		&mut task_manager,
		collator_options.clone(),
	)
	.await
	.map_err(|e| sc_service::Error::Application(Box::new(e) as Box<_>))?;

	let spawn_handle = task_manager.spawn_handle();

	let block_announce_validator = RequireSecondedInBlockAnnounce::new(relay_chain_interface.clone(), para_id);

	let warp_sync_params = match parachain_config.network.sync_mode {
		SyncMode::Warp => {
			let target_block = warp_sync_get::<Block, _>(para_id, relay_chain_interface.clone(), spawn_handle.clone());
			Some(WarpSyncParams::WaitForTarget(target_block))
		}
		_ => None,
	};

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue_service = params.import_queue.service();
	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle,
			import_queue: params.import_queue,
			block_announce_validator_builder: Some(Box::new(|_| Box::new(block_announce_validator))),
			warp_sync_params,
		})?;

	let rpc_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		move |deny_unsafe, _| {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: None,
			};

			acala_rpc::create_full(deps).map_err(Into::into)
		}
	};

	if parachain_config.offchain_worker.enabled {
		use futures::FutureExt;

		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-work",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				keystore: Some(params.keystore_container.keystore()),
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(transaction_pool.clone())),
				network_provider: network.clone(),
				is_validator: parachain_config.role.is_authority(),
				enable_http_requests: false,
				custom_extensions: move |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		network: network.clone(),
		client: client.clone(),
		keystore: params.keystore_container.keystore(),
		task_manager: &mut task_manager,
		transaction_pool: transaction_pool.clone(),
		rpc_builder: Box::new(rpc_builder),
		backend: backend.clone(),
		system_rpc_tx,
		tx_handler_controller,
		config: parachain_config,
		telemetry: telemetry.as_mut(),
		sync_service: sync_service.clone(),
	})?;

	let announce_block = {
		let sync_service = sync_service.clone();
		Arc::new(move |hash, data| sync_service.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	let overseer_handle = relay_chain_interface
		.overseer_handle()
		.map_err(|e| sc_service::Error::Application(Box::new(e)))?;

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			block_import,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			sync_service.clone(),
			params.keystore_container.keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();

		let params = StartCollatorParams {
			para_id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue: import_queue_service,
			collator_key: collator_key.expect("Command line arguments do not allow this. qed"),
			relay_chain_slot_duration,
			recovery_handle: Box::new(overseer_handle),
			sync_service: sync_service.clone(),
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id,
			relay_chain_interface,
			relay_chain_slot_duration,
			import_queue: import_queue_service,
			recovery_handle: Box::new(overseer_handle),
			sync_service: sync_service.clone(),
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node<RuntimeApi>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	collator_options: CollatorOptions,
	para_id: ParaId,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
{
	start_node_impl(
		parachain_config,
		polkadot_config,
		collator_options,
		para_id,
		|_| Ok(RpcModule::new(())),
		|client,
		 block_import,
		 prometheus_registry,
		 telemetry,
		 task_manager,
		 relay_chain_interface,
		 transaction_pool,
		 sync_oracle,
		 keystore,
		 force_authoring| {
			let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

			let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
				task_manager.spawn_handle(),
				client.clone(),
				transaction_pool,
				prometheus_registry,
				telemetry.clone(),
			);

			Ok(AuraConsensus::build::<
				sp_consensus_aura::sr25519::AuthorityPair,
				_,
				_,
				_,
				_,
				_,
				_,
			>(BuildAuraConsensusParams {
				proposer_factory,
				create_inherent_data_providers: move |_, (relay_parent, validation_data)| {
					let relay_chain_interface = relay_chain_interface.clone();
					async move {
						let parachain_inherent =
							cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
								relay_parent,
								&relay_chain_interface,
								&validation_data,
								para_id,
							)
							.await;

						let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

						let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from("Failed to create parachain inherent")
						})?;
						Ok((slot, timestamp, parachain_inherent))
					}
				},
				block_import,
				para_client: client,
				backoff_authoring_blocks: Option::<()>::None,
				sync_oracle,
				keystore,
				force_authoring,
				slot_duration,
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry,
			}))
		},
	)
	.await
}

pub const MANDALA_RUNTIME_NOT_AVAILABLE: &str =
	"Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.";
pub const KARURA_RUNTIME_NOT_AVAILABLE: &str =
	"Karura runtime is not available. Please compile the node with `--features with-karura-runtime` to enable it.";
pub const ACALA_RUNTIME_NOT_AVAILABLE: &str =
	"Acala runtime is not available. Please compile the node with `--features with-acala-runtime` to enable it.";

/// Builds a new object suitable for chain operations.
pub fn new_chain_ops(
	config: &mut Configuration,
) -> Result<
	(
		Arc<Client>,
		Arc<FullBackend>,
		sc_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		TaskManager,
	),
	ServiceError,
> {
	config.keystore = sc_service::config::KeystoreConfig::InMemory;
	if config.chain_spec.is_mandala() {
		#[cfg(feature = "with-mandala-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial(config, config.chain_spec.is_dev(), false)?;
			Ok((Arc::new(Client::Mandala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-mandala-runtime"))]
		Err(MANDALA_RUNTIME_NOT_AVAILABLE.into())
	} else if config.chain_spec.is_karura() {
		#[cfg(feature = "with-karura-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial::<karura_runtime::RuntimeApi>(config, false, false)?;
			Ok((Arc::new(Client::Karura(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-karura-runtime"))]
		Err(KARURA_RUNTIME_NOT_AVAILABLE.into())
	} else {
		#[cfg(feature = "with-acala-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial::<acala_runtime::RuntimeApi>(config, false, false)?;
			Ok((Arc::new(Client::Acala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-acala-runtime"))]
		Err(ACALA_RUNTIME_NOT_AVAILABLE.into())
	}
}

pub fn start_dev_node<RuntimeApi>(config: Configuration, instant_sealing: bool) -> Result<TaskManager, ServiceError>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
{
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain: maybe_select_chain,
		transaction_pool,
		other: (_, _, _),
	} = new_partial::<RuntimeApi>(&config, true, instant_sealing)?;

	let net_config = sc_network::config::FullNetworkConfiguration::new(&config.network);

	let (network, system_rpc_tx, tx_handler_controller, start_network, sync_service) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &config,
			net_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync_params: None,
		})?;

	if config.offchain_worker.enabled {
		task_manager.spawn_handle().spawn(
			"offchain-workers-runner",
			"offchain-work",
			sc_offchain::OffchainWorkers::new(sc_offchain::OffchainWorkerOptions {
				runtime_api_provider: client.clone(),
				keystore: None,
				offchain_db: backend.offchain_storage(),
				transaction_pool: Some(OffchainTransactionPoolFactory::new(transaction_pool.clone())),
				network_provider: network.clone(),
				is_validator: config.role.is_authority(),
				enable_http_requests: false,
				custom_extensions: move |_| vec![],
			})
			.run(client.clone(), task_manager.spawn_handle())
			.boxed(),
		);
	}

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks: Option<()> = None;

	let select_chain = maybe_select_chain.expect("In `dev` mode, `new_partial` will return some `select_chain`; qed");

	let command_sink = if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			None,
			None,
		);

		if instant_sealing {
			// Channel for the rpc handler to communicate with the authorship task.
			let (command_sink, commands_stream) = futures::channel::mpsc::channel(1024);

			let pool = transaction_pool.pool().clone();
			let import_stream = pool.validated_pool().import_notification_stream().map(|_| {
				sc_consensus_manual_seal::rpc::EngineCommand::SealNewBlock {
					create_empty: false,
					finalize: true,
					parent_hash: None,
					sender: None,
				}
			});

			let client_for_cidp = client.clone();

			let authorship_future =
				sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
					block_import: client.clone(),
					env: proposer_factory,
					client: client.clone(),
					pool: transaction_pool.clone(),
					commands_stream: futures::stream_select!(commands_stream, import_stream),
					select_chain,
					consensus_data_provider: None,
					create_inherent_data_providers: move |block: Hash, _| {
						let current_para_block = client_for_cidp
							.number(block)
							.expect("Header lookup should succeed")
							.expect("Header passed in as parent should be present in backend.");
						let client_for_xcm = client_for_cidp.clone();
						async move {
							let mocked_parachain = MockValidationDataInherentDataProvider {
								current_para_block,
								relay_offset: 1000,
								relay_blocks_per_para_block: 2,
								para_blocks_per_relay_epoch: 0,
								relay_randomness_config: (),
								xcm_config: MockXcmConfig::new(
									&*client_for_xcm,
									block,
									Default::default(),
									Default::default(),
								),
								raw_downward_messages: vec![],
								raw_horizontal_messages: vec![],
							};
							Ok((sp_timestamp::InherentDataProvider::from_system_time(), mocked_parachain))
						}
					},
				});
			// we spawn the future on a background thread managed by service.
			task_manager.spawn_essential_handle().spawn_blocking(
				"instant-seal",
				Some("block-authoring"),
				authorship_future,
			);
			Some(command_sink)
		} else {
			// aura
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
			let client_for_cidp = client.clone();

			let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _>(StartAuraParams {
				slot_duration: sc_consensus_aura::slot_duration(&*client)?,
				client: client.clone(),
				select_chain,
				block_import: instant_finalize::InstantFinalizeBlockImport::new(client.clone()),
				proposer_factory,
				create_inherent_data_providers: move |block: Hash, ()| {
					let current_para_block = client_for_cidp
						.number(block)
						.expect("Header lookup should succeed")
						.expect("Header passed in as parent should be present in backend.");
					let client_for_xcm = client_for_cidp.clone();

					async move {
						let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

						let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

						let mocked_parachain = MockValidationDataInherentDataProvider {
							current_para_block,
							relay_offset: 1000,
							relay_blocks_per_para_block: 2,
							para_blocks_per_relay_epoch: 0,
							relay_randomness_config: (),
							xcm_config: MockXcmConfig::new(
								&*client_for_xcm,
								block,
								Default::default(),
								Default::default(),
							),
							raw_downward_messages: vec![],
							raw_horizontal_messages: vec![],
						};

						Ok((slot, timestamp, mocked_parachain))
					}
				},
				force_authoring,
				backoff_authoring_blocks,
				keystore: keystore_container.keystore(),
				sync_oracle: sync_service.clone(),
				justification_sync_link: sync_service.clone(),
				// We got around 500ms for proposing
				block_proposal_slot_portion: SlotProportion::new(1f32 / 24f32),
				// And a maximum of 750ms if slots are skipped
				max_block_proposal_slot_portion: Some(SlotProportion::new(1f32 / 16f32)),
				telemetry: None,
				compatibility_mode: Default::default(),
			})?;

			// the AURA authoring task is considered essential, i.e. if it
			// fails we take down the service with it.
			task_manager
				.spawn_essential_handle()
				.spawn_blocking("aura", Some("block-authoring"), aura);

			None
		}
	} else {
		None
	};

	let rpc_extensions_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		move |deny_unsafe, _| {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: command_sink.clone(),
			};

			acala_rpc::create_full(deps).map_err(Into::into)
		}
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder: Box::new(rpc_extensions_builder),
		client,
		transaction_pool,
		task_manager: &mut task_manager,
		config,
		keystore: keystore_container.keystore(),
		backend,
		network,
		system_rpc_tx,
		tx_handler_controller,
		telemetry: None,
		sync_service,
	})?;

	start_network.start_network();

	Ok(task_manager)
}

/// Creates a new background task to wait for the relay chain to sync up and retrieve the parachain
/// header
fn warp_sync_get<B, RCInterface>(
	para_id: ParaId,
	relay_chain_interface: RCInterface,
	spawner: SpawnTaskHandle,
) -> oneshot::Receiver<<B as BlockT>::Header>
where
	B: BlockT + 'static,
	RCInterface: RelayChainInterface + 'static,
{
	let (sender, receiver) = oneshot::channel::<B::Header>();
	spawner.spawn(
		"cumulus-parachain-wait-for-target-block",
		None,
		async move {
			log::debug!(
				target: "cumulus-network",
				"waiting for announce block in a background task...",
			);

			let _ = wait_for_target_block::<B, _>(sender, para_id, relay_chain_interface)
				.await
				.map_err(|e| {
					log::error!(
						target: LOG_TARGET_SYNC,
						"Unable to determine parachain target block {:?}",
						e
					)
				});
		}
		.boxed(),
	);

	receiver
}
/// Waits for the relay chain to have finished syncing and then gets the parachain header that
/// corresponds to the last finalized relay chain block.
async fn wait_for_target_block<B, RCInterface>(
	sender: oneshot::Sender<<B as BlockT>::Header>,
	para_id: ParaId,
	relay_chain_interface: RCInterface,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
	B: BlockT + 'static,
	RCInterface: RelayChainInterface + Send + 'static,
{
	let mut imported_blocks = relay_chain_interface.import_notification_stream().await?.fuse();
	while imported_blocks.next().await.is_some() {
		let is_syncing = relay_chain_interface.is_major_syncing().await.map_err(|e| {
			Box::<dyn std::error::Error + Send + Sync>::from(format!("Unable to determine sync status. {e}"))
		})?;

		if !is_syncing {
			let relay_chain_best_hash = relay_chain_interface
				.finalized_block_hash()
				.await
				.map_err(|e| Box::new(e) as Box<_>)?;

			let validation_data = relay_chain_interface
				.persisted_validation_data(relay_chain_best_hash, para_id, OccupiedCoreAssumption::TimedOut)
				.await
				.map_err(|e| format!("{e:?}"))?
				.ok_or("Could not find parachain head in relay chain")?;

			let target_block = B::Header::decode(&mut &validation_data.parent_head.0[..])
				.map_err(|e| format!("Failed to decode parachain head: {e}"))?;

			log::debug!(target: LOG_TARGET_SYNC, "Target block reached {:?}", target_block);
			let _ = sender.send(target_block);
			return Ok(());
		}
	}

	Err("Stopping following imported blocks. Could not determine parachain target block".into())
}
