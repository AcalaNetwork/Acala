//! EVM rpc interface.

use ethereum_types::U256;
use jsonrpc_core::Result;
use jsonrpc_derive::rpc;
use sp_core::Bytes;

pub use rpc_impl_EVMApi::gen_server::EVMApi as EVMApiServer;

use crate::call_request::CallRequest;

/// EVM rpc interface.
#[rpc(server)]
pub trait EVMApi<BlockHash> {
	/// Call contract, returning the output data.
	#[rpc(name = "evm_call")]
	fn call(&self, _: CallRequest, _: Option<BlockHash>) -> Result<Bytes>;

	/// Estimate gas needed for execution of given contract.
	#[rpc(name = "evm_estimateGas")]
	fn estimate_gas(&self, _: CallRequest, _: Option<BlockHash>) -> Result<U256>;
}
