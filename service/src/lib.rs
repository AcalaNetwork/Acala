//! Acala service. Specialized wrapper over substrate service.

use std::sync::Arc;

use acala_primitives::{AccountId, Balance, Block, CurrencyId, Nonce};
use sc_client_api::{ExecutorProvider, RemoteBackend};
use sc_executor::native_executor_instance;
use sc_finality_grandpa::FinalityProofProvider as GrandpaFinalityProofProvider;
use sc_service::{config::Configuration, error::Error as ServiceError, RpcHandlers, ServiceComponents, TaskManager};
use sp_core::traits::BareCryptoStorePtr;
use sp_inherents::InherentDataProviders;
use sp_runtime::traits::{BlakeTwo256, Block as BlockT};

pub use dev_runtime;
pub use sc_executor::NativeExecutionDispatch;
pub use sc_service::ChainSpec;
pub use sp_api::ConstructRuntimeApi;

pub mod chain_spec;

native_executor_instance!(
	pub DevExecutor,
	dev_runtime::api::dispatch,
	dev_runtime::native_version,
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

type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullClient<RuntimeApi, Executor> = sc_service::TFullClient<Block, RuntimeApi, Executor>;
type FullGrandpaBlockImport<RuntimeApi, Executor> =
	sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient<RuntimeApi, Executor>, FullSelectChain>;
type LightBackend = sc_service::TLightBackendWithHash<Block, BlakeTwo256>;
type LightClient<RuntimeApi, Executor> = sc_service::TLightClientWithBackend<Block, RuntimeApi, Executor, LightBackend>;

/// A set of APIs that polkadot-like runtimes must implement.
pub trait RuntimeApiCollection<UncheckedExtrinsic>:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block, Error = sp_blockchain::Error>
	+ sp_consensus_babe::BabeApi<Block>
	+ sp_finality_grandpa::GrandpaApi<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance, UncheckedExtrinsic>
	+ orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, dev_runtime::TimeStampedPrice>
	+ module_staking_pool_rpc::StakingPoolRuntimeApi<Block, AccountId, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
	UncheckedExtrinsic: codec::Codec,
{
}

impl<Api, UncheckedExtrinsic> RuntimeApiCollection<UncheckedExtrinsic> for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block, Error = sp_blockchain::Error>
		+ sp_consensus_babe::BabeApi<Block>
		+ sp_finality_grandpa::GrandpaApi<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance, UncheckedExtrinsic>
		+ orml_oracle_rpc::OracleRuntimeApi<Block, CurrencyId, dev_runtime::TimeStampedPrice>
		+ module_staking_pool_rpc::StakingPoolRuntimeApi<Block, AccountId, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>,
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
	UncheckedExtrinsic: codec::Codec,
{
}

#[allow(clippy::type_complexity)]
pub fn new_full_params<RuntimeApi, Executor, UncheckedExtrinsic>(
	config: Configuration,
) -> Result<
	(
		sc_service::ServiceParams<
			Block,
			FullClient<RuntimeApi, Executor>,
			sc_consensus_babe::BabeImportQueue<Block, FullClient<RuntimeApi, Executor>>,
			sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>,
			acala_rpc::RpcExtension,
			FullBackend,
		>,
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
		FullSelectChain,
		InherentDataProviders,
	),
	ServiceError,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi:
		RuntimeApiCollection<UncheckedExtrinsic, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
	UncheckedExtrinsic: Send + Sync + 'static + codec::Codec,
{
	let (client, backend, keystore, task_manager) = sc_service::new_full_parts::<Block, RuntimeApi, Executor>(&config)?;
	let client = Arc::new(client);

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let pool_api = sc_transaction_pool::FullChainApi::new(client.clone(), config.prometheus_registry());
	let transaction_pool = sc_transaction_pool::BasicPool::new_full(
		config.transaction_pool.clone(),
		std::sync::Arc::new(pool_api),
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

	let import_queue = sc_consensus_babe::import_queue(
		babe_link.clone(),
		block_import.clone(),
		Some(Box::new(justification_import)),
		None,
		client.clone(),
		select_chain.clone(),
		inherent_data_providers.clone(),
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
	)?;

	let import_setup = (block_import, grandpa_link, babe_link);

	let (rpc_extensions_builder, rpc_setup) = {
		let (_, grandpa_link, babe_link) = &import_setup;

		let shared_authority_set = grandpa_link.shared_authority_set().clone();
		let shared_voter_state = sc_finality_grandpa::SharedVoterState::empty();

		let rpc_setup = shared_voter_state.clone();

		let babe_config = babe_link.config().clone();
		let shared_epoch_changes = babe_link.epoch_changes().clone();

		let client = client.clone();
		let pool = transaction_pool.clone();
		let select_chain = select_chain.clone();
		let keystore = keystore.clone();

		let rpc_extensions_builder = Box::new(move |deny_unsafe| {
			let deps = acala_rpc::FullDeps {
				client: client.clone(),
				pool: pool.clone(),
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
				},
			};

			acala_rpc::create_full::<_, _, _, UncheckedExtrinsic>(deps)
		});

		(rpc_extensions_builder, rpc_setup)
	};

	let provider = client.clone() as Arc<dyn sc_finality_grandpa::StorageAndProofProvider<_, _>>;
	let finality_proof_provider = Arc::new(sc_finality_grandpa::FinalityProofProvider::new(
		backend.clone(),
		provider,
	));

	let params = sc_service::ServiceParams {
		config,
		backend,
		client,
		import_queue,
		keystore,
		task_manager,
		rpc_extensions_builder,
		transaction_pool,
		block_announce_validator_builder: None,
		finality_proof_request_builder: None,
		finality_proof_provider: Some(finality_proof_provider),
		on_demand: None,
		remote_blockchain: None,
	};

	Ok((params, import_setup, rpc_setup, select_chain, inherent_data_providers))
}

/// Creates a full service from the configuration.
#[allow(clippy::type_complexity)]
pub fn new_full_base<
	RuntimeApi,
	Executor,
	UncheckedExtrinsic,
	T: FnOnce(
		&sc_consensus_babe::BabeBlockImport<
			Block,
			FullClient<RuntimeApi, Executor>,
			FullGrandpaBlockImport<RuntimeApi, Executor>,
		>,
		&sc_consensus_babe::BabeLink<Block>,
	),
>(
	config: Configuration,
	with_startup_data: T,
) -> Result<
	(
		TaskManager,
		InherentDataProviders,
		Arc<FullClient<RuntimeApi, Executor>>,
		Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
		Arc<sc_transaction_pool::FullPool<Block, FullClient<RuntimeApi, Executor>>>,
	),
	ServiceError,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi:
		RuntimeApiCollection<UncheckedExtrinsic, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
	UncheckedExtrinsic: Send + Sync + 'static + codec::Codec,
{
	let (params, import_setup, rpc_setup, select_chain, inherent_data_providers) =
		new_full_params::<RuntimeApi, Executor, UncheckedExtrinsic>(config)?;

	let (role, force_authoring, name, enable_grandpa, prometheus_registry, client, transaction_pool, keystore) = {
		let sc_service::ServiceParams {
			config,
			client,
			transaction_pool,
			keystore,
			..
		} = &params;

		(
			config.role.clone(),
			config.force_authoring,
			config.network.node_name.clone(),
			!config.disable_grandpa,
			config.prometheus_registry().cloned(),
			client.clone(),
			transaction_pool.clone(),
			keystore.clone(),
		)
	};

	let ServiceComponents {
		task_manager,
		network,
		telemetry_on_connect_sinks,
		..
	} = sc_service::build(params)?;

	let (block_import, grandpa_link, babe_link) = import_setup;
	let shared_voter_state = rpc_setup;

	(with_startup_data)(&block_import, &babe_link);

	if let sc_service::config::Role::Authority { .. } = &role {
		let proposer = sc_basic_authorship::ProposerFactory::new(
			client.clone(),
			transaction_pool.clone(),
			prometheus_registry.as_ref(),
		);

		let can_author_with = sp_consensus::CanAuthorWithNativeVersion::new(client.executor().clone());

		let babe_config = sc_consensus_babe::BabeParams {
			keystore: keystore.clone(),
			client: client.clone(),
			select_chain,
			env: proposer,
			block_import,
			sync_oracle: network.clone(),
			inherent_data_providers: inherent_data_providers.clone(),
			force_authoring,
			babe_link,
			can_author_with,
		};

		let babe = sc_consensus_babe::start_babe(babe_config)?;
		task_manager
			.spawn_essential_handle()
			.spawn_blocking("babe-proposer", babe);
	}

	// // Spawn authority discovery module.
	// if matches!(role, Role::Authority{..} | Role::Sentry {..}) {
	// 	let (sentries, authority_discovery_role) = match role {
	// 		sc_service::config::Role::Authority { ref sentry_nodes } => (
	// 			sentry_nodes.clone(),
	// 			sc_authority_discovery::Role::Authority (
	// 				keystore.clone(),
	// 			),
	// 		),
	// 		sc_service::config::Role::Sentry {..} => (
	// 			vec![],
	// 			sc_authority_discovery::Role::Sentry,
	// 		),
	// 		_ => unreachable!("Due to outer matches! constraint; qed.")
	// 	};

	// 	let dht_event_stream = network.event_stream("authority-discovery")
	// 		.filter_map(|e| async move { match e {
	// 			Event::Dht(e) => Some(e),
	// 			_ => None,
	// 		}}).boxed();
	// 	let authority_discovery = sc_authority_discovery::AuthorityDiscovery::new(
	// 		client.clone(),
	// 		network.clone(),
	// 		sentries,
	// 		dht_event_stream,
	// 		authority_discovery_role,
	// 		prometheus_registry.clone(),
	// 	);

	// 	task_manager.spawn_handle().spawn("authority-discovery",
	// authority_discovery); }

	// if the node isn't actively participating in consensus then it doesn't
	// need a keystore, regardless of which protocol we use below.
	let keystore = if role.is_authority() {
		Some(keystore as BareCryptoStorePtr)
	} else {
		None
	};

	let config = sc_finality_grandpa::Config {
		// FIXME #1578 make this available through chainspec
		gossip_duration: std::time::Duration::from_millis(333),
		justification_period: 512,
		name: Some(name),
		observer_enabled: false,
		keystore,
		is_authority: role.is_network_authority(),
	};

	if enable_grandpa {
		// start the full GRANDPA voter
		// NOTE: non-authorities could run the GRANDPA observer protocol, but at
		// this point the full voter should provide better guarantees of block
		// and vote data availability than the observer. The observer has not
		// been tested extensively yet and having most nodes in a network run it
		// could lead to finality stalls.
		let grandpa_config = sc_finality_grandpa::GrandpaParams {
			config,
			link: grandpa_link,
			network: network.clone(),
			inherent_data_providers: inherent_data_providers.clone(),
			telemetry_on_connect: Some(telemetry_on_connect_sinks.on_connect_stream()),
			voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
			prometheus_registry,
			shared_voter_state,
		};

		// the GRANDPA voter task is considered infallible, i.e.
		// if it fails we take down the service with it.
		task_manager
			.spawn_essential_handle()
			.spawn_blocking("grandpa-voter", sc_finality_grandpa::run_grandpa_voter(grandpa_config)?);
	} else {
		sc_finality_grandpa::setup_disabled_grandpa(client.clone(), &inherent_data_providers, network.clone())?;
	}

	Ok((task_manager, inherent_data_providers, client, network, transaction_pool))
}

/// Builds a new service for a full client.
pub fn new_full<RuntimeApi, Executor, UncheckedExtrinsic>(config: Configuration) -> Result<TaskManager, ServiceError>
where
	RuntimeApi: ConstructRuntimeApi<Block, FullClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	RuntimeApi::RuntimeApi:
		RuntimeApiCollection<UncheckedExtrinsic, StateBackend = sc_client_api::StateBackendFor<FullBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
	UncheckedExtrinsic: Send + Sync + 'static + codec::Codec,
{
	new_full_base::<RuntimeApi, Executor, UncheckedExtrinsic, _>(config, |_, _| ())
		.map(|(task_manager, _, _, _, _)| task_manager)
}

/// Creates a light service from the configuration.
#[allow(clippy::type_complexity)]
pub fn new_light_base<RuntimeApi, Executor, UncheckedExtrinsic>(
	config: Configuration,
) -> Result<
	(
		TaskManager,
		Arc<RpcHandlers>,
		Arc<LightClient<RuntimeApi, Executor>>,
		Arc<sc_network::NetworkService<Block, <Block as BlockT>::Hash>>,
		Arc<
			sc_transaction_pool::LightPool<
				Block,
				LightClient<RuntimeApi, Executor>,
				sc_network::config::OnDemand<Block>,
			>,
		>,
	),
	ServiceError,
>
where
	RuntimeApi: ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	<RuntimeApi as ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>>>::RuntimeApi:
		RuntimeApiCollection<UncheckedExtrinsic, StateBackend = sc_client_api::StateBackendFor<LightBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
	UncheckedExtrinsic: Send + Sync + 'static + codec::Codec,
{
	let (client, backend, keystore, task_manager, on_demand) =
		sc_service::new_light_parts::<Block, RuntimeApi, Executor>(&config)?;

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let transaction_pool_api = Arc::new(sc_transaction_pool::LightChainApi::new(
		client.clone(),
		on_demand.clone(),
	));
	let transaction_pool = Arc::new(sc_transaction_pool::BasicPool::new_light(
		config.transaction_pool.clone(),
		transaction_pool_api,
		config.prometheus_registry(),
		task_manager.spawn_handle(),
	));

	let grandpa_block_import = sc_finality_grandpa::light_block_import(
		client.clone(),
		backend.clone(),
		&(client.clone() as Arc<_>),
		Arc::new(on_demand.checker().clone()),
	)?;

	let finality_proof_import = grandpa_block_import.clone();
	let finality_proof_request_builder = finality_proof_import.create_finality_proof_request_builder();

	let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
		sc_consensus_babe::Config::get_or_compute(&*client)?,
		grandpa_block_import,
		client.clone(),
	)?;

	let inherent_data_providers = sp_inherents::InherentDataProviders::new();

	let import_queue = sc_consensus_babe::import_queue(
		babe_link,
		babe_block_import,
		None,
		Some(Box::new(finality_proof_import)),
		client.clone(),
		select_chain,
		inherent_data_providers,
		&task_manager.spawn_handle(),
		config.prometheus_registry(),
	)?;

	// GenesisAuthoritySetProvider is implemented for StorageAndProofProvider
	let provider = client.clone() as Arc<dyn sc_finality_grandpa::StorageAndProofProvider<_, _>>;
	let finality_proof_provider = Arc::new(GrandpaFinalityProofProvider::new(backend.clone(), provider));

	let light_deps = acala_rpc::LightDeps {
		remote_blockchain: backend.remote_blockchain(),
		fetcher: on_demand.clone(),
		client: client.clone(),
		pool: transaction_pool.clone(),
	};

	let rpc_extensions = acala_rpc::create_light(light_deps);

	let ServiceComponents {
		task_manager,
		rpc_handlers,
		network,
		..
	} = sc_service::build(sc_service::ServiceParams {
		block_announce_validator_builder: None,
		finality_proof_request_builder: Some(finality_proof_request_builder),
		finality_proof_provider: Some(finality_proof_provider),
		on_demand: Some(on_demand),
		remote_blockchain: Some(backend.remote_blockchain()),
		rpc_extensions_builder: Box::new(sc_service::NoopRpcExtensionBuilder(rpc_extensions)),
		client: client.clone(),
		transaction_pool: transaction_pool.clone(),
		config,
		import_queue,
		keystore,
		backend,
		task_manager,
	})?;

	Ok((task_manager, rpc_handlers, client, network, transaction_pool))
}

/// Builds a new service for a light client.
pub fn new_light<RuntimeApi, Executor, UncheckedExtrinsic>(config: Configuration) -> Result<TaskManager, ServiceError>
where
	RuntimeApi: ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>> + Send + Sync + 'static,
	<RuntimeApi as ConstructRuntimeApi<Block, LightClient<RuntimeApi, Executor>>>::RuntimeApi:
		RuntimeApiCollection<UncheckedExtrinsic, StateBackend = sc_client_api::StateBackendFor<LightBackend, Block>>,
	Executor: NativeExecutionDispatch + 'static,
	UncheckedExtrinsic: Send + Sync + 'static + codec::Codec,
{
	new_light_base::<RuntimeApi, Executor, UncheckedExtrinsic>(config).map(|(task_manager, _, _, _, _)| task_manager)
}
