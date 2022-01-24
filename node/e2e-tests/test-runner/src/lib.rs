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

#![allow(clippy::all)]

use sc_consensus::BlockImport;
use sc_executor::{NativeElseWasmExecutor, NativeExecutionDispatch};
use sc_service::TFullClient;
use sp_api::{ConstructRuntimeApi, TransactionFor};
use sp_consensus::SelectChain;
use sp_inherents::InherentDataProvider;
use sp_runtime::traits::{Block as BlockT, SignedExtension};

mod client;
mod host_functions;
mod node;
mod utils;

pub use client::*;
pub use host_functions::*;
pub use node::*;
pub use utils::*;

/// Wrapper trait for concrete type required by this testing framework.
pub trait ChainInfo: Sized {
	/// Opaque block type
	type Block: BlockT;

	/// ExecutorDispatch dispatch type
	type ExecutorDispatch: NativeExecutionDispatch + 'static;

	/// Runtime
	type Runtime: frame_system::Config;

	/// RuntimeApi
	type RuntimeApi: Send
		+ Sync
		+ 'static
		+ ConstructRuntimeApi<
			Self::Block,
			TFullClient<Self::Block, Self::RuntimeApi, NativeElseWasmExecutor<Self::ExecutorDispatch>>,
		>;

	/// select chain type.
	type SelectChain: SelectChain<Self::Block> + 'static;

	/// Block import type.
	type BlockImport: Send
		+ Sync
		+ Clone
		+ BlockImport<
			Self::Block,
			Error = sp_consensus::Error,
			Transaction = TransactionFor<
				TFullClient<Self::Block, Self::RuntimeApi, NativeElseWasmExecutor<Self::ExecutorDispatch>>,
				Self::Block,
			>,
		> + 'static;

	/// The signed extras required by the runtime
	type SignedExtras: SignedExtension;

	/// The inherent data providers.
	type InherentDataProviders: InherentDataProvider + 'static;

	/// Signed extras, this function is caled in an externalities provided environment.
	fn signed_extras(from: <Self::Runtime as frame_system::Config>::AccountId) -> Self::SignedExtras;
}
