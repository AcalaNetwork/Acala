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

//! Acala-specific RPCs implementation.

#![warn(missing_docs)]

use primitives::{AccountId, Balance, Block, CurrencyId, DataProviderId, Hash, Nonce};
pub use sc_rpc::SubscriptionTaskExecutor;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;

/// substrate rpc
use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
use sc_consensus_manual_seal::rpc::{EngineCommand, ManualSeal, ManualSealApiServer};
pub use sc_rpc::dev::Dev;
pub use sc_rpc_api::{dev::DevApiServer, DenyUnsafe};
use sc_transaction_pool_api::TransactionPool;
use substrate_frame_rpc_system::{System, SystemApiServer};

/// orml rpc
use orml_oracle_rpc::{Oracle, OracleApiServer};
use orml_tokens_rpc::{Tokens, TokensApiServer};

/// module rpc
pub use evm_rpc::{EVMApiServer, EVMRuntimeRPCApi, EVM};

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Full client dependencies.
pub struct FullDeps<C, P> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
	/// Manual seal command sink
	pub command_sink: Option<futures::channel::mpsc::Sender<EngineCommand<Hash>>>,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P>(deps: FullDeps<C, P>) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block> + sc_client_api::BlockBackend<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, DataProviderId, CurrencyId, runtime_common::TimeStampedPrice>,
	C::Api: orml_tokens_rpc::TokensRuntimeApi<Block, CurrencyId, Balance>,
	C::Api: EVMRuntimeRPCApi<Block, Balance>,
	C::Api: BlockBuilder<Block>,
	P: TransactionPool + Sync + Send + 'static,
{
	let mut module = RpcExtension::new(());
	let FullDeps {
		client,
		pool,
		deny_unsafe,
		command_sink,
	} = deps;

	module.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
	module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

	// Making synchronous calls in light client freezes the browser currently,
	// more context: https://github.com/paritytech/substrate/pull/3480
	// These RPCs should use an asynchronous caller instead.
	module.merge(Oracle::new(client.clone()).into_rpc())?;
	module.merge(Tokens::new(client.clone()).into_rpc())?;
	module.merge(EVM::new(client.clone(), deny_unsafe).into_rpc())?;
	module.merge(Dev::new(client, deny_unsafe).into_rpc())?;

	if let Some(command_sink) = command_sink {
		module.merge(
			// We provide the rpc handler with the sending end of the channel to allow the rpc
			// send EngineCommands to the background block authorship task.
			ManualSeal::new(command_sink).into_rpc(),
		)?;
	}

	Ok(module)
}
