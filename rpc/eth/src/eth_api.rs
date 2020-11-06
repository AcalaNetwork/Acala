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
