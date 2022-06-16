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

#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use super::*;
use cumulus_primitives_parachain_inherent::{MockValidationDataInherentDataProvider, MockXcmConfig};

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
		sc_service::new_full_parts::<Block, RuntimeApi, _>(config, None, executor)?;
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
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
			let client_for_cidp = client.clone();

			(
				sc_consensus_aura::import_queue::<sp_consensus_aura::sr25519::AuthorityPair, _, _, _, _, _, _>(
					ImportQueueParams {
						block_import: client.clone(),
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

								let slot =
									sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
										*timestamp,
										slot_duration,
									);

								let mocked_parachain = MockValidationDataInherentDataProvider {
									current_para_block,
									relay_offset: 1000,
									relay_blocks_per_para_block: 2,
									xcm_config: MockXcmConfig::new(
										&*client_for_xcm,
										block,
										Default::default(),
										Default::default(),
									),
									raw_downward_messages: vec![],
									raw_horizontal_messages: vec![],
								};

								Ok((timestamp, slot, mocked_parachain))
							}
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

				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
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

pub async fn start_dev_node(
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
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
			let create_inherent_data_providers = Box::new(move |_, _| async move {
				let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

				let slot = sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
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
			let slot_duration = sc_consensus_aura::slot_duration(&*client)?;
			let client_for_cidp = client.clone();

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
							xcm_config: MockXcmConfig::new(
								&*client_for_xcm,
								block,
								Default::default(),
								Default::default(),
							),
							raw_downward_messages: vec![],
							raw_horizontal_messages: vec![],
						};

						Ok((timestamp, slot, mocked_parachain))
					}
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

	let rpc_builder = {
		let client = client.clone();
		move |_, _| {
			let deps = crate::rpc::FullDeps {
				client: client.clone(),
				command_sink: rpc_sink.clone(),
				_marker: Default::default(),
			};
			crate::rpc::create_full(deps).map_err(Into::into)
		}
	};

	let rpc_handlers = sc_service::spawn_tasks(SpawnTasksParams {
		config,
		client: client.clone(),
		backend: backend.clone(),
		task_manager: &mut task_manager,
		keystore: keystore_container.sync_keystore(),
		transaction_pool: transaction_pool.clone(),
		rpc_builder: Box::new(rpc_builder),
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

async fn build_relay_chain_interface(
	relay_chain_config: Configuration,
	collator_key: Option<CollatorPair>,
	collator_options: CollatorOptions,
	task_manager: &mut TaskManager,
) -> RelayChainResult<Arc<dyn RelayChainInterface + 'static>> {
	if let Some(relay_chain_url) = collator_options.relay_chain_rpc_url {
		return Ok(Arc::new(RelayChainRPCInterface::new(relay_chain_url).await?) as Arc<_>);
	}

	let relay_chain_full_node = polkadot_test_service::new_full(
		relay_chain_config,
		if let Some(ref key) = collator_key {
			polkadot_service::IsCollator::Yes(key.clone())
		} else {
			polkadot_service::IsCollator::Yes(CollatorPair::generate().0)
		},
		None,
	)?;

	task_manager.add_child(relay_chain_full_node.task_manager);
	Ok(Arc::new(RelayChainInProcessInterface::new(
		relay_chain_full_node.client.clone(),
		relay_chain_full_node.backend.clone(),
		Arc::new(Mutex::new(Box::new(relay_chain_full_node.network.clone()))),
		relay_chain_full_node.overseer_handle,
	)) as Arc<_>)
}

/// Start a node with the given parachain `Configuration` and relay chain `Configuration`.
///
/// This is the actual implementation that is abstract over the executor and the runtime api.
#[sc_tracing::logging::prefix_logs_with(parachain_config.network.node_name.as_str())]
pub async fn start_node_impl<RB>(
	parachain_config: Configuration,
	collator_key: Option<CollatorPair>,
	relay_chain_config: Configuration,
	collator_options: CollatorOptions,
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
	RB: Fn(Arc<Client>) -> Result<RpcModule<()>, sc_service::Error> + Send + 'static,
{
	if matches!(parachain_config.role, Role::Light) {
		return Err("Light client not supported!".into());
	}

	let parachain_config = prepare_node_config(parachain_config);

	let params = new_partial(&parachain_config, seal_mode)?;
	let keystore = params.keystore_container.sync_keystore();
	let force_authoring = parachain_config.force_authoring;

	let transaction_pool = params.transaction_pool.clone();
	let mut task_manager = params.task_manager;

	let client = params.client.clone();
	let backend = params.backend.clone();
	let backend_for_node = backend.clone();

	let relay_chain_interface = build_relay_chain_interface(
		relay_chain_config,
		collator_key.clone(),
		collator_options.clone(),
		&mut task_manager,
	)
	.await
	.map_err(|e| match e {
		RelayChainError::ServiceError(polkadot_service::Error::Sub(x)) => x,
		s => s.to_string().into(),
	})?;

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

	let rpc_builder = {
		let client = client.clone();

		move |_, _| rpc_ext_builder(client.clone())
	};

	let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
		rpc_builder: Box::new(rpc_builder),
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
						let relay_chain_interface = relay_chain_interface_for_closure.clone();
						async move {
							let parachain_inherent =
								cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
									relay_parent,
									&relay_chain_interface,
									&validation_data,
									para_id,
								)
								.await;

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
							let relay_chain_interface = relay_chain_interface_for_closure.clone();
							async move {
								let parachain_inherent =
									cumulus_primitives_parachain_inherent::ParachainInherentData::create_at(
										relay_parent,
										&relay_chain_interface,
										&validation_data,
										para_id,
									)
									.await;

								let time = sp_timestamp::InherentDataProvider::from_system_time();

								let slot =
									sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
										*time,
										slot_duration,
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
			collator_options,
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
