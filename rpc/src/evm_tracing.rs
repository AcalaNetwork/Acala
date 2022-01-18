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

use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;
use serde::{Deserialize, Serialize};

use std::{marker::PhantomData, sync::Arc};

use sc_client_api::backend::Backend;
use sp_api::{BlockId, Core, HeaderT, ProvideRuntimeApi};
use sp_blockchain::{Backend as BlockchainBackend, HeaderBackend};
use sp_runtime::traits::Block as BlockT;

use primitives_evm_tracing::runtime_api::{EvmTracingRuntimeApi, TracerInput};
use rpc_evm_tracing::{
	formatters::{self, ResponseFormatter},
	listeners,
	types::single,
};

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceParams {
	pub disable_storage: Option<bool>,
	pub disable_memory: Option<bool>,
	pub disable_stack: Option<bool>,
	/// JavaScript tracer (we just check if it's BlockScout tracer string)
	pub tracer: Option<String>,
	pub timeout: Option<String>,
}

#[rpc(server)]
pub trait EvmTracingApi<Extrinsic, BlockHash> {
	#[rpc(name = "evm_traceTransaction")]
	fn trace_transaction(
		&self,
		extrinsic: Extrinsic,
		block_hash: BlockHash,
		params: Option<TraceParams>,
	) -> Result<Response>;

	#[rpc(name = "evm_traceBlock")]
	fn trace_block(&self, block_hash: BlockHash, params: Option<TraceParams>) -> Result<Response>;
}

#[derive(Serialize)]
pub enum Response {
	Single(single::TransactionTrace),
	Block(Vec<single::TransactionTrace>),
}

// TODO:
// 1. impl `EvmTracingApi`
// 2. add to json rpc io handler

/// EVM tracing RPC.
pub struct EvmTracing<C, B, BE> {
	client: Arc<C>,
	backend: Arc<BE>,
	_marker: PhantomData<B>,
}

impl<C, B, BE> EvmTracing<C, B, BE> {
	pub fn new(client: Arc<C>, backend: Arc<BE>) -> Self {
		Self {
			client,
			backend,
			_marker: Default::default(),
		}
	}
}

