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

#![allow(clippy::upper_case_acronyms)]

use frame_support::log;
use jsonrpsee::{
	core::{async_trait, Error as JsonRpseeError, RpcResult},
	proc_macros::rpc,
	types::error::{CallError, ErrorCode, ErrorObject},
};
use pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi;
use rustc_hex::ToHex;
use sc_rpc_api::DenyUnsafe;
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_core::{Bytes, Decode, H160, U256};
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
	codec::Codec,
	generic::BlockId,
	traits::{self, Block as BlockT, MaybeDisplay, MaybeFromStr},
	SaturatedConversion,
};
use std::{marker::PhantomData, sync::Arc};

use call_request::{CallRequest, EstimateResourcesResponse};
pub use module_evm::{ExitError, ExitReason};
pub use module_evm_rpc_runtime_api::EVMRuntimeRPCApi;
use primitives::evm::{BlockLimits, EstimateResourcesRequest};

mod call_request;

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

fn internal_err<T: ToString>(message: T) -> JsonRpseeError {
	JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
		ErrorCode::InternalError.code(),
		message.to_string(),
		None::<()>,
	)))
}

fn invalid_params<T: ToString>(message: T) -> JsonRpseeError {
	JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
		ErrorCode::InvalidParams.code(),
		message.to_string(),
		None::<()>,
	)))
}

#[allow(dead_code)]
fn error_on_execution_failure(reason: &ExitReason, data: &[u8]) -> RpcResult<()> {
	match reason {
		ExitReason::Succeed(_) => Ok(()),
		ExitReason::Error(e) => {
			if *e == ExitError::OutOfGas {
				// `ServerError(0)` will be useful in estimate gas
				Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
					ErrorCode::ServerError(0).code(),
					"out of gas".to_string(),
					None::<()>,
				))))
			} else {
				Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
					ErrorCode::InternalError.code(),
					format!("execution error: {:?}", e),
					Some("0x".to_string()),
				))))
			}
		}
		ExitReason::Revert(_) => {
			let message = "VM Exception while processing transaction: execution revert".to_string();
			Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
				ErrorCode::InternalError.code(),
				decode_revert_message(data).map_or(message.clone(), |reason| format!("{} {}", message, reason)),
				Some(format!("0x{}", data.to_hex::<String>())),
			))))
		}
		ExitReason::Fatal(e) => Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
			ErrorCode::InternalError.code(),
			format!("execution fatal: {:?}", e),
			Some("0x".to_string()),
		)))),
	}
}

