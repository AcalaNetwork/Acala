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

//! Test service specific RPCs implementation.

#![warn(missing_docs)]
use super::*;

/// A type representing all RPC extensions.
pub type RpcExtension = jsonrpsee::RpcModule<()>;

/// Full client dependencies.
pub struct FullDeps<C> {
	/// The client instance to use.
	pub client: Arc<C>,
	/// Manual seal command sink
	pub command_sink: futures::channel::mpsc::Sender<EngineCommand<Hash>>,
	pub _marker: std::marker::PhantomData<C>,
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C>(deps: FullDeps<C>) -> Result<RpcExtension, Box<dyn std::error::Error + Send + Sync>>
where
	C: ProvideRuntimeApi<Block> + sc_client_api::BlockBackend<Block> + Send + Sync + 'static,
	C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError>,
{
	let mut module = RpcExtension::new(());
	let FullDeps { command_sink, .. } = deps;

	module.merge(
		// We provide the rpc handler with the sending end of the channel to allow the rpc
		// send EngineCommands to the background block authorship task.
		ManualSeal::new(command_sink).into_rpc(),
	)?;

	Ok(module)
}
