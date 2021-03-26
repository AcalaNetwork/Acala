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

// Disable the following lints
#![allow(clippy::type_complexity)]

//! Acala service. Specialized wrapper over substrate service.

use cumulus_client_consensus_relay_chain::{build_relay_chain_consensus, BuildRelayChainConsensusParams};
use cumulus_client_network::build_block_announce_validator;
use cumulus_client_service::{
	prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams,
};
use cumulus_primitives_core::ParaId;

#[cfg(feature = "with-acala-runtime")]
pub use acala_runtime;

#[cfg(feature = "with-karura-runtime")]
pub use karura_runtime;

#[cfg(feature = "with-mandala-runtime")]
pub use mandala_runtime;

use acala_primitives::Block;
use polkadot_primitives::v0::CollatorPair;
use sc_executor::native_executor_instance;
use sc_service::{
	error::Error as ServiceError, Configuration, PartialComponents, Role, TFullBackend, TFullClient, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryWorker, TelemetryWorkerHandle};
use sp_runtime::traits::BlakeTwo256;
use sp_trie::PrefixedMemoryDB;
use std::sync::Arc;

pub use client::*;

pub use sc_executor::NativeExecutionDispatch;
pub use sc_service::{
	config::{DatabaseConfig, PrometheusConfig},
	ChainSpec,
};
pub use sp_api::ConstructRuntimeApi;

pub mod chain_spec;
mod client;
mod mock_timestamp_data_provider;

#[cfg(feature = "with-mandala-runtime")]
native_executor_instance!(
	pub MandalaExecutor,
	mandala_runtime::api::dispatch,
	mandala_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);

#[cfg(feature = "with-karura-runtime")]
native_executor_instance!(
	pub KaruraExecutor,
	karura_runtime::api::dispatch,
	karura_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);

#[cfg(feature = "with-acala-runtime")]
native_executor_instance!(
	pub AcalaExecutor,
	acala_runtime::api::dispatch,
	acala_runtime::native_version,
	frame_benchmarking::benchmarking::HostFunctions,
);

/// Can be called for a `Configuration` to check if it is a configuration for
/// the `Acala` network.
pub trait IdentifyVariant {
	/// Returns if this is a configuration for the `Acala` network.
	fn is_acala(&self) -> bool;

	/// Returns if this is a configuration for the `Karura` network.
	fn is_karura(&self) -> bool;

	/// Returns if this is a configuration for the `Mandala` network.
	fn is_mandala(&self) -> bool;
}

impl IdentifyVariant for Box<dyn ChainSpec> {
	fn is_acala(&self) -> bool {
		self.id().starts_with("acala") || self.id().starts_with("aca")
	}

	fn is_karura(&self) -> bool {
		self.id().starts_with("karura") || self.id().starts_with("kar")
	}

	fn is_mandala(&self) -> bool {
		self.id().starts_with("mandala") || self.id().starts_with("man")
	}
}

/// Acala's full backend.
type FullBackend = TFullBackend<Block>;

/// Acala's full client.
type FullClient<RuntimeApi, Executor> = TFullClient<Block, RuntimeApi, Executor>;

