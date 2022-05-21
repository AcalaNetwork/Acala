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

//! EVM rpc interface.

use jsonrpsee::{
	core::RpcResult,
	proc_macros::rpc,
};
use primitives::evm::BlockLimits;
use sp_core::{Bytes, H160};

use crate::call_request::{CallRequest, EstimateResourcesResponse};

/// EVM rpc interface.
#[rpc(client, server)]
pub trait EVMApi<BlockHash> {
	/// Call contract, returning the output data.
	#[method(name = "evm_call")]
	fn call(&self, call_request: CallRequest, at: Option<BlockHash>) -> RpcResult<Bytes>;

	/// Estimate resources needed for execution of given contract.
	#[method(name = "evm_estimateResources")]
	fn estimate_resources(
		&self,
		from: H160,
		unsigned_extrinsic: Bytes,
		at: Option<BlockHash>,
	) -> RpcResult<EstimateResourcesResponse>;

	/// Get max gas and storage limits per transaction
	#[method(name = "evm_blockLimits")]
	fn block_limits(&self, at: Option<BlockHash>) -> RpcResult<BlockLimits>;
}
