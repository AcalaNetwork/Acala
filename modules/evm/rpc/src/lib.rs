use std::{marker::PhantomData, sync::Arc};

use ethereum_types::U256;
use jsonrpc_core::{Error, ErrorCode, Result, Value};
use rustc_hex::ToHex;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_runtime::{
	codec::Codec,
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
};

use call_request::CallRequest;
pub use module_evm::ExitReason;
pub use module_evm_rpc_runtime_api::EVMRuntimeRPCApi;

pub use crate::evm_api::{EVMApi as EVMApiT, EVMApiServer};

mod call_request;
mod evm_api;

fn internal_err<T: ToString>(message: T) -> Error {
	Error {
		code: ErrorCode::InternalError,
		message: message.to_string(),
		data: None,
	}
}

#[allow(dead_code)]
fn error_on_execution_failure(reason: &ExitReason, data: &[u8]) -> Result<()> {
	match reason {
		ExitReason::Succeed(_) => Ok(()),
		ExitReason::Error(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("execution error: {:?}", e),
			data: Some(Value::String("0x".to_string())),
		}),
		ExitReason::Revert(_) => {
			let message = String::from_utf8_lossy(&data);
			Err(Error {
				code: ErrorCode::InternalError,
				message: format!("execution revert: {}", message),
				data: Some(Value::String(data.to_hex())),
			})
		}
		ExitReason::Fatal(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("execution fatal: {:?}", e),
			data: Some(Value::String("0x".to_string())),
		}),
	}
}

pub struct EVMApi<B, C, Balance> {
	client: Arc<C>,
	_marker: PhantomData<(B, Balance)>,
}

impl<B, C, Balance> EVMApi<B, C, Balance> {
	pub fn new(client: Arc<C>) -> Self {
		Self {
			client,
			_marker: Default::default(),
		}
	}
}

impl<B, C, Balance> EVMApiT<B, Balance> for EVMApi<B, C, Balance>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EVMRuntimeRPCApi<B, Balance>,
	Balance: Codec + MaybeDisplay + MaybeFromStr + Default + Send + Sync + 'static,
{
	fn call(&self, request: CallRequest<Balance>, _: Option<B>) -> Result<Bytes> {
		let hash = self.client.info().best_hash;

		let CallRequest {
			from,
			to,
			gas_limit,
			value,
			data,
		} = request;

		let gas_limit = gas_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
		let data = data.map(|d| d.0).unwrap_or_default();

		let api = self.client.runtime_api();

		match to {
			Some(to) => {
				let info = api
					.call(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						to,
						data,
						value.unwrap_or_default(),
						gas_limit,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {}", Into::<&str>::into(err))))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				Ok(Bytes(info.output))
			}
			None => {
				let info = api
					.create(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						data,
						value.unwrap_or_default(),
						gas_limit,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {}", Into::<&str>::into(err))))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				Ok(Bytes(info.output[..].to_vec()))
			}
		}
	}

	fn estimate_gas(&self, request: CallRequest<Balance>, _: Option<B>) -> Result<U256> {
		let hash = self.client.info().best_hash;

		let CallRequest {
			from,
			to,
			gas_limit,
			value,
			data,
		} = request;

		let gas_limit = gas_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
		let data = data.map(|d| d.0).unwrap_or_default();

		let api = self.client.runtime_api();

		let used_gas = match to {
			Some(to) => {
				let info = api
					.call(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						to,
						data,
						value.unwrap_or_default(),
						gas_limit,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {}", Into::<&str>::into(err))))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				info.used_gas
			}
			None => {
				let info = api
					.create(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						data,
						value.unwrap_or_default(),
						gas_limit,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {}", Into::<&str>::into(err))))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				info.used_gas
			}
		};

		Ok(used_gas)
	}
}
