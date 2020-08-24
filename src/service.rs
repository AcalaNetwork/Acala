#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.

use crate::executor::Executor;
use ansi_term::Color;
use cumulus_network::DelayedBlockAnnounceValidator;
use cumulus_service::{prepare_node_config, start_collator, start_full_node, StartCollatorParams, StartFullNodeParams};
use polkadot_primitives::v0::CollatorPair;
use runtime::{opaque::Block, RuntimeApi};
use sc_informant::OutputFormat;
use sc_service::{config::Configuration, error::Error as ServiceError, Role, TaskManager};
use std::sync::Arc;

type FullClient = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullBackend = sc_service::TFullBackend<Block>;

pub fn new_partial(
	config: &Configuration,
) -> Result<
	sc_service::PartialComponents<
		FullClient,
		FullBackend,
		(),
		sp_consensus::DefaultImportQueue<Block, FullClient>,
		sc_transaction_pool::FullPool<Block, FullClient>,
		(),
	>,
	ServiceError,
> {
	let (client, backend, keystore, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		config.prometheus_registry(),
		task_manager.spawn_handle(),
		client.clone(),
	);

	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	let registry = config.prometheus_registry();

	let import_queue = cumulus_consensus::import_queue::import_queue(
		client.clone(),
		client.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		registry.clone(),
	)?;

	Ok(sc_service::PartialComponents {
		client,
		backend,
		task_manager,
		keystore,
		select_chain: (),
		import_queue,
		transaction_pool,
		inherent_data_providers,
		other: (),
	})
}

/// Run a node with the given parachain `Configuration` and relay chain
/// `Configuration`
///
/// This function blocks until done.
pub fn run_node(
	parachain_config: Configuration,
	collator_key: Arc<CollatorPair>,
	mut polkadot_config: polkadot_collator::Configuration,
	id: polkadot_primitives::v0::Id,
	validator: bool,
) -> sc_service::error::Result<(TaskManager, Arc<FullClient>)> {
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let mut parachain_config = prepare_node_config(parachain_config);

	parachain_config.informant_output_format = OutputFormat {
		enable_color: true,
		prefix: format!("[{}] ", Color::Yellow.bold().paint("Parachain")),
	};
	polkadot_config.informant_output_format = OutputFormat {
		enable_color: true,
		prefix: format!("[{}] ", Color::Blue.bold().paint("Relaychain")),
	};

	let params = new_partial(&mut parachain_config)?;
	params
		.inherent_data_providers
		.register_provider(sp_timestamp::InherentDataProvider)
		.unwrap();

	let client = params.client.clone();
	let backend = params.backend.clone();
	let block_announce_validator = DelayedBlockAnnounceValidator::new();
	let block_announce_validator_builder = {
		let block_announce_validator = block_announce_validator.clone();
		move |_| Box::new(block_announce_validator) as Box<_>
	};

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
			block_announce_validator_builder: Some(Box::new(block_announce_validator_builder)),
			finality_proof_request_builder: None,
			finality_proof_provider: None,
		})?;

	let rpc_extensions_builder = {
		let client = client.clone();
		let pool = transaction_pool.clone();

		let rpc_extensions_builder = move |deny_unsafe| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
				deny_unsafe,
			};

			crate::rpc::create_full(deps)
		};

		Box::new(rpc_extensions_builder)
	};

	if parachain_config.offchain_worker.enabled {
		sc_service::build_offchain_workers(
			&parachain_config,
			backend.clone(),
			task_manager.spawn_handle(),
			client.clone(),
			network.clone(),
		);
	}

	sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		on_demand: None,
		remote_blockchain: None,
		rpc_extensions_builder,
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		task_manager: &mut task_manager,
		telemetry_connection_sinks: Default::default(),
		config: parachain_config,
		keystore: params.keystore,
		backend: backend,
		network: network.clone(),
		network_status_sinks,
		system_rpc_tx,
	})?;

	let announce_block = Arc::new(move |hash, data| network.announce_block(hash, data));

	if validator {
		let proposer_factory =
			sc_basic_authorship::ProposerFactory::new(client.clone(), transaction_pool, prometheus_registry.as_ref());

		let params = StartCollatorParams {
			para_id: id,
			block_import: client.clone(),
			proposer_factory,
			inherent_data_providers: params.inherent_data_providers,
			block_status: client.clone(),
			announce_block,
			client: client.clone(),
			block_announce_validator,
			task_manager: &mut task_manager,
			polkadot_config,
			collator_key,
		};

		start_collator(params)?;
	} else {
		let params = StartFullNodeParams {
			client: client.clone(),
			announce_block,
			polkadot_config,
			collator_key,
			block_announce_validator,
			task_manager: &mut task_manager,
			para_id: id,
		};

		start_full_node(params)?;
	}

	start_network.start_network();

	Ok((task_manager, client))
}