pub fn new_partial<RuntimeApi, Executor>(
	config: &Configuration,
	_test: bool,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi, Executor>,
		FullBackend,
		(),
		sp_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
		(Option<Telemetry>, Option<TelemetryWorkerHandle>),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

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

	let (client, backend, keystore_container, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, Executor>(
		&config,
		telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
	)?;
	let client = Arc::new(client);

	let telemetry_worker_handle = telemetry.as_ref().map(|(worker, _)| worker.handle());

	let telemetry = telemetry.map(|(worker, telemetry)| {
		task_manager.spawn_handle().spawn("telemetry", worker.run());
		telemetry
	});

	let registry = config.prometheus_registry();

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.role.is_authority().into(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let import_queue = cumulus_client_consensus_relay_chain::import_queue(
		client.clone(),
		client.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_essential_handle(),
		registry,
	)?;

	Ok(PartialComponents {
		backend,
		client,
		import_queue,
		keystore_container,
		task_manager,
		transaction_pool,
		inherent_data_providers,
		select_chain: (),
		other: (telemetry, telemetry_worker_handle),
	})
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_tracing::logging::prefix_logs_with("Parachain")]
async fn start_node_impl<RB, RuntimeApi, Executor>(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: ParaId,
	validator: bool,
	_rpc_ext_builder: RB,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
	RB: Fn(Arc<FullClient<RuntimeApi, Executor>>) -> jsonrpc_core::IoHandler<sc_rpc::Metadata> + Send + 'static,
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config, false)?;
	params
		.inherent_data_providers
		.register_provider(sp_timestamp::InherentDataProvider)
		.unwrap();
	let (mut telemetry, telemetry_worker_handle) = params.other;

	let polkadot_full_node = cumulus_client_service::build_polkadot_full_node(
		polkadot_config,
		collator_key.clone(),
		telemetry_worker_handle,
	)
	.map_err(|e| match e {
		polkadot_service::Error::Sub(x) => x,
		s => format!("{}", s).into(),
	})?;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let block_announce_validator = build_block_announce_validator(
		polkadot_full_node.client.clone(),
		id,
		Box::new(polkadot_full_node.network.clone()),
		polkadot_full_node.backend.clone(),
	);

	let prometheus_registry = parachain_config.prometheus_registry().cloned();
	let transaction_pool = params.transaction_pool.clone();
	let mut task_manager = params.task_manager;
	let import_queue = params.import_queue;
	let (network, network_status_sinks, system_rpc_tx, start_network) =
		sc_service::build_network(sc_service::BuildNetworkParams {
			config: &parachain_config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			on_demand: None,
			block_announce_validator_builder: Some(Box::new(|_| block_announce_validator)),
		})?;

	let rpc_extensions_builder = {
		let client = client.clone();
		let transaction_pool = transaction_pool.clone();

		Box::new(move |deny_unsafe, _| -> acala_rpc::RpcExtension {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				deny_unsafe,
			};

			acala_rpc::create_full(deps)
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
		on_demand: None,
		remote_blockchain: None,
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		network_status_sinks,
		system_rpc_tx,
		telemetry: telemetry.as_mut(),
	})?;

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	if validator {
		let proposer_factory = sc_basic_authorship::ProposerFactory::with_proof_recording(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool,
			prometheus_registry.as_ref(),
			telemetry.as_ref().map(|x| x.handle()),
		);
		let spawner = task_manager.spawn_handle();

		let parachain_consensus = build_relay_chain_consensus(BuildRelayChainConsensusParams {
			para_id: id,
			proposer_factory,
			inherent_data_providers: params.inherent_data_providers,
			block_import: client.clone(),
			relay_chain_client: polkadot_full_node.client.clone(),
			relay_chain_backend: polkadot_full_node.backend.clone(),
		});

		let params = StartCollatorParams {
			para_id: id,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			collator_key,
			relay_chain_full_node: polkadot_full_node,
			spawner,
			backend,
			parachain_consensus,
		};

		start_collator(params).await?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			task_manager: &mut task_manager,
			para_id: id,
			polkadot_full_node,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}

/// Start a normal parachain node.
pub async fn start_node<RuntimeApi, Executor>(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: ParaId,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient<RuntimeApi, Executor>>)>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	start_node_impl(parachain_config, collator_key, polkadot_config, id, validator, |_| {
		Default::default()
	})
	.await
}

/// Builds a new object suitable for chain operations.
pub fn new_chain_ops(
	mut config: &mut Configuration,
) -> Result<
	(
		Arc<Client>,
		Arc<FullBackend>,
		sp_consensus::import_queue::BasicQueue<Block, PrefixedMemoryDB<BlakeTwo256>>,
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
			} = new_partial(config, false)?;
			Ok((Arc::new(Client::Mandala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-mandala-runtime"))]
		Err("Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.".into())
	} else if config.chain_spec.is_karura() {
		#[cfg(feature = "with-karura-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial::<karura_runtime::RuntimeApi, KaruraExecutor>(config, false)?;
			Ok((Arc::new(Client::Karura(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-karura-runtime"))]
		Err("Karura runtime is not available. Please compile the node with `--features with-karura-runtime` to enable it.".into())
	} else {
		#[cfg(feature = "with-acala-runtime")]
		{
			let PartialComponents {
				client,
				backend,
				import_queue,
				task_manager,
				..
			} = new_partial::<acala_runtime::RuntimeApi, AcalaExecutor>(config, false)?;
			Ok((Arc::new(Client::Acala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-acala-runtime"))]
		Err("Acala runtime is not available. Please compile the node with `--features with-acala-runtime` to enable it.".into())
	}
}
