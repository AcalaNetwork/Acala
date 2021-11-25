// This file is part of Acala.

// Copyright (C) 2020-2021 Acala Foundation.
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

use ethereum_types::{H160, U256};
use frame_support::log;
use jsonrpc_core::{Error, ErrorCode, Result, Value};
use pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi;
use rustc_hex::ToHex;
use sc_rpc_api::DenyUnsafe;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::{Bytes, Decode};
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
	codec::Codec,
	generic::BlockId,
	traits::{self, Block as BlockT, MaybeDisplay, MaybeFromStr},
	SaturatedConversion,
};
use std::convert::{TryFrom, TryInto};
use std::{marker::PhantomData, sync::Arc};

use call_request::{CallRequest, EstimateResourcesResponse};
pub use module_evm::{ExitError, ExitReason};
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
		ExitReason::Error(e) => {
			if *e == ExitError::OutOfGas {
				// `ServerError(0)` will be useful in estimate gas
				return Err(Error {
					code: ErrorCode::ServerError(0),
					message: "out of gas".to_string(),
					data: None,
				});
			}
			Err(Error {
				code: ErrorCode::InternalError,
				message: format!("execution error: {:?}", e),
				data: Some(Value::String("0x".to_string())),
			})
		}
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

pub struct EVMApi<B, C, Balance> {
	client: Arc<C>,
	deny_unsafe: DenyUnsafe,
	_marker: PhantomData<(B, Balance)>,
}

impl<B, C, Balance> EVMApi<B, C, Balance> {
	pub fn new(client: Arc<C>, deny_unsafe: DenyUnsafe) -> Self {
		Self {
			client,
			deny_unsafe,
			_marker: Default::default(),
		}
	}
}

fn to_u128(val: NumberOrHex) -> std::result::Result<u128, ()> {
	val.into_u256().try_into().map_err(|_| ())
}

impl<B, C, Balance> EVMApiT<<B as BlockT>::Hash> for EVMApi<B, C, Balance>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: EVMRuntimeRPCApi<B, Balance>,
	C::Api: TransactionPaymentApi<B, Balance>,
	Balance: Codec + MaybeDisplay + MaybeFromStr + Default + Send + Sync + 'static + TryFrom<u128> + Into<U256>,
{
	fn call(&self, request: CallRequest, at: Option<<B as BlockT>::Hash>) -> Result<Bytes> {
		self.deny_unsafe.check_if_safe()?;

		let hash = at.unwrap_or_else(|| self.client.info().best_hash);

		let CallRequest {
			from,
			to,
			gas_limit,
			storage_limit,
			value,
			data,
		} = request;

		let gas_limit = gas_limit.unwrap_or_else(u64::max_value); // TODO: set a limit
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
			None => Err(Error {
				code: ErrorCode::InternalError,
				message: "Not supported".into(),
				data: None,
			}),
		}
	}

	fn estimate_resources(
		&self,
		from: H160,
		unsigned_extrinsic: Bytes,
		at: Option<<B as BlockT>::Hash>,
	) -> Result<EstimateResourcesResponse> {
		self.deny_unsafe.check_if_safe()?;

		let hash = at.unwrap_or_else(|| self.client.info().best_hash);
		let request = self
			.client
			.runtime_api()
			.get_estimate_resources_request(&BlockId::Hash(hash), unsigned_extrinsic.to_vec())
			.map_err(|err| internal_err(format!("runtime error: {:?}", err)))?
			.map_err(|err| internal_err(format!("execution fatal: {:?}", err)))?;

		// Determine the highest possible gas limits
		let max_gas_limit = u64::max_value(); // TODO: set a limit
		let mut highest = U256::from(request.gas_limit.unwrap_or(max_gas_limit));

		let request = CallRequest {
			from: Some(from),
			to: request.to,
			gas_limit: request.gas_limit,
			storage_limit: request.storage_limit,
			value: request.value.map(|v| NumberOrHex::Hex(U256::from(v))),
			data: request.data.map(Bytes),
		};

		log::debug!(
			target: "evm",
			"estimate_resources, from: {:?}, to: {:?}, gas_limit: {:?}, storage_limit: {:?}, value: {:?}, at_hash: {:?}",
			request.from, request.to, request.gas_limit, request.storage_limit, request.value, hash
		);

		struct ExecutableResult {
			data: Vec<u8>,
			exit_reason: ExitReason,
			used_gas: U256,
			used_storage: i32,
		}

		// Create a helper to check if a gas allowance results in an executable transaction
		let executable = move |request: CallRequest, gas| -> Result<ExecutableResult> {
			let CallRequest {
				from,
				to,
				gas_limit,
				storage_limit,
				value,
				data,
			} = request;

			// Use request gas limit only if it less than gas_limit parameter
			let gas_limit = core::cmp::min(gas_limit.unwrap_or(gas), gas);
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

			let (exit_reason, data, used_gas, used_storage) = match to {
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

					(info.exit_reason, info.value, info.used_gas, info.used_storage)
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

					(info.exit_reason, Vec::new(), info.used_gas, info.used_storage)
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
		} = executable(request.clone(), highest.as_u64())?;
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
					let ExecutableResult { data, exit_reason, .. } = executable(request.clone(), max_gas_limit)?;
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
			const MIN_GAS_PER_TX: U256 = U256([21_000, 0, 0, 0]);
			let mut lowest = MIN_GAS_PER_TX;

			// Start close to the used gas for faster binary search
			let mut mid = std::cmp::min(used_gas * 3, (highest + lowest) / 2);

			// Execute the binary search and hone in on an executable gas limit.
			let mut previous_highest = highest;
			while (highest - lowest) > U256::one() {
				let ExecutableResult { data, exit_reason, .. } = executable(request.clone(), mid.as_u64())?;
				match exit_reason {
					ExitReason::Succeed(_) => {
						highest = mid;
						// If the variation in the estimate is less than 10%,
						// then the estimate is considered sufficiently accurate.
						if (previous_highest - highest) * 10 / previous_highest < U256::one() {
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

		let uxt: <B as traits::Block>::Extrinsic = Decode::decode(&mut &*unsigned_extrinsic).map_err(|e| Error {
			code: ErrorCode::InternalError,
			message: "Unable to dry run extrinsic.".into(),
			data: Some(format!("{:?}", e).into()),
		})?;

		let fee = self
			.client
			.runtime_api()
			.query_fee_details(&BlockId::Hash(hash), uxt, unsigned_extrinsic.len() as u32)
			.map_err(|e| Error {
				code: ErrorCode::InternalError,
				message: "Unable to query fee details.".into(),
				data: Some(format!("{:?}", e).into()),
			})?;

		let adjusted_weight_fee = fee
			.inclusion_fee
			.map_or_else(Default::default, |inclusion| inclusion.adjusted_weight_fee);

		Ok(EstimateResourcesResponse {
			gas: highest,
			storage: used_storage,
			weight_fee: adjusted_weight_fee.into(),
		})
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