impl<C, B, BE> EvmTracingApi<<B as BlockT>::Extrinsic, <B as BlockT>::Hash> for EvmTracing<C, B, BE>
where
	B: BlockT,
	BE: Backend<B> + 'static,
	BE::Blockchain: BlockchainBackend<B>,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EvmTracingRuntimeApi<B>,
{
	fn trace_transaction(
		&self,
		extrinsic: B::Extrinsic,
		block_hash: B::Hash,
		params: Option<TraceParams>,
	) -> Result<Response> {
		let (tracer_input, trace_type) = parse_params(params)?;

		let invalid_block_err = invalid_params_err(format!("invalid  block hash: {:?}", block_hash));
		let header = self
			.client
			.header(BlockId::Hash(block_hash))
			.map_err(|_| invalid_block_err.clone())?
			.ok_or(invalid_block_err)?;
		let parent_block_id = BlockId::Hash(*header.parent_hash());
		let api = self.client.runtime_api();
		let f = || -> Result<_> {
			api.initialize_block(&parent_block_id, &header)
				.map_err(|e| internal_err(format!("Runtime api access error: {:?}", e)))?;

			let _ = api
				.trace_transaction(&parent_block_id, extrinsic)
				.map_err(|e| internal_err(format!("Runtime api access error: {:?}", e)))?
				.map_err(|e| internal_err(format!("DispatchError: {:?}", e)))?;

			Ok(primitives_evm_tracing::runtime_api::Response::Single)
		};

		return match trace_type {
			single::TraceType::Raw {
				disable_storage,
				disable_memory,
				disable_stack,
			} => {
				let mut proxy = listeners::Raw::new(disable_storage, disable_memory, disable_stack);
				proxy.using(f)?;
				Ok(Response::Single(formatters::Raw::format(proxy).unwrap()))
			}
			single::TraceType::CallList => {
				let mut proxy = listeners::CallList::default();
				proxy.using(f)?;
				proxy.finish_transaction();
				let response = match tracer_input {
					TracerInput::BlockScout => formatters::Blockscout::format(proxy)
						.ok_or("Trace result is empty.")
						.map_err(|e| internal_err(format!("{:?}", e))),
					TracerInput::CallTracer => {
						let mut res = formatters::CallTracer::format(proxy)
							.ok_or("Trace result is empty.")
							.map_err(|e| internal_err(format!("{:?}", e)))?;
						Ok(res.pop().unwrap())
					}
					_ => Err(internal_err(format!("Bug: failed to resolve the tracer format."))),
				}?;
				Ok(Response::Single(response))
			}
			not_supported => Err(internal_err(format!(
				"Bug: `handle_transaction_request` does not support {:?}.",
				not_supported
			))),
		};
	}

	fn trace_block(&self, block_hash: B::Hash, params: Option<TraceParams>) -> Result<Response> {
		let (tracer_input, trace_type) = parse_params(params)?;

		let block_id = BlockId::Hash(block_hash);
		let invalid_block_err = invalid_params_err(format!("invalid block hash: {:?}", block_hash));
		let header = self
			.client
			.header(block_id)
			.map_err(|_| invalid_block_err.clone())?
			.ok_or(invalid_block_err.clone())?;
		let parent_block_id = BlockId::Hash(*header.parent_hash());

		let extrinsics = self
			.backend
			.blockchain()
			.body(block_id)
			.map_err(|_| invalid_block_err.clone())?
			.ok_or(invalid_block_err)?;

		let api = self.client.runtime_api();
		let f = || -> Result<_> {
			api.initialize_block(&parent_block_id, &header)
				.map_err(|e| internal_err(format!("Runtime api access error: {:?}", e)))?;

			let _ = api
				.trace_block(&parent_block_id, extrinsics)
				.map_err(|e| {
					internal_err(format!(
						"Blockchain error when replaying block {} : {:?}",
						block_hash, e
					))
				})?
				.map_err(|e| {
					internal_err(format!(
						"Internal runtime error when replaying block {} : {:?}",
						block_hash, e
					))
				})?;
			Ok(primitives_evm_tracing::runtime_api::Response::Block)
		};

		return match trace_type {
			single::TraceType::CallList => {
				let mut proxy = listeners::CallList::default();
				proxy.using(f)?;
				proxy.finish_transaction();
				let response = match tracer_input {
					TracerInput::CallTracer => formatters::CallTracer::format(proxy)
						.ok_or("Trace result is empty.")
						.map_err(|e| internal_err(format!("{:?}", e))),
					_ => Err(internal_err(format!("Bug: failed to resolve the tracer format."))),
				}?;

				Ok(Response::Block(response))
			}
			not_supported => Err(internal_err(format!(
				"Bug: `trace_block` does not support {:?}.",
				not_supported
			))),
		};
	}
}

fn parse_params(params: Option<TraceParams>) -> Result<(TracerInput, single::TraceType)> {
	match params {
		Some(TraceParams {
			tracer: Some(tracer), ..
		}) => {
			const BLOCK_SCOUT_JS_CODE_HASH: [u8; 16] = [
				148, 217, 240, 135, 150, 249, 30, 177, 58, 46, 130, 166, 6, 104, 130, 247,
			];
			let hash = sp_io::hashing::twox_128(&tracer.as_bytes());
			let maybe_tracer_input = if hash == BLOCK_SCOUT_JS_CODE_HASH {
				Some(TracerInput::BlockScout)
			} else if tracer == "callTracer" {
				Some(TracerInput::CallTracer)
			} else {
				None
			};
			if let Some(tracer_input) = maybe_tracer_input {
				Ok((tracer_input, single::TraceType::CallList))
			} else {
				return Err(internal_err(format!(
					"javascript based tracing is not available (hash :{:?})",
					hash
				)));
			}
		}
		Some(params) => Ok((
			TracerInput::None,
			single::TraceType::Raw {
				disable_storage: params.disable_storage.unwrap_or(false),
				disable_memory: params.disable_memory.unwrap_or(false),
				disable_stack: params.disable_stack.unwrap_or(false),
			},
		)),
		_ => Ok((
			TracerInput::None,
			single::TraceType::Raw {
				disable_storage: false,
				disable_memory: false,
				disable_stack: false,
			},
		)),
	}
}

fn internal_err<T: ToString>(message: T) -> Error {
	Error {
		code: ErrorCode::InternalError,
		message: message.to_string(),
		data: None,
	}
}

fn invalid_params_err<T: ToString>(message: T) -> Error {
	Error {
		code: ErrorCode::InvalidParams,
		message: message.to_string(),
		data: None,
	}
}
