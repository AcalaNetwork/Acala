// Disable the following lints
#![allow(clippy::type_complexity)]

//! Acala service. Specialized wrapper over substrate service.

use std::sync::Arc;

use acala_primitives::Block;
use prometheus_endpoint::Registry;
use sc_client_api::{ExecutorProvider, RemoteBackend};
use sc_executor::native_executor_instance;
use sc_finality_grandpa::FinalityProofProvider as GrandpaFinalityProofProvider;
use sc_service::{config::Configuration, error::Error as ServiceError, PartialComponents, RpcHandlers, TaskManager};
use sp_inherents::InherentDataProviders;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

pub use client::*;

#[cfg(feature = "with-acala-runtime")]
pub use acala_runtime;
#[cfg(feature = "with-karurua-runtime")]
pub use karura_runtime;
#[cfg(feature = "with-mandala-runtime")]
pub use mandala_runtime;

pub use sc_executor::NativeExecutionDispatch;
pub use sc_service::{
	config::{DatabaseConfig, PrometheusConfig},
	ChainSpec,
};
pub use sp_api::ConstructRuntimeApi;

use cumulus_network::build_block_announce_validator;
use cumulus_service::{prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams};

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
type FullBackend = sc_service::TFullBackend<Block>;

/// Acala's select chain.
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;

/// Acala's full client.
type FullClient<RuntimeApi, Executor> = sc_service::TFullClient<Block, RuntimeApi, Executor>;

/// Acala's full Grandpa block import.
type FullGrandpaBlockImport<RuntimeApi, Executor> =
	sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient<RuntimeApi, Executor>, FullSelectChain>;

pub fn new_partial<RuntimeApi, Executor>(
	config: &mut Configuration,
	test: bool,
) -> Result<
	PartialComponents<
		FullClient<RuntimeApi, Executor>,
		FullBackend,
		FullSelectChain,
		sp_consensus::DefaultImportQueue<Block, FullClient<RuntimeApi, Executor>>,
		sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
		(
			impl Fn(acala_rpc::DenyUnsafe, acala_rpc::SubscriptionTaskExecutor) -> acala_rpc::RpcExtension,
			(
				sc_consensus_babe::BabeBlockImport<
					Block,
					FullClient<RuntimeApi, Executor>,
					FullGrandpaBlockImport<RuntimeApi, Executor>,
				>,
				sc_finality_grandpa::LinkHalf<Block, FullClient<RuntimeApi, Executor>, FullSelectChain>,
				sc_consensus_babe::BabeLink<Block>,
			),
			sc_finality_grandpa::SharedVoterState,
		),
	>,
	sc_service::Error,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
{
	if !test {
		// If we're using prometheus, use a registry with a prefix of `acala`.
		if let Some(PrometheusConfig { registry, .. }) = config.prometheus_config.as_mut() {
			*registry = Registry::new_custom(Some("acala".into()), None)?;
		}
	}

	let (client, backend, keystore_container, task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let (grandpa_block_import, grandpa_link) =
		sc_finality_grandpa::block_import(client.clone(), &(client.clone() as Arc<_>), select_chain.clone())?;
	let justification_import = grandpa_block_import.clone();

	let (block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::Config::get_or_compute(&*client)?,
		grandpa_block_import,
		client.clone(),
	)?;

	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	let import_queue = cumulus_consensus::import_queue::import_queue(
		client.clone(),
		client.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		registry.clone(),
	)?;

	let justification_stream = grandpa_link.justification_stream();
	let shared_authority_set = grandpa_link.shared_authority_set().clone();
	let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();

	let import_setup = (block_import, grandpa_link, babe_link.clone());
	let rpc_setup = shared_voter_state.clone();

	let finality_proof_provider = GrandpaFinalityProofProvider::new_for_service(backend.clone(), client.clone());

	let babe_config = babe_link.config().clone();
	let shared_epoch_changes = babe_link.epoch_changes().clone();

	let rpc_extensions_builder = {
		let client = client.clone();
		let keystore = keystore_container.sync_keystore();
		let transaction_pool = transaction_pool.clone();
		let select_chain = select_chain.clone();

		move |deny_unsafe, subscription_executor| -> acala_rpc::RpcExtension {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: transaction_pool.clone(),
				select_chain: select_chain.clone(),
				deny_unsafe,
				babe: acala_rpc::BabeDeps {
					babe_config: babe_config.clone(),
					shared_epoch_changes: shared_epoch_changes.clone(),
					keystore: keystore.clone(),
				},
				grandpa: acala_rpc::GrandpaDeps {
					shared_voter_state: shared_voter_state.clone(),
					shared_authority_set: shared_authority_set.clone(),
					justification_stream: justification_stream.clone(),
					subscription_executor,
					finality_provider: finality_proof_provider.clone(),
				},
			};

			acala_rpc::create_full(deps)
		}
	};

	Ok(PartialComponents {
		client,
		backend,
		task_manager,
		keystore_container,
		select_chain,
		import_queue,
		transaction_pool,
		inherent_data_providers,
		other: (rpc_extensions_builder, import_setup, rpc_setup),
	})
}

