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

		let request = CallRequest {
			from: Some(from),
			to: request.to,
			gas_limit: request.gas_limit,
			storage_limit: request.storage_limit,
			value: request.value.map(|v| NumberOrHex::Hex(U256::from(v))),
			data: request.data.map(Bytes),
		};

		let calculate_gas_used = |request| -> Result<(U256, i32)> {
			let hash = self.client.info().best_hash;

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

			let (used_gas, used_storage) = match to {
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

					(info.used_gas, info.used_storage)
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

					(info.used_gas, info.used_storage)
				}
			};

			Ok((used_gas, used_storage))
		};

		if cfg!(feature = "rpc_binary_search_estimate") {
			let mut lower = U256::from(21_000);
			// TODO: get a good upper limit, but below U64::max to operation overflow
			let mut upper = U256::from(1_000_000_000);
			let mut mid = upper;
			let mut best = mid;
			let mut old_best: U256;
			let mut storage: i32 = Default::default();

			// if the gas estimation depends on the gas limit, then we want to binary
			// search until the change is under some threshold. but if not dependent,
			// we want to stop immediately.
			let mut change_pct = U256::from(100);
			let threshold_pct = U256::from(10);

			// invariant: lower <= mid <= upper
			while change_pct > threshold_pct {
				let mut test_request = request.clone();
				test_request.gas_limit = Some(mid.as_u64());
				match calculate_gas_used(test_request) {
					// if Ok -- try to reduce the gas used
					Ok((used_gas, used_storage)) => {
						log::debug!(
							target: "evm",
							"calculate_gas_used ok, used_gas: {:?}, used_storage: {:?}",
							used_gas, used_storage,
						);

						old_best = best;
						best = used_gas;
						change_pct = (U256::from(100) * (old_best - best))
							.checked_div(old_best)
							.unwrap_or_default();
						upper = mid;
						mid = (lower + upper + 1) / 2;
						storage = used_storage;
					}

					Err(err) => {
						log::debug!(
							target: "evm",
							"calculate_gas_used err, lower: {:?}, upper: {:?}, mid: {:?}",
							lower, upper, mid
						);

						// if Err == OutofGas, we need more gas
						if err.code == ErrorCode::ServerError(0) {
							lower = mid;
							mid = (lower + upper + 1) / 2;
							if mid == lower {
								break;
							}
						} else {
							// Other errors, return directly
							return Err(err);
						}
					}
				}
			}

			let uxt: <B as traits::Block>::Extrinsic =
				Decode::decode(&mut &*unsigned_extrinsic).map_err(|e| Error {
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
				gas: best,
				storage,
				weight_fee: adjusted_weight_fee.into(),
			})
		} else {
			let (used_gas, used_storage) = calculate_gas_used(request)?;

			let uxt: <B as traits::Block>::Extrinsic =
				Decode::decode(&mut &*unsigned_extrinsic).map_err(|e| Error {
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
				gas: used_gas,
				storage: used_storage,
				weight_fee: adjusted_weight_fee.into(),
			})
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
