//! EVM rpc interface.

use crate::{bytes::Bytes, call_request::CallRequest};
use ethereum_types::U256;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;

pub use rpc_impl_EVMApi::gen_server::EVMApi as EVMApiServer;

/// EVM rpc interface.
#[rpc(server)]
pub trait EVMApi<BlockHash> {
	/// Call contract, returning the output data.
	#[rpc(name = "eth_call")]
	fn call(&self, _: CallRequest, _: Option<BlockHash>) -> Result<Bytes>;

	/// Estimate gas needed for execution of given contract.
	#[rpc(name = "eth_estimateGas")]
	fn estimate_gas(&self, _: CallRequest, _: Option<BlockHash>) -> Result<U256>;
}
