#![allow(clippy::upper_case_acronyms)]

use ethereum_types::U256;
use jsonrpc_core::{Error, ErrorCode, Result, Value};
use rustc_hex::ToHex;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::Bytes;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
	codec::Codec,
	generic::BlockId,
	traits::{Block as BlockT, MaybeDisplay, MaybeFromStr},
	SaturatedConversion,
};
use std::convert::{TryFrom, TryInto};
use std::{marker::PhantomData, sync::Arc};

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
		ExitReason::Revert(_) => Err(Error {
			code: ErrorCode::InternalError,
			message: decode_revert_message(data)
				.map_or("execution revert".into(), |data| format!("execution revert: {}", data)),
			data: Some(Value::String(format!("0x{}", data.to_hex::<String>()))),
		}),
		ExitReason::Fatal(e) => Err(Error {
			code: ErrorCode::InternalError,
			message: format!("execution fatal: {:?}", e),
			data: Some(Value::String("0x".to_string())),
		}),
	}
}

fn decode_revert_message(data: &[u8]) -> Option<String> {
	// A minimum size of error function selector (4) + offset (32) + string length
	// (32) should contain a utf-8 encoded revert reason.
	let msg_start: usize = 68;
	if data.len() > msg_start {
		let message_len = U256::from(&data[36..msg_start]).saturated_into::<usize>();
		let msg_end = msg_start + message_len;
		if data.len() < msg_end {
			return None;
		}
		let body: &[u8] = &data[msg_start..msg_end];
		if let Ok(reason) = std::str::from_utf8(body) {
			return Some(reason.to_string());
		}
	}
	None
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

fn to_u128(val: NumberOrHex) -> std::result::Result<u128, ()> {
	val.into_u256().try_into().map_err(|_| ())
}

impl<B, C, Balance> EVMApiT<B> for EVMApi<B, C, Balance>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EVMRuntimeRPCApi<B, Balance>,
	Balance: Codec + MaybeDisplay + MaybeFromStr + Default + Send + Sync + 'static + TryFrom<u128>,
{
	fn call(&self, request: CallRequest, _: Option<B>) -> Result<Bytes> {
		let hash = self.client.info().best_hash;

		let CallRequest {
			from,
			to,
			gas_limit,
			storage_limit,
			value,
			data,
		} = request;

		let gas_limit = gas_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
		let storage_limit = storage_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
		let data = data.map(|d| d.0).unwrap_or_default();

		let api = self.client.runtime_api();

		let balance_value = if let Some(value) = value {
			to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
		} else {
			Ok(Default::default())
		};

		let balance_value = balance_value.map_err(|_| Error {
			code: ErrorCode::InvalidParams,
			message: format!("Invalid parameter value: {:?}", value),
			data: None,
		})?;

		match to {
			Some(to) => {
				let info = api
					.call(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						to,
						data,
						balance_value,
						gas_limit,
						storage_limit,
						false,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				Ok(Bytes(info.output))
			}
			None => {
				let info = api
					.create(
						&BlockId::Hash(hash),
						from.unwrap_or_default(),
						data,
						balance_value,
						gas_limit,
						storage_limit,
						false,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				error_on_execution_failure(&info.exit_reason, &info.output)?;

				Ok(Bytes(info.output[..].to_vec()))
			}
		}
	}

	fn estimate_gas(&self, request: CallRequest, _: Option<B>) -> Result<U256> {
		let calculate_gas_used = |request| {
			let hash = self.client.info().best_hash;

			let CallRequest {
				from,
				to,
				gas_limit,
				storage_limit,
				value,
				data,
			} = request;

			let gas_limit = gas_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
			let storage_limit = storage_limit.unwrap_or_else(u32::max_value); // TODO: set a limit
			let data = data.map(|d| d.0).unwrap_or_default();

			let balance_value = if let Some(value) = value {
				to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
			} else {
				Ok(Default::default())
			};

			let balance_value = balance_value.map_err(|_| Error {
				code: ErrorCode::InvalidParams,
				message: format!("Invalid parameter value: {:?}", value),
				data: None,
			})?;

			let used_gas = match to {
				Some(to) => {
					let info = self
						.client
						.runtime_api()
						.call(
							&BlockId::Hash(hash),
							from.unwrap_or_default(),
							to,
							data,
							balance_value,
							gas_limit,
							storage_limit,
							true,
						)
						.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
						.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

					error_on_execution_failure(&info.exit_reason, &info.output)?;

					info.used_gas
				}
				None => {
					let info = self
						.client
						.runtime_api()
						.create(
							&BlockId::Hash(hash),
							from.unwrap_or_default(),
							data,
							balance_value,
							gas_limit,
							storage_limit,
							true,
						)
						.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
						.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

					error_on_execution_failure(&info.exit_reason, &[])?;

					info.used_gas
				}
			};

			Ok(used_gas)
		};

		if cfg!(feature = "rpc_binary_search_estimate") {
			let mut lower = U256::from(21_000);
			// TODO: get a good upper limit, but below U64::max to operation overflow
			let mut upper = U256::from(1_000_000_000);
			let mut mid = upper;
			let mut best = mid;
			let mut old_best: U256;

			// if the gas estimation depends on the gas limit, then we want to binary
			// search until the change is under some threshold. but if not dependent,
			// we want to stop immediately.
			let mut change_pct = U256::from(100);
			let threshold_pct = U256::from(10);

			// invariant: lower <= mid <= upper
			while change_pct > threshold_pct {
				let mut test_request = request.clone();
				test_request.gas_limit = Some(mid.as_u32());
				match calculate_gas_used(test_request) {
					// if Ok -- try to reduce the gas used
					Ok(used_gas) => {
						old_best = best;
						best = used_gas;
						change_pct = (U256::from(100) * (old_best - best))
							.checked_div(old_best)
							.unwrap_or_default();
						upper = mid;
						mid = (lower + upper + 1) / 2;
					}

					// if Err -- we need more gas
					Err(_) => {
						lower = mid;
						mid = (lower + upper + 1) / 2;

						// exit the loop
						if mid == lower {
							break;
						}
					}
				}
			}
			Ok(best)
		} else {
			calculate_gas_used(request)
		}
	}
}

#[test]
fn decode_revert_message_should_work() {
	use sp_core::bytes::from_hex;
	assert_eq!(decode_revert_message(&vec![]), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020").unwrap();
	assert_eq!(decode_revert_message(&data), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676").unwrap();
	assert_eq!(decode_revert_message(&data), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000").unwrap();
	assert_eq!(decode_revert_message(&data), Some("error message".into()));
}
