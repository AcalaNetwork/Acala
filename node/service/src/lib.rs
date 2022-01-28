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

// Disable the following lints
#![allow(clippy::type_complexity)]

//! Acala service. Specialized wrapper over substrate service.

use cumulus_client_consensus_aura::{AuraConsensus, BuildAuraConsensusParams, SlotProportion};
use cumulus_client_consensus_common::ParachainConsensus;
use cumulus_client_network::BlockAnnounceValidator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;
use cumulus_relay_chain_interface::RelayChainInterface;
use cumulus_relay_chain_local::build_relay_chain_interface;

use acala_primitives::{Block, Hash};
use cumulus_primitives_parachain_inherent::MockValidationDataInherentDataProvider;
use sc_client_api::ExecutorProvider;
use sc_consensus::LongestChain;
use sc_consensus_aura::ImportQueueParams;
use sc_executor::NativeElseWasmExecutor;
use sc_network::NetworkService;
use sc_service::{error::Error as ServiceError, Configuration, PartialComponents, Role, TFullBackend, TaskManager};
use sc_telemetry::{Telemetry, TelemetryHandle, TelemetryWorker, TelemetryWorkerHandle};
use sp_consensus::SlotData;
use sp_consensus_aura::sr25519::{AuthorityId as AuraId, AuthorityPair as AuraPair};
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::traits::BlakeTwo256;
use sp_trie::PrefixedMemoryDB;
use substrate_prometheus_endpoint::Registry;

use std::{sync::Arc, time::Duration};

pub use client::*;

pub use sc_service::{
	config::{DatabaseSource, PrometheusConfig},
	ChainSpec,
};
pub use sp_api::ConstructRuntimeApi;

pub mod chain_spec;
mod client;
#[cfg(feature = "with-mandala-runtime")]
mod instant_finalize;

pub fn default_mock_parachain_inherent_data_provider() -> MockValidationDataInherentDataProvider {
	MockValidationDataInherentDataProvider {
		current_para_block: 0,
		relay_offset: 1000,
		relay_blocks_per_para_block: 2,
		xcm_config: Default::default(),
		raw_downward_messages: vec![],
		raw_horizontal_messages: vec![],
	}
}

#[cfg(feature = "with-mandala-runtime")]
mod mandala_executor {
	pub use futures::stream::StreamExt;
	pub use mandala_runtime;
	pub use sc_consensus_aura::StartAuraParams;

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

	/// Returns `true` if this is a configuration for the `Mandala` dev network.
	fn is_mandala_dev(&self) -> bool;

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

	fn is_mandala_dev(&self) -> bool {
		self.id().starts_with("mandala-dev")
	}

	fn is_dev(&self) -> bool {
		self.id().ends_with("dev")
	}
}

/// Acala's full backend.
type FullBackend = TFullBackend<Block>;

/// Acala's full client.
type FullClient<RuntimeApi, ExecutorDispatch> =
	sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<ExecutorDispatch>>;

/// Maybe Mandala Dev full select chain.
type MaybeFullSelectChain = Option<LongestChain<FullBackend, Block>>;

pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
	dev: bool,
	instant_sealing: bool,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi, Executor>,
		FullBackend,
		MaybeFullSelectChain,
		sc_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
		(Option<Telemetry>, Option<TelemetryWorkerHandle>),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
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

	let executor = NativeElseWasmExecutor::<Executor>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
		config.runtime_cache_size,
	);

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, NativeElseWasmExecutor<Executor>>(
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
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();

			sc_consensus_aura::import_queue::<AuraPair, _, _, _, _, _, _>(ImportQueueParams {
				block_import: client.clone(),
				justification_import: None,
				client: client.clone(),
				create_inherent_data_providers: move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
						*timestamp,
						slot_duration,
					);

					Ok((timestamp, slot, default_mock_parachain_inherent_data_provider()))
				},
				spawner: &task_manager.spawn_essential_handle(),
				registry,
				can_author_with: sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
				check_for_equivocation: Default::default(),
				telemetry: telemetry.as_ref().map(|x| x.handle()),
			})?
		}
	} else {
		let slot_duration = cumulus_client_consensus_aura::slot_duration(&*client)?;

		cumulus_client_consensus_aura::import_queue::<AuraPair, _, _, _, _, _, _>(
			cumulus_client_consensus_aura::ImportQueueParams {
				block_import: client.clone(),
				client: client.clone(),
				create_inherent_data_providers: move |_, _| async move {
					let time = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
						*time,
						slot_duration.slot_duration(),
					);

					Ok((time, slot))
				},
				registry,
				can_author_with: sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone()),
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
		other: (telemetry, telemetry_worker_handle),
	})
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RB, RuntimeApi, Executor, BIC>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	id: ParaId,
	_rpc_ext_builder: RB,
	build_consensus: BIC,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
	RB: Fn(
			Arc<FullClient<RuntimeApi, Executor>>,
		) -> Result<jsonrpc_core::IoHandler<sc_rpc::Metadata>, sc_service::Error>
		+ Send
		+ 'static,
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
	BIC: FnOnce(
		Arc<FullClient<RuntimeApi, Executor>>,
		Option<&Registry>,
		Option<TelemetryHandle>,
		&TaskManager,
		Arc<dyn RelayChainInterface>,
		Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>>,
		Arc<NetworkService<Block, Hash>>,
		SyncCryptoStorePtr,
		bool,
	) -> Result<Box<dyn ParachainConsensus<Block>>, sc_service::Error>,
{
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config, false, false)?;
	let (mut telemetry, telemetry_worker_handle) = params.other;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let mut task_manager = params.task_manager;

	let (relay_chain_interface, collator_key) =
		build_relay_chain_interface(polkadot_config, telemetry_worker_handle, &mut task_manager).map_err(
			|e| match e {
				polkadot_service::Error::Sub(x) => x,
				s => format!("{}", s).into(),
			},
		)?;
	let block_announce_validator = BlockAnnounceValidator::new(relay_chain_interface.clone(), id);

	let force_authoring = parachain_config.force_authoring;
	let validator = parachain_config.role.is_authority();
	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let import_queue = cumulus_client_service::SharedImportQueue::new(params.import_queue);
	let (network, system_rpc_tx, start_network) = sc_service::build_network(sc_service::BuildNetworkParams {
		config: &parachain_config,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		spawn_handle: task_manager.spawn_handle(),
		import_queue: import_queue.clone(),
		block_announce_validator_builder: Some(Box::new(|_| Box::new(block_announce_validator))),
		warp_sync: None,
	})?;

	let rpc_extensions_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: None,
			};

			Ok(acala_rpc::create_full(deps))
		})
	};

	if parachain_config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&parachain_config,
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	let relay_chain_slot_duration = Duration::from_secs(6);

	if validator {
		let parachain_consensus = build_consensus(
			client.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|t| t.handle()),
			&task_manager,
			relay_chain_interface.clone(),
			transaction_pool,
			network,
			params.keystore_container.sync_keystore(),
			force_authoring,
		)?;

		let spawner = task_manager.spawn_handle();

		let params = StartCollatorParams {
			para_id: id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			relay_chain_interface,
			spawner,
			parachain_consensus,
			import_queue,
			collator_key,
			relay_chain_slot_duration,
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id: id,
			relay_chain_interface,
			import_queue,
			relay_chain_slot_duration,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node<RuntimeApi, Executor>(
	parachain_config: Configuration,
	polkadot_config: Configuration,
	id: ParaId,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	RuntimeApi::RuntimeApi: sp_consensus_aura::AuraApi<Block, AuraId>,
	Executor: sc_executor::NativeExecutionDispatch + 'static,
{
	start_node_impl(
		parachain_config,
		polkadot_config,
		id,
		|_| Ok(Default::default()),
		|client,
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
								id,
							)
							.await;

						let time = sp_timestamp::InherentDataProvider::from_system_time();

						let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
							*time,
							slot_duration.slot_duration(),
						);

						let parachain_inherent = parachain_inherent.ok_or_else(|| {
							Box::<dyn std::error::Error + Send + Sync>::from("Failed to create parachain inherent")
						})?;
						Ok((time, slot, parachain_inherent))
					}
				},
				block_import: client.clone(),
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
	mut config: &mut Configuration,
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
	if config.chain_spec.is_mandala_dev() || config.chain_spec.is_mandala() {
		#[cfg(feature = "with-mandala-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial(config, config.chain_spec.is_mandala_dev(), false)?;
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
			} = new_partial::<karura_runtime::RuntimeApi, KaruraExecutorDispatch>(config, false, false)?;
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
			} = new_partial::<acala_runtime::RuntimeApi, AcalaExecutorDispatch>(config, false, false)?;
			Ok((Arc::new(Client::Acala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-acala-runtime"))]
		Err(ACALA_RUNTIME_NOT_AVAILABLE.into())
	}
}

#[cfg(feature = "with-mandala-runtime")]
fn inner_mandala_dev(config: Configuration, instant_sealing: bool) -> Result<TaskManager, ServiceError> {
	let sc_service::PartialComponents {
		client,
		backend,
		mut task_manager,
		import_queue,
		keystore_container,
		select_chain: maybe_select_chain,
		transaction_pool,
		other: (mut telemetry, _),
	} = new_partial::<mandala_runtime::RuntimeApi, MandalaExecutorDispatch>(&config, true, instant_sealing)?;

	let (network, system_rpc_tx, network_starter) = sc_service::build_network(sc_service::BuildNetworkParams {
		config: &config,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		spawn_handle: task_manager.spawn_handle(),
		import_queue,
		block_announce_validator_builder: None,
		warp_sync: None,
	})?;

	if config.offchain_worker.enabled {
		let offchain_workers = Arc::new(sc_offchain::OffchainWorkers::new_with_options(
			client.clone(),
			sc_offchain::OffchainWorkerOptions {
				enable_http_requests: false,
			},
		));

		// Start the offchain workers to have
		task_manager.spawn_handle().spawn(
			"offchain-notifications",
			None,
			sc_offchain::notification_future(
				config.role.is_authority(),
				client.clone(),
				offchain_workers,
				task_manager.spawn_handle(),
				network.clone(),
			),
		);
	}

	let prometheus_registry = config.prometheus_registry().cloned();

	let role = config.role.clone();
	let force_authoring = config.force_authoring;
	let backoff_authoring_blocks: Option<()> = None;

	let select_chain =
		maybe_select_chain.expect("In mandala dev mode, `new_partial` will return some `select_chain`; qed");

	let command_sink = if role.is_authority() {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
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

			let authorship_future =
				sc_consensus_manual_seal::run_manual_seal(sc_consensus_manual_seal::ManualSealParams {
					block_import: client.clone(),
					env: proposer_factory,
					client: client.clone(),
					pool: transaction_pool.clone(),
					commands_stream: futures::stream_select!(commands_stream, import_stream),
					select_chain,
					consensus_data_provider: None,
					create_inherent_data_providers: |_, _| async {
						Ok((
							sp_timestamp::InherentDataProvider::from_system_time(),
							default_mock_parachain_inherent_data_provider(),
						))
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
			let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();
			let aura = sc_consensus_aura::start_aura::<AuraPair, _, _, _, _, _, _, _, _, _, _, _>(StartAuraParams {
				slot_duration: sc_consensus_aura::slot_duration(&*client)?,
				client: client.clone(),
				select_chain,
				block_import: instant_finalize::InstantFinalizeBlockImport::new(client.clone()),
				proposer_factory,
				create_inherent_data_providers: move |_, ()| async move {
					let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

					let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(
						*timestamp,
						slot_duration,
					);

					Ok((timestamp, slot, default_mock_parachain_inherent_data_provider()))
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
				telemetry: telemetry.as_ref().map(|x| x.handle()),
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

		Box::new(move |deny_unsafe, _| {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
				command_sink: command_sink.clone(),
			};

			Ok(acala_rpc::create_full(deps))
		})
	};

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_extensions_builder,
		client,
		transaction_pool,
		task_manager: &mut task_manager,
		config,
		keystore: keystore_container.sync_keystore(),
		backend,
		network,
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	network_starter.start_network();

	Ok(task_manager)
}

#[cfg(feature = "with-mandala-runtime")]
pub fn mandala_dev(config: Configuration, instant_sealing: bool) -> Result<TaskManager, ServiceError> {
	inner_mandala_dev(config, instant_sealing)
}
