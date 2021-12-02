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

//! Client parts

use crate::{default_config, ChainInfo};
use futures::channel::mpsc;
use jsonrpc_core::MetaIoHandler;
use node_primitives::Block;
use node_runtime::RuntimeApi;
use node_service::{default_mock_parachain_inherent_data_provider, RuntimeApiCollection};
use sc_consensus_manual_seal::{
	rpc::{ManualSeal, ManualSealApi},
	run_manual_seal, EngineCommand, ManualSealParams,
};
use sc_executor::NativeElseWasmExecutor;
use sc_service::{
	build_network, spawn_tasks, BuildNetworkParams, ChainSpec, Configuration, SpawnTasksParams, TFullBackend,
	TFullClient, TaskManager,
};
use sc_transaction_pool::BasicPool;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ConstructRuntimeApi;
use sp_consensus::SlotData;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

type ClientParts<T> = (
	Arc<MetaIoHandler<sc_rpc::Metadata, sc_rpc_server::RpcMiddleware>>,
	TaskManager,
	Arc<TFullClient<Block, <T as ChainInfo>::RuntimeApi, NativeElseWasmExecutor<<T as ChainInfo>::ExecutorDispatch>>>,
	Arc<
		dyn TransactionPool<
			Block = Block,
			Hash = <Block as BlockT>::Hash,
			Error = sc_transaction_pool::error::Error,
			InPoolTransaction = sc_transaction_pool::Transaction<<Block as BlockT>::Hash, <Block as BlockT>::Extrinsic>,
		>,
	>,
	mpsc::Sender<EngineCommand<<Block as BlockT>::Hash>>,
	Arc<TFullBackend<Block>>,
);

/// Provide the config or chain spec for a given chain
pub enum ConfigOrChainSpec {
	/// Configuration object
	Config(Configuration),
	/// Chain spec object
	ChainSpec(Box<dyn ChainSpec>, tokio::runtime::Handle),
}
/// Creates all the client parts you need for [`Node`](crate::node::Node)
pub fn client_parts<T>(config_or_chain_spec: ConfigOrChainSpec) -> Result<ClientParts<T>, sc_service::Error>
where
	T: ChainInfo<Block = Block, RuntimeApi = node_runtime::RuntimeApi> + 'static,
	<T::RuntimeApi as ConstructRuntimeApi<
		Block,
		TFullClient<T::Block, T::RuntimeApi, NativeElseWasmExecutor<T::ExecutorDispatch>>,
	>>::RuntimeApi: RuntimeApiCollection<StateBackend = sc_client_api::StateBackendFor<TFullBackend<Block>, Block>>,
	<T::Runtime as frame_system::Config>::Call: From<frame_system::Call<T::Runtime>>,
{
	let config = match config_or_chain_spec {
		ConfigOrChainSpec::Config(config) => config,
		ConfigOrChainSpec::ChainSpec(chain_spec, tokio_handle) => default_config(tokio_handle, chain_spec),
	};

	let executor = NativeElseWasmExecutor::<T::ExecutorDispatch>::new(
		config.wasm_method,
		config.default_heap_pages,
		config.max_runtime_instances,
	);

	let (client, backend, keystore, mut task_manager) =
		sc_service::new_full_parts::<Block, RuntimeApi, NativeElseWasmExecutor<T::ExecutorDispatch>>(
			&config, None, executor,
		)?;
	let client = Arc::new(client);

	let select_chain = sc_consensus::LongestChain::new(backend.clone());

	let import_queue =
		sc_consensus_manual_seal::import_queue(Box::new(client.clone()), &task_manager.spawn_essential_handle(), None);

	let transaction_pool = BasicPool::new_full(
		config.transaction_pool.clone(),
		true.into(),
		config.prometheus_registry(),
		task_manager.spawn_essential_handle(),
		client.clone(),
	);

	let (network, system_rpc_tx, network_starter) = {
		let params = BuildNetworkParams {
			config: &config,
			client: client.clone(),
			transaction_pool: transaction_pool.clone(),
			spawn_handle: task_manager.spawn_handle(),
			import_queue,
			block_announce_validator_builder: None,
			warp_sync: None,
		};
		build_network(params)?
	};

	// offchain workers
	sc_service::build_offchain_workers(&config, task_manager.spawn_handle(), client.clone(), network.clone());

	// Proposer object for block authorship.
	let env = sc_basic_authorship::ProposerFactory::new(
		task_manager.spawn_handle(),
		client.clone(),
		transaction_pool.clone(),
		config.prometheus_registry(),
		None,
	);

	// Channel for the rpc handler to communicate with the authorship task.
	let (command_sink, commands_stream) = mpsc::channel(10);

	let rpc_sink = command_sink.clone();

	let rpc_handlers = {
		let params = SpawnTasksParams {
			config,
			client: client.clone(),
			backend: backend.clone(),
			task_manager: &mut task_manager,
			keystore: keystore.sync_keystore(),
			transaction_pool: transaction_pool.clone(),
			rpc_extensions_builder: Box::new(move |_, _| {
				let mut io = jsonrpc_core::IoHandler::default();
				io.extend_with(ManualSealApi::to_delegate(ManualSeal::new(rpc_sink.clone())));
				Ok(io)
			}),
			network,
			system_rpc_tx,
			telemetry: None,
		};
		spawn_tasks(params)?
	};

	let slot_duration = sc_consensus_aura::slot_duration(&*client)?.slot_duration();

	let create_inherent_data_providers = Box::new(move |_, _| async move {
		let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

		let slot =
			sp_consensus_aura::inherents::InherentDataProvider::from_timestamp_and_duration(*timestamp, slot_duration);

		Ok((timestamp, slot, default_mock_parachain_inherent_data_provider()))
	});

	// Background authorship future.
	let authorship_future = run_manual_seal(ManualSealParams {
		block_import: client.clone(),
		env,
		client: client.clone(),
		pool: transaction_pool.clone(),
		commands_stream,
		select_chain,
		consensus_data_provider: None,
		create_inherent_data_providers,
	});

	// spawn the authorship task as an essential task.
	task_manager
		.spawn_essential_handle()
		.spawn("manual-seal", None, authorship_future);

	network_starter.start_network();
	let rpc_handler = rpc_handlers.io_handler();

	Ok((
		rpc_handler,
		task_manager,
		client,
		transaction_pool,
		command_sink,
		backend,
	))
}
