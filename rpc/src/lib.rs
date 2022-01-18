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

mod evm_tracing;

use primitives::{AccountId, Balance, Block, CurrencyId, DataProviderId, Nonce};
use sc_client_api::backend::Backend;
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Backend as BlockchainBackend, Error as BlockChainError, HeaderBackend, HeaderMetadata};
use std::sync::Arc;

pub use sc_rpc::SubscriptionTaskExecutor;

pub use evm_rpc::{EVMApi, EVMApiServer, EVMRuntimeRPCApi};

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpc_core::IoHandler<sc_rpc::Metadata>;

/// Full client dependencies.
pub struct FullDeps<C, P, BE> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Backend
	pub backend: Arc<BE>,
	/// Transaction pool instance.
	pub pool: Arc<P>,
	/// Whether to deny unsafe calls
	pub deny_unsafe: DenyUnsafe,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, BE>(deps: FullDeps<C, P, BE>) -> RpcExtension
where
	BE: Backend<Block> + 'static,
	BE::Blockchain: BlockchainBackend<Block>,
	C: ProvideRuntimeApi<Block>,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
	C: Send + Sync + 'static,
	C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
	C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
	C::Api: orml_oracle_rpc::OracleRuntimeApi<Block, DataProviderId, CurrencyId, runtime_common::TimeStampedPrice>,
	C::Api: EVMRuntimeRPCApi<Block, Balance>,
	C::Api: BlockBuilder<Block>,
	C::Api: primitives_evm_tracing::runtime_api::EvmTracingRuntimeApi<Block>,
	P: TransactionPool + Sync + Send + 'static,
{
	use evm_tracing::{EvmTracing, EvmTracingApi};
	use orml_oracle_rpc::{Oracle, OracleApi};
	use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApi};
	use substrate_frame_rpc_system::{FullSystem, SystemApi};

	let mut io = jsonrpc_core::IoHandler::default();
	let FullDeps {
		client,
		backend,
		pool,
		deny_unsafe,
	} = deps;

	io.extend_with(SystemApi::to_delegate(FullSystem::new(
		client.clone(),
		pool,
		deny_unsafe,
	)));
	io.extend_with(TransactionPaymentApi::to_delegate(TransactionPayment::new(
		client.clone(),
	)));
	// Making synchronous calls in light client freezes the browser currently,
	// more context: https://github.com/paritytech/substrate/pull/3480
	// These RPCs should use an asynchronous caller instead.
	io.extend_with(OracleApi::to_delegate(Oracle::new(client.clone())));
	io.extend_with(EVMApiServer::to_delegate(EVMApi::new(client.clone(), deny_unsafe)));
	io.extend_with(EvmTracingApi::to_delegate(EvmTracing::new(client, backend)));

	io
}
