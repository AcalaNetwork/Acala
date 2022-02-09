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
use super::*;

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

			const ERROR: &str = "manual-seal authorship task is shutting down";
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
			.submit_one(&BlockId::Hash(at), TransactionSource::Local, extrinsic)
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
			.state_at(id)
			.unwrap_or_else(|_| panic!("State at block {} not found", id));

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