/// Start a node with the given parachain `Configuration` and relay chain
/// `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the
/// runtime api.
#[sc_cli::prefix_logs_with("Parachain")]
async fn start_node_impl(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: polkadot_primitives::v0::Id,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)> {
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let polkadot_full_node = cumulus_service::build_polkadot_full_node(polkadot_config, collator_key.public())?;

	let params = new_partial(&parachain_config)?;
	params
		.inherent_data_providers
		.register_provider(sp_timestamp::InherentDataProvider)
		.unwrap();

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

	if config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&config,
			backend.clone(),
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	};

	let rpc_extensions_builder = Box::new(params.other.0);

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		on_demand: None,
		remote_blockchain: None,
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		telemetry_connection_sinks: Default::default(),
		config: parachain_config,
		keystore: params.keystore_container.sync_keystore(),
		backend: backend.clone(),
		network: network.clone(),
		network_status_sinks,
		system_rpc_tx,
	})?;

	let announce_block = {
		let network = network.clone();
		Arc::new(move |hash, data| network.announce_block(hash, data))
	};

	if validator {
		let proposer_factory = sc_basic_authorship::ProposerFactory::new(
			task_manager.spawn_handle(),
			client.clone(),
			transaction_pool,
			prometheus_registry.as_ref(),
		);
		let spawner = task_manager.spawn_handle();

		let params = StartCollatorParams {
			para_id: id,
			block_import: client.clone(),
			proposer_factory,
			inherent_data_providers: params.inherent_data_providers,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			task_manager: &mut task_manager,
			collator_key,
			polkadot_full_node,
			spawner,
			backend,
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
pub async fn start_node(
	parachain_config: Configuration,
	collator_key: CollatorPair,
	polkadot_config: Configuration,
	id: polkadot_primitives::v0::Id,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<TFullClient<Block, RuntimeApi, Executor>>)> {
	start_node_impl(parachain_config, collator_key, polkadot_config, id, validator).await
}

/// Builds a new object suitable for chain operations.
pub fn new_chain_ops(
	mut config: &mut Configuration,
) -> Result<
	(
		Arc<Client>,
		Arc<FullBackend>,
		sp_consensus::import_queue::BasicQueue<Block, sp_trie::PrefixedMemoryDB<BlakeTwo256>>,
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
			} = new_partial::<mandala_runtime::RuntimeApi, MandalaExecutor>(config, false)?;
			Ok((Arc::new(Client::Mandala(client)), backend, import_queue, task_manager))
		}
		#[cfg(not(feature = "with-mandala-runtime"))]
		Err("Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.".into())
	} else if config.chain_spec.is_karura() {
		#[cfg(feature = "with-kaura-runtime")]
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
		#[cfg(not(feature = "with-kaura-runtime"))]
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

pub fn build_full(
	config: Configuration,
	instant_sealing: bool,
	test: bool,
) -> Result<(Arc<Client>, sc_service::NetworkStatusSinks<Block>, TaskManager), ServiceError> {
	if config.chain_spec.is_acala() {
		#[cfg(feature = "with-acala-runtime")]
		{
			let (task_manager, _, client, _, _, network_status_sinks) =
				new_full::<acala_runtime::RuntimeApi, AcalaExecutor>(config, instant_sealing, test)?;
			Ok((Arc::new(Client::Acala(client)), network_status_sinks, task_manager))
		}
		#[cfg(not(feature = "with-acala-runtime"))]
		Err("Acala runtime is not available. Please compile the node with `--features with-acala-runtime` to enable it.".into())
	} else if config.chain_spec.is_karura() {
		#[cfg(feature = "with-kaura-runtime")]
		{
			let (task_manager, _, client, _, _, network_status_sinks) =
				new_full::<karura_runtime::RuntimeApi, KaruraExecutor>(config, instant_sealing, test)?;
			Ok((Arc::new(Client::Karura(client)), network_status_sinks, task_manager))
		}
		#[cfg(not(feature = "with-kaura-runtime"))]
		Err("Karura runtime is not available. Please compile the node with `--features with-karura-runtime` to enable it.".into())
	} else {
		#[cfg(feature = "with-mandala-runtime")]
		{
			let (task_manager, _, client, _, _, network_status_sinks) =
				new_full::<mandala_runtime::RuntimeApi, MandalaExecutor>(config, instant_sealing, test)?;
			Ok((Arc::new(Client::Mandala(client)), network_status_sinks, task_manager))
		}
		#[cfg(not(feature = "with-mandala-runtime"))]
		Err("Mandala runtime is not available. Please compile the node with `--features with-mandala-runtime` to enable it.".into())
	}
}
