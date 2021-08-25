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

#![allow(clippy::all)]

use manual_seal::consensus::ConsensusDataProvider;
use sc_consensus::BlockImport;
use sc_executor::NativeExecutionDispatch;
use sc_service::{Configuration, TFullBackend, TFullClient, TaskExecutor, TaskManager};
use sp_api::{ConstructRuntimeApi, TransactionFor};
use sp_consensus::SelectChain;
use sp_inherents::{CreateInherentDataProviders, InherentDataProvider};
use sp_keystore::SyncCryptoStorePtr;
use sp_runtime::traits::{Block as BlockT, SignedExtension};
use std::sync::Arc;

mod host_functions;
mod node;
mod utils;

pub use host_functions::*;
pub use node::*;
pub use utils::*;

/// Wrapper trait for concrete type required by this testing framework.
pub trait ChainInfo: Sized {
	/// Opaque block type
	type Block: BlockT;

	/// Executor type
	type Executor: NativeExecutionDispatch + 'static;

	/// Runtime
	type Runtime: frame_system::Config;

	/// RuntimeApi
	type RuntimeApi: Send
		+ Sync
		+ 'static
		+ ConstructRuntimeApi<Self::Block, TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>;

	/// select chain type.
	type SelectChain: SelectChain<Self::Block> + 'static;

	/// Block import type.
	type BlockImport: Send
		+ Sync
		+ Clone
		+ BlockImport<
			Self::Block,
			Error = sp_consensus::Error,
			Transaction = TransactionFor<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>, Self::Block>,
		> + 'static;

	/// The signed extras required by the runtime
	type SignedExtras: SignedExtension;

	/// The inherent data providers.
	type InherentDataProviders: InherentDataProvider + 'static;

	/// Signed extras, this function is caled in an externalities provided environment.
	fn signed_extras(from: <Self::Runtime as frame_system::Config>::AccountId) -> Self::SignedExtras;

	/// config factory
	fn config(task_executor: TaskExecutor) -> Configuration;

	/// Attempt to create client parts, including block import,
	/// select chain strategy and consensus data provider.
	fn create_client_parts(
		config: &Configuration,
	) -> Result<
		(
			Arc<TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>>,
			Arc<TFullBackend<Self::Block>>,
			SyncCryptoStorePtr,
			TaskManager,
			Box<dyn CreateInherentDataProviders<Self::Block, (), InherentDataProviders = Self::InherentDataProviders>>,
			Option<
				Box<
					dyn ConsensusDataProvider<
						Self::Block,
						Transaction = TransactionFor<
							TFullClient<Self::Block, Self::RuntimeApi, Self::Executor>,
							Self::Block,
						>,
					>,
				>,
			>,
			Self::SelectChain,
			Self::BlockImport,
		),
		sc_service::Error,
	>;

	/// Given a call and a handle to the node, execute the call with root privileges.
	fn dispatch_with_root(call: <Self::Runtime as frame_system::Config>::Call, node: &mut Node<Self>);
}
