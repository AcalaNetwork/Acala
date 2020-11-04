// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of Frontier.

// Open Ethereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Open Ethereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Open Ethereum.  If not, see <http://www.gnu.org/licenses/>.

//! Eth rpc interface.

use crate::{block_number::BlockNumber, bytes::Bytes, call_request::CallRequest};
use ethereum_types::U256;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;

pub use rpc_impl_EthApi::gen_server::EthApi as EthApiServer;

/// Eth rpc interface.
#[rpc(server)]
pub trait EthApi {
	/// Call contract, returning the output data.
	#[rpc(name = "eth_call")]
	fn call(&self, _: CallRequest, _: Option<BlockNumber>) -> Result<Bytes>;

	/// Estimate gas needed for execution of given contract.
	#[rpc(name = "eth_estimateGas")]
	fn estimate_gas(&self, _: CallRequest, _: Option<BlockNumber>) -> Result<U256>;
}
