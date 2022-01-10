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

//! Acala Client abstractions.

use acala_primitives::{AccountId, Balance, Block, BlockNumber, CurrencyId, DataProviderId, Hash, Header, Nonce};
use runtime_common::TimeStampedPrice;
use sc_client_api::{Backend as BackendT, BlockchainEvents, KeyIterator};
use sp_api::{CallApiAt, NumberFor, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus::BlockStatus;
use sp_runtime::{
	generic::{BlockId, SignedBlock},
	traits::{BlakeTwo256, Block as BlockT},
	Justifications,
};
use sp_storage::{ChildInfo, StorageData, StorageKey};
use std::sync::Arc;

/// A set of APIs that polkadot-like runtimes must implement.
pub trait RuntimeApiCollection:
	sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
	+ sp_api::ApiExt<Block>
	+ sp_block_builder::BlockBuilder<Block>
	+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
	+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
	+ orml_oracle_rpc::OracleRuntimeApi<Block, DataProviderId, CurrencyId, TimeStampedPrice>
	+ module_evm_rpc_runtime_api::EVMRuntimeRPCApi<Block, Balance>
	+ sp_api::Metadata<Block>
	+ sp_offchain::OffchainWorkerApi<Block>
	+ sp_session::SessionKeys<Block>
	+ cumulus_primitives_core::CollectCollationInfo<Block>
where
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

impl<Api> RuntimeApiCollection for Api
where
	Api: sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block>
		+ sp_api::ApiExt<Block>
		+ sp_block_builder::BlockBuilder<Block>
		+ frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce>
		+ pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance>
		+ orml_oracle_rpc::OracleRuntimeApi<Block, DataProviderId, CurrencyId, TimeStampedPrice>
		+ module_evm_rpc_runtime_api::EVMRuntimeRPCApi<Block, Balance>
		+ sp_api::Metadata<Block>
		+ sp_offchain::OffchainWorkerApi<Block>
		+ sp_session::SessionKeys<Block>
		+ cumulus_primitives_core::CollectCollationInfo<Block>,
	<Self as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
{
}

/// Config that abstracts over all available client implementations.
///
/// For a concrete type there exists [`Client`].
pub trait AbstractClient<Block, Backend>:
	BlockchainEvents<Block>
	+ Sized
	+ Send
	+ Sync
	+ ProvideRuntimeApi<Block>
	+ HeaderBackend<Block>
	+ CallApiAt<Block, StateBackend = Backend::State>
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Self::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

impl<Block, Backend, Client> AbstractClient<Block, Backend> for Client
where
	Block: BlockT,
	Backend: BackendT<Block>,
	Backend::State: sp_api::StateBackend<BlakeTwo256>,
	Client: BlockchainEvents<Block>
		+ ProvideRuntimeApi<Block>
		+ HeaderBackend<Block>
		+ Sized
		+ Send
		+ Sync
		+ CallApiAt<Block, StateBackend = Backend::State>,
	Client::Api: RuntimeApiCollection<StateBackend = Backend::State>,
{
}

/// Execute something with the client instance.
///
/// As there exist multiple chains inside Acala, like Acala itself, Karura,
/// Mandala etc, there can exist different kinds of client types. As these
/// client types differ in the generics that are being used, we can not easily
/// return them from a function. For returning them from a function there exists
/// [`Client`]. However, the problem on how to use this client instance still
/// exists. This trait "solves" it in a dirty way. It requires a type to
/// implement this trait and than the [`execute_with_client`](ExecuteWithClient:
/// :execute_with_client) function can be called with any possible client
/// instance.
///
/// In a perfect world, we could make a closure work in this way.
pub trait ExecuteWithClient {
	/// The return type when calling this instance.
	type Output;

	/// Execute whatever should be executed with the given client instance.
	fn execute_with_client<Client, Api, Backend>(self, client: Arc<Client>) -> Self::Output
	where
		<Api as sp_api::ApiExt<Block>>::StateBackend: sp_api::StateBackend<BlakeTwo256>,
		Backend: sc_client_api::Backend<Block>,
		Backend::State: sp_api::StateBackend<BlakeTwo256>,
		Api: crate::RuntimeApiCollection<StateBackend = Backend::State>,
		Client: AbstractClient<Block, Backend, Api = Api> + 'static;
}

/// A handle to a Acala client instance.
///
/// The Acala service supports multiple different runtimes (Karura, Acala
/// itself, etc). As each runtime has a specialized client, we need to hide them
/// behind a trait. This is this trait.
///
/// When wanting to work with the inner client, you need to use `execute_with`.
pub trait ClientHandle {
	/// Execute the given something with the client.
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output;
}

/// A client instance of Acala.
#[derive(Clone)]
pub enum Client {
	#[cfg(feature = "with-mandala-runtime")]
	Mandala(Arc<crate::FullClient<mandala_runtime::RuntimeApi, crate::MandalaExecutorDispatch>>),
	#[cfg(feature = "with-karura-runtime")]
	Karura(Arc<crate::FullClient<karura_runtime::RuntimeApi, crate::KaruraExecutorDispatch>>),
	#[cfg(feature = "with-acala-runtime")]
	Acala(Arc<crate::FullClient<acala_runtime::RuntimeApi, crate::AcalaExecutorDispatch>>),
}

impl ClientHandle for Client {
	fn execute_with<T: ExecuteWithClient>(&self, t: T) -> T::Output {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => T::execute_with_client::<_, _, crate::FullBackend>(t, client.clone()),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => T::execute_with_client::<_, _, crate::FullBackend>(t, client.clone()),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => T::execute_with_client::<_, _, crate::FullBackend>(t, client.clone()),
		}
	}
}