fn decode_revert_message(data: &[u8]) -> Option<String> {
	// A minimum size of error function selector (4) + offset (32) + string length
	// (32) should contain a utf-8 encoded revert reason.
	let msg_start: usize = 68;
	if data.len() > msg_start {
		let message_len = U256::from(&data[36..msg_start]).saturated_into::<usize>();
		let msg_end = msg_start.checked_add(message_len)?;

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

pub struct EVM<B, C, Balance> {
	client: Arc<C>,
	_deny_unsafe: DenyUnsafe,
	_marker: PhantomData<(B, Balance)>,
}

impl<B, C, Balance> EVM<B, C, Balance> {
	pub fn new(client: Arc<C>, _deny_unsafe: DenyUnsafe) -> Self {
		Self {
			client,
			_deny_unsafe,
			_marker: Default::default(),
		}
	}
}

fn to_u128(val: NumberOrHex) -> std::result::Result<u128, ()> {
	val.into_u256().try_into().map_err(|_| ())
}

#[async_trait]
impl<B, C, Balance> EVMApiServer<<B as BlockT>::Hash> for EVM<B, C, Balance>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EVMRuntimeRPCApi<B, Balance>,
	C::Api: TransactionPaymentApi<B, Balance>,
	Balance: Codec + MaybeDisplay + MaybeFromStr + Default + Send + Sync + 'static + TryFrom<u128> + Into<U256>,
{
	fn call(&self, request: CallRequest, at: Option<<B as BlockT>::Hash>) -> RpcResult<Bytes> {
		let api = self.client.runtime_api();

		let hash = at.unwrap_or_else(|| self.client.info().best_hash);

		let block_id = BlockId::Hash(hash);

		if !self
			.client
			.runtime_api()
			.has_api::<dyn EVMRuntimeRPCApi<B, Balance>>(&block_id)
			.unwrap_or(false)
		{
			return Err(internal_err(format!(
				"Could not find `EVMRuntimeRPCApi` api for block `{:?}`.",
				&block_id
			)));
		}

		log::debug!(target: "evm", "rpc call, request: {:?}", request);

		let CallRequest {
			from,
			to,
			gas_limit,
			storage_limit,
			value,
			data,
			access_list,
		} = request;

		let block_limits = self.block_limits(at)?;

		// eth_call is capped at 10x (1000%) the current block gas limit
		let gas_limit_cap = 10 * block_limits.max_gas_limit;

		let gas_limit = gas_limit.unwrap_or(gas_limit_cap);
		if gas_limit > gas_limit_cap {
			return Err(invalid_params(format!(
				"GasLimit exceeds capped allowance: {}",
				gas_limit_cap
			)));
		}
		let storage_limit = storage_limit.unwrap_or(block_limits.max_storage_limit);
		if storage_limit > block_limits.max_storage_limit {
			return Err(invalid_params(format!(
				"StorageLimit exceeds allowance: {}",
				block_limits.max_storage_limit
			)));
		}
		let data = data.map(|d| d.0).unwrap_or_default();

		let balance_value = if let Some(value) = value {
			to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
		} else {
			Ok(Default::default())
		};

		let balance_value =
			balance_value.map_err(|_| invalid_params(format!("Invalid parameter value: {:?}", value)))?;

		match to {
			Some(to) => {
				let info = api
					.call(
						&block_id,
						from.unwrap_or_default(),
						to,
						data,
						balance_value,
						gas_limit,
						storage_limit,
						access_list,
						true,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				log::debug!(
					target: "evm",
					"rpc call, info.exit_reason: {:?}, info.value: {:?}",
					info.exit_reason, info.value,
				);
				error_on_execution_failure(&info.exit_reason, &info.value)?;

				Ok(Bytes(info.value))
			}
			None => {
				let info = api
					.create(
						&block_id,
						from.unwrap_or_default(),
						data,
						balance_value,
						gas_limit,
						storage_limit,
						access_list,
						true,
					)
					.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
					.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

				log::debug!(
					target: "evm",
					"rpc create, info.exit_reason: {:?}, info.value: {:?}",
					info.exit_reason, info.value,
				);
				error_on_execution_failure(&info.exit_reason, &[])?;

				Ok(Bytes(info.value[..].to_vec()))
			}
		}
	}

	fn estimate_resources(
		&self,
		from: H160,
		unsigned_extrinsic: Bytes,
		at: Option<<B as BlockT>::Hash>,
	) -> RpcResult<EstimateResourcesResponse> {
		let hash = at.unwrap_or_else(|| self.client.info().best_hash);

		let block_id = BlockId::Hash(hash);

		if !self
			.client
			.runtime_api()
			.has_api::<dyn EVMRuntimeRPCApi<B, Balance>>(&block_id)
			.unwrap_or(false)
		{
			return Err(internal_err(format!(
				"Could not find `EVMRuntimeRPCApi` api for block `{:?}`.",
				&block_id
			)));
		}

		let block_limits = self.block_limits(at)?;

		let request: EstimateResourcesRequest = self
			.client
			.runtime_api()
			.get_estimate_resources_request(&block_id, unsigned_extrinsic.to_vec())
			.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
			.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

		let gas_limit = core::cmp::min(
			request.gas_limit.unwrap_or(block_limits.max_gas_limit),
			block_limits.max_gas_limit,
		);

		let storage_limit = core::cmp::min(
			request.storage_limit.unwrap_or(block_limits.max_storage_limit),
			block_limits.max_storage_limit,
		);

		// Determine the highest possible gas limits
		let mut highest = gas_limit;

		let request = CallRequest {
			from: Some(from),
			to: request.to,
			gas_limit: Some(gas_limit),
			storage_limit: Some(storage_limit),
			value: request.value.map(|v| NumberOrHex::Hex(U256::from(v))),
			data: request.data.map(Bytes),
			access_list: request.access_list,
		};

		log::debug!(
			target: "evm",
			"estimate_resources, request: {:?}, hash: {:?}",
			request, hash
		);

		struct ExecutableResult {
			data: Vec<u8>,
			exit_reason: ExitReason,
			used_gas: u64,
			used_storage: i32,
		}

		// Create a helper to check if a gas allowance results in an executable transaction
		let executable = move |request: CallRequest, gas: u64| -> RpcResult<ExecutableResult> {
			let CallRequest {
				from,
				to,
				gas_limit,
				storage_limit,
				value,
				data,
				access_list,
			} = request;

			let gas_limit = gas_limit.expect("Cannot be none, value set when request is constructed above; qed");
			let storage_limit =
				storage_limit.expect("Cannot be none, value set when request is constructed above; qed");
			let data = data.map(|d| d.0).unwrap_or_default();

			// Use request gas limit only if it less than gas_limit parameter
			let gas_limit = core::cmp::min(gas_limit, gas);

			let balance_value = if let Some(value) = value {
				to_u128(value).and_then(|v| TryInto::<Balance>::try_into(v).map_err(|_| ()))
			} else {
				Ok(Default::default())
			};

			let balance_value =
				balance_value.map_err(|_| invalid_params(format!("Invalid parameter value: {:?}", value)))?;

			let (exit_reason, data, used_gas, used_storage) = match to {
				Some(to) => {
					let info = self
						.client
						.runtime_api()
						.call(
							&block_id,
							from.unwrap_or_default(),
							to,
							data,
							balance_value,
							gas_limit,
							storage_limit,
							access_list,
							true,
						)
						.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
						.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

					(info.exit_reason, info.value, info.used_gas.as_u64(), info.used_storage)
				}
				None => {
					let info = self
						.client
						.runtime_api()
						.create(
							&block_id,
							from.unwrap_or_default(),
							data,
							balance_value,
							gas_limit,
							storage_limit,
							access_list,
							true,
						)
						.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
						.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

					(info.exit_reason, Vec::new(), info.used_gas.as_u64(), info.used_storage)
				}
			};

			Ok(ExecutableResult {
				exit_reason,
				data,
				used_gas,
				used_storage,
			})
		};

		// Verify that the transaction succeed with highest capacity
		let cap = highest;
		let ExecutableResult {
			data,
			exit_reason,
			used_gas,
			used_storage,
		} = executable(request.clone(), highest)?;
		match exit_reason {
			ExitReason::Succeed(_) => (),
			ExitReason::Error(ExitError::OutOfGas) => {
				return Err(internal_err(format!("gas required exceeds allowance {}", cap)))
			}
			// If the transaction reverts, there are two possible cases,
			// it can revert because the called contract feels that it does not have enough
			// gas left to continue, or it can revert for another reason unrelated to gas.
			ExitReason::Revert(revert) => {
				if request.gas_limit.is_some() {
					// If the user has provided a gas limit, then we have executed
					// with less block gas limit, so we must reexecute with block gas limit to
					// know if the revert is due to a lack of gas or not.
					let ExecutableResult { data, exit_reason, .. } =
						executable(request.clone(), block_limits.max_gas_limit)?;
					match exit_reason {
						ExitReason::Succeed(_) => {
							return Err(internal_err(format!("gas required exceeds allowance {}", cap)))
						}
						// The execution has been done with block gas limit, so it is not a lack of gas from the user.
						other => error_on_execution_failure(&other, &data)?,
					}
				} else {
					// The execution has already been done with block gas limit, so it is not a lack of gas from the
					// user.
					error_on_execution_failure(&ExitReason::Revert(revert), &data)?
				}
			}
			other => error_on_execution_failure(&other, &data)?,
		};

		// rpc_binary_search_estimate block
		{
			// Define the lower bound of the binary search
			let mut lowest = 21_000;

			// Start close to the used gas for faster binary search
			let mut mid = std::cmp::min(used_gas * 3, (highest + lowest) / 2);

			// Execute the binary search and hone in on an executable gas limit.
			let mut previous_highest = highest;
			while (highest - lowest) > 1 {
				let ExecutableResult { data, exit_reason, .. } = executable(request.clone(), mid)?;
				match exit_reason {
					ExitReason::Succeed(_) => {
						highest = mid;
						// If the variation in the estimate is less than 10%,
						// then the estimate is considered sufficiently accurate.
						if (previous_highest - highest) * 10 / previous_highest < 1 {
							break;
						}
						previous_highest = highest;
					}
					ExitReason::Revert(_) | ExitReason::Error(ExitError::OutOfGas) => {
						lowest = mid;
					}
					other => error_on_execution_failure(&other, &data)?,
				}
				mid = (highest + lowest) / 2;
			}
		}

		let uxt: <B as traits::Block>::Extrinsic = Decode::decode(&mut &*unsigned_extrinsic)
			.map_err(|e| internal_err(format!("execution error: Unable to dry run extrinsic {:?}", e)))?;

		let fee = self
			.client
			.runtime_api()
			.query_fee_details(&block_id, uxt, unsigned_extrinsic.len() as u32)
			.map_err(|e| internal_err(format!("runtime error: Unable to query fee details {:?}", e)))?;

		let adjusted_weight_fee = fee
			.inclusion_fee
			.map_or_else(Default::default, |inclusion| inclusion.adjusted_weight_fee);

		Ok(EstimateResourcesResponse {
			gas: highest,
			storage: used_storage,
			weight_fee: adjusted_weight_fee.into(),
		})
	}

	fn block_limits(&self, at: Option<<B as BlockT>::Hash>) -> RpcResult<BlockLimits> {
		let hash = at.unwrap_or_else(|| self.client.info().best_hash);

		let block_id = BlockId::Hash(hash);

		let version = self
			.client
			.runtime_api()
			.api_version::<dyn EVMRuntimeRPCApi<B, Balance>>(&block_id)
			.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
			.ok_or_else(|| {
				internal_err(format!(
					"Could not find `EVMRuntimeRPCApi` api for block `{:?}`.",
					&block_id
				))
			})?;

		let block_limits = if version > 1 {
			self.client
				.runtime_api()
				.block_limits(&block_id)
				.map_err(|e| internal_err(format!("runtime error: Unable to query block limits {:?}", e)))?
		} else {
			BlockLimits {
				max_gas_limit: 20_000_000,    // 20M
				max_storage_limit: 4_194_304, // 4Mb
			}
		};

		Ok(block_limits)
	}
}

#[test]
fn decode_revert_message_should_work() {
	use sp_core::bytes::from_hex;
	assert_eq!(decode_revert_message(&[]), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020").unwrap();
	assert_eq!(decode_revert_message(&data), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676").unwrap();
	assert_eq!(decode_revert_message(&data), None);

	let data = from_hex("0x8c379a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000d6572726f72206d65737361676500000000000000000000000000000000000000").unwrap();
	assert_eq!(decode_revert_message(&data), Some("error message".into()));

	// ensures we protect against msg_start + message_len overflow
	let data = from_hex("0x9850188c1837189a0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000018d618571827182618f718220618d6185718371836161876").unwrap();
	assert_eq!(decode_revert_message(&data), None);
	// ensures we protect against msg_start + message_len overflow
	let data = from_hex("0x9860189818501818188c181818371818189a00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000181818d6181818571818182718181826181818f71818182206181818d61818185718181837181818361618181876").unwrap();
	assert_eq!(decode_revert_message(&data), None);
	// ensures we protect against msg_start + message_len overflow
	let data = from_hex("0x98640818c3187918a0000000000000000000000000000000000000000000000000000000000000001820000000000000000000000000000000000000000000000000000000000000000d186518721872186f18721820186d18651873187318611867186500000000000000000000000000000000000000").unwrap();
	assert_eq!(decode_revert_message(&data), None);
}
