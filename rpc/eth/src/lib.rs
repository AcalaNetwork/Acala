mod block_number;
mod bytes;
mod call_request;
mod eth_api;

use block_number::BlockNumber;
use bytes::Bytes;
use call_request::CallRequest;

use rustc_hex::ToHex;

use ethereum_types::U256;
use jsonrpc_core::{Error, ErrorCode, Result, Value};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::{marker::PhantomData, sync::Arc};

pub use crate::eth_api::{EthApi as EthApiT, EthApiServer};

pub use eth_rpc_runtime_api::EthereumApi as EthereumRuntimeRPCApi;

pub use evm::ExitReason;

fn internal_err<T: ToString>(message: T) -> Error {
	Error {
		code: ErrorCode::InternalError,
		message: message.to_string(),
		data: None,
	}
}

fn error_on_execution_failure(reason: &ExitReason, data: &[u8]) -> Result<()> {
	match reason {
		ExitReason::Succeed(_) => Ok(()),
		ExitReason::Error(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("evm error: {:?}", e),
			data: Some(Value::String("0x".to_string())),
		}),
		ExitReason::Revert(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("evm revert: {:?}", e),
			data: Some(Value::String(data.to_hex())),
		}),
		ExitReason::Fatal(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("evm fatal: {:?}", e),
			data: Some(Value::String("0x".to_string())),
		}),
	}
}

pub struct EthApi<B, C> {
	client: Arc<C>,
	_marker: PhantomData<B>,
}

impl<B, C> EthApi<B, C> {
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

impl<B, C> EthApiT for EthApi<B, C>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EthereumRuntimeRPCApi<B>,
{
	fn call(&self, request: CallRequest, _: Option<BlockNumber>) -> Result<Bytes> {
		let hash = self.client.info().best_hash;

		let CallRequest {
			from,
			to,
			gas_price,
			gas,
			value,
			data,
			nonce,
		} = request;

		let gas_limit = gas.unwrap_or(U256::max_value()); // TODO: set a limit
		let data = data.map(|d| d.0).unwrap_or_default();

		let api = self.client.runtime_api();

		match to {
			Some(to) => {
				let (value, _) = api
					.call(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						to,
						data,
						value.unwrap_or_default(),
						gas_limit,
						gas_price.unwrap_or(U256::one()),
						nonce,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				// error_on_execution_failure(&exit_reason, &value)?;

				Ok(Bytes(value))
			}
			None => {
				let (value, _) = api
					.create(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						data,
						value.unwrap_or_default(),
						gas_limit,
						gas_price.unwrap_or(U256::one()),
						nonce,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				// error_on_execution_failure(&exit_reason, &[])?;

				Ok(Bytes(value[..].to_vec()))
			}
		}
	}

	fn estimate_gas(&self, request: CallRequest, _: Option<BlockNumber>) -> Result<U256> {
		let hash = self.client.info().best_hash;

		let CallRequest {
			from,
			to,
			gas_price,
			gas,
			value,
			data,
			nonce,
		} = request;

		let gas_limit = gas.unwrap_or(U256::max_value()); // TODO: set a limit
		let data = data.map(|d| d.0).unwrap_or_default();

		let api = self.client.runtime_api();

		let used_gas = match to {
			Some(to) => {
				let (value, used_gas) = api
					.call(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						to,
						data,
						value.unwrap_or_default(),
						gas_limit,
						gas_price.unwrap_or(U256::one()),
						nonce,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				// error_on_execution_failure(&exit_reason, &value)?;

				used_gas
			}
			None => {
				let (_, used_gas) = api
					.create(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						data,
						value.unwrap_or_default(),
						gas_limit,
						gas_price.unwrap_or(U256::one()),
						nonce,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				// error_on_execution_failure(&exit_reason, &[])?;

				used_gas
			}
		};

		Ok(used_gas)
	}
}