impl sc_client_api::UsageProvider<Block> for Client {
	fn usage_info(&self) -> sc_client_api::ClientInfo<Block> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.usage_info(),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.usage_info(),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.usage_info(),
		}
	}
}

impl sc_client_api::BlockBackend<Block> for Client {
	fn block_body(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Vec<<Block as BlockT>::Extrinsic>>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.block_body(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.block_body(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.block_body(id),
		}
	}

	fn block(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<SignedBlock<Block>>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.block(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.block(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.block(id),
		}
	}

	fn block_status(&self, id: &BlockId<Block>) -> sp_blockchain::Result<BlockStatus> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.block_status(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.block_status(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.block_status(id),
		}
	}

	fn justifications(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Justifications>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.justifications(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.justifications(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.justifications(id),
		}
	}

	fn block_hash(&self, number: NumberFor<Block>) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.block_hash(number),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.block_hash(number),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.block_hash(number),
		}
	}

	fn indexed_transaction(&self, hash: &<Block as BlockT>::Hash) -> sp_blockchain::Result<Option<Vec<u8>>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.indexed_transaction(hash),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.indexed_transaction(hash),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.indexed_transaction(hash),
		}
	}

	fn has_indexed_transaction(&self, hash: &<Block as BlockT>::Hash) -> sp_blockchain::Result<bool> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.has_indexed_transaction(hash),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.has_indexed_transaction(hash),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.has_indexed_transaction(hash),
		}
	}

	fn block_indexed_body(&self, id: &BlockId<Block>) -> sp_blockchain::Result<Option<Vec<Vec<u8>>>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.block_indexed_body(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.block_indexed_body(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.block_indexed_body(id),
		}
	}
}

impl sc_client_api::StorageProvider<Block, crate::FullBackend> for Client {
	fn storage(&self, id: &BlockId<Block>, key: &StorageKey) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.storage(id, key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.storage(id, key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.storage(id, key),
		}
	}

	fn storage_keys(&self, id: &BlockId<Block>, key_prefix: &StorageKey) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.storage_keys(id, key_prefix),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.storage_keys(id, key_prefix),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.storage_keys(id, key_prefix),
		}
	}

	fn storage_hash(
		&self,
		id: &BlockId<Block>,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.storage_hash(id, key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.storage_hash(id, key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.storage_hash(id, key),
		}
	}

	fn storage_pairs(
		&self,
		id: &BlockId<Block>,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<(StorageKey, StorageData)>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.storage_pairs(id, key_prefix),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.storage_pairs(id, key_prefix),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.storage_pairs(id, key_prefix),
		}
	}

	fn storage_keys_iter<'a>(
		&self,
		id: &BlockId<Block>,
		prefix: Option<&'a StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<KeyIterator<'a, <crate::FullBackend as sc_client_api::Backend<Block>>::State, Block>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.storage_keys_iter(id, prefix, start_key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.storage_keys_iter(id, prefix, start_key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.storage_keys_iter(id, prefix, start_key),
		}
	}

	fn child_storage_keys_iter<'a>(
		&self,
		id: &BlockId<Block>,
		child_info: ChildInfo,
		prefix: Option<&'a StorageKey>,
		start_key: Option<&StorageKey>,
	) -> sp_blockchain::Result<KeyIterator<'a, <crate::FullBackend as sc_client_api::Backend<Block>>::State, Block>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.child_storage_keys_iter(id, child_info, prefix, start_key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.child_storage_keys_iter(id, child_info, prefix, start_key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.child_storage_keys_iter(id, child_info, prefix, start_key),
		}
	}

	fn child_storage(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<StorageData>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.child_storage(id, child_info, key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.child_storage(id, child_info, key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.child_storage(id, child_info, key),
		}
	}

	fn child_storage_keys(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key_prefix: &StorageKey,
	) -> sp_blockchain::Result<Vec<StorageKey>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.child_storage_keys(id, child_info, key_prefix),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.child_storage_keys(id, child_info, key_prefix),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.child_storage_keys(id, child_info, key_prefix),
		}
	}

	fn child_storage_hash(
		&self,
		id: &BlockId<Block>,
		child_info: &ChildInfo,
		key: &StorageKey,
	) -> sp_blockchain::Result<Option<<Block as BlockT>::Hash>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.child_storage_hash(id, child_info, key),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.child_storage_hash(id, child_info, key),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.child_storage_hash(id, child_info, key),
		}
	}
}

impl sp_blockchain::HeaderBackend<Block> for Client {
	fn header(&self, id: BlockId<Block>) -> sp_blockchain::Result<Option<Header>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.header(&id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.header(&id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.header(&id),
		}
	}

	fn info(&self) -> sp_blockchain::Info<Block> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.info(),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.info(),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.info(),
		}
	}

	fn status(&self, id: BlockId<Block>) -> sp_blockchain::Result<sp_blockchain::BlockStatus> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.status(id),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.status(id),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.status(id),
		}
	}

	fn number(&self, hash: Hash) -> sp_blockchain::Result<Option<BlockNumber>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.number(hash),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.number(hash),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.number(hash),
		}
	}

	fn hash(&self, number: BlockNumber) -> sp_blockchain::Result<Option<Hash>> {
		match self {
			#[cfg(feature = "with-mandala-runtime")]
			Self::Mandala(client) => client.hash(number),
			#[cfg(feature = "with-karura-runtime")]
			Self::Karura(client) => client.hash(number),
			#[cfg(feature = "with-acala-runtime")]
			Self::Acala(client) => client.hash(number),
		}
	}
}
