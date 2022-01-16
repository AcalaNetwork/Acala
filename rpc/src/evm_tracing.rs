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

use jsonrpc_core::Result;
use jsonrpc_derive::rpc;

use sp_runtime::traits::Block as BlockT;

use rpc_evm_tracing::types::single;

pub enum Response {
	Single(single::TransactionTrace),
	Block(Vec<single::TransactionTrace>),
}

#[rpc(server)]
pub trait EvmTracingApi<Extrinsic> {
	#[rpc(name = "evm_traceTransaction")]
	fn trace_transaction(extrinsic: Extrinsic) -> Result<Response>;

	#[rpc(name = "evm_traceBlock")]
	fn trace_block(extrinsics: Vec<Extrinsic>) -> Result<Response>;
}

// TODO:
// 1. impl `EvmTracingApi`
// 2. add to json rpc io handler
