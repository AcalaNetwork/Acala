// This file is part of Acala.

// Copyright (C) 2020-2024 Acala Foundation.
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

use module_evm_utility::{
	evm::{Context, CreateScheme, ExitError, ExitFatal, ExitReason, ExitSucceed, Opcode, Transfer},
	evm_gasometer, evm_runtime,
};
use sp_core::{H160, H256, U256};
use sp_std::prelude::*;

pub use primitives::evm::tracing::{CallTrace, CallType, Step, VMTrace};

#[derive(Debug, Copy, Clone)]
pub enum Event<'a> {
	Call {
		code_address: H160,
		transfer: &'a Option<Transfer>,
		input: &'a [u8],
		target_gas: Option<u64>,
		is_static: bool,
		context: &'a Context,
	},
	Create {
		caller: H160,
		address: H160,
		scheme: CreateScheme,
		value: U256,
		init_code: &'a [u8],
		target_gas: Option<u64>,
	},
	Suicide {
		address: H160,
		target: H160,
		balance: U256,
	},
	Exit {
		reason: &'a ExitReason,
		return_value: &'a [u8],
	},
	TransactCall {
		caller: H160,
		address: H160,
		value: U256,
		data: &'a [u8],
		gas_limit: u64,
	},
	TransactCreate {
		caller: H160,
		value: U256,
		init_code: &'a [u8],
		gas_limit: u64,
		address: H160,
	},
	TransactCreate2 {
		caller: H160,
		value: U256,
		init_code: &'a [u8],
		salt: H256,
		gas_limit: u64,
		address: H160,
	},
	PrecompileSubcall {
		code_address: H160,
		transfer: &'a Option<Transfer>,
		input: &'a [u8],
		target_gas: Option<u64>,
		is_static: bool,
		context: &'a Context,
	},
	Enter {
		depth: u32,
	},
}

pub struct Tracer {
	vm: bool,
	events: Vec<CallTrace>,
	stack: Vec<CallTrace>,
	steps: Vec<Step>,
	opcode: Option<Opcode>,
	snapshot: Option<evm_gasometer::Snapshot>,
}

impl Tracer {
	pub fn new(vm: bool) -> Self {
		Self {
			vm,
			events: Vec::new(),
			stack: Vec::new(),
			steps: Vec::new(),
			opcode: None,
			snapshot: None,
		}
	}

	pub fn finalize(&mut self) -> Vec<CallTrace> {
		self.events.drain(..).rev().collect()
	}

	pub fn steps(&self) -> Vec<Step> {
		self.steps.clone()
	}

	fn call_type(&self) -> CallType {
		match self.opcode {
			Some(Opcode::CALLCODE) => CallType::CALLCODE,
			Some(Opcode::DELEGATECALL) => CallType::DELEGATECALL,
			Some(Opcode::STATICCALL) => CallType::STATICCALL,
			Some(Opcode::CREATE) | Some(Opcode::CREATE2) => CallType::CREATE,
			Some(Opcode::SUICIDE) => CallType::SUICIDE,
			_ => CallType::CALL,
		}
	}

	fn evm_runtime_event(&mut self, event: evm_runtime::tracing::Event) {
		match event {
			evm_runtime::tracing::Event::Step {
				context: _,
				opcode,
				position,
				stack,
				memory,
			} => {
				self.opcode = Some(opcode);
				if !self.vm {
					return;
				}
				self.steps.push(Step {
					op: opcode,
					pc: position.as_ref().map_or(0, |pc| *pc as u32),
					depth: self.stack.last().map_or(0, |x| x.depth),
					gas: self.snapshot.map_or(0, |s| s.gas()),
					stack: stack
						.data()
						.iter()
						.map(|x| {
							let slice = x.as_fixed_bytes();
							// trim leading zeros
							let start = slice.iter().position(|x| *x != 0).unwrap_or(31);
							slice[start..].to_vec()
						})
						.collect(),
					memory: if memory.is_empty() {
						None
					} else {
						let chunks = memory.data().chunks(32);
						let size = chunks.len();
						let mut slices: Vec<Vec<u8>> = Vec::with_capacity(size);
						for (idx, chunk) in chunks.enumerate() {
							if idx + 1 == size {
								// last chunk must not be trimmed because it can be less than 32 bytes
								slices.push(chunk.to_vec());
							} else {
								// trim leading zeros
								if let Some(start) = chunk.iter().position(|x| *x != 0) {
									slices.push(chunk[start..].to_vec());
								} else {
									slices.push(Vec::from([0u8]));
								}
							}
						}
						Some(slices)
					},
				})
			}
			_ => {}
		};
	}

	fn evm_gasometer_event(&mut self, event: evm_gasometer::tracing::Event) {
		match event {
			evm_gasometer::tracing::Event::RecordCost { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordDynamicCost { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordTransaction { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordRefund { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordStipend { snapshot, .. } => self.snapshot = snapshot,
		};
	}
}

pub trait EventListener {
	fn event(&mut self, event: Event);
}

impl EventListener for Tracer {
	fn event(&mut self, event: Event) {
		match event {
			Event::Call {
				code_address,
				transfer,
				input,
				target_gas,
				is_static,
				context,
			}
			| Event::PrecompileSubcall {
				code_address,
				transfer,
				input,
				target_gas,
				is_static,
				context,
			} => {
				let call_type = if is_static {
					CallType::STATICCALL
				} else {
					self.call_type()
				};
				self.stack.push(CallTrace {
					call_type,
					from: context.caller,
					to: code_address,
					input: input.to_vec(),
					value: transfer.clone().map(|x| x.value).unwrap_or_default(),
					gas: target_gas.unwrap_or_default(),
					gas_used: 0,
					output: None,
					error: None,
					revert_reason: None,
					calls: Vec::new(),
					depth: 0,
				});
			}
			Event::TransactCall {
				caller,
				address,
				value,
				data,
				gas_limit,
			} => {
				self.stack.push(CallTrace {
					call_type: CallType::CALL,
					from: caller,
					to: address,
					input: data.to_vec(),
					value,
					gas: gas_limit,
					gas_used: 0,
					output: None,
					error: None,
					revert_reason: None,
					calls: Vec::new(),
					depth: 0,
				});
			}
			Event::Create {
				caller,
				address,
				value,
				init_code,
				target_gas,
				..
			} => {
				self.stack.push(CallTrace {
					call_type: CallType::CREATE,
					from: caller,
					to: address,
					input: init_code.to_vec(),
					value,
					gas: target_gas.unwrap_or_default(),
					gas_used: 0,
					output: None,
					error: None,
					revert_reason: None,
					calls: Vec::new(),
					depth: 0,
				});
			}
			Event::TransactCreate {
				caller,
				value,
				init_code,
				gas_limit,
				address,
			}
			| Event::TransactCreate2 {
				caller,
				value,
				init_code,
				gas_limit,
				address,
				..
			} => {
				self.stack.push(CallTrace {
					call_type: CallType::CREATE,
					from: caller,
					to: address,
					input: init_code.to_vec(),
					value,
					gas: gas_limit,
					gas_used: 0,
					output: None,
					error: None,
					revert_reason: None,
					calls: Vec::new(),
					depth: 0,
				});
			}
			Event::Suicide {
				address,
				target,
				balance,
			} => {
				self.stack.push(CallTrace {
					call_type: CallType::SUICIDE,
					from: address,
					to: target,
					input: vec![],
					value: balance,
					gas: 0,
					gas_used: 0,
					output: None,
					error: None,
					revert_reason: None,
					calls: Vec::new(),
					depth: 0,
				});
			}
			Event::Exit { reason, return_value } => {
				if let Some(mut trace) = self.stack.pop() {
					match reason {
						ExitReason::Succeed(ExitSucceed::Returned) => trace.output = Some(return_value.to_vec()),
						ExitReason::Succeed(_) => {}
						ExitReason::Error(e) => trace.error = Some(e.stringify().as_bytes().to_vec()),
						ExitReason::Revert(_) => trace.revert_reason = Some(return_value.to_vec()),
						ExitReason::Fatal(e) => trace.error = Some(e.stringify().as_bytes().to_vec()),
					}

					if let Some(snapshot) = self.snapshot {
						trace.gas_used = trace.gas.saturating_sub(snapshot.gas());
					}

					if let Some(index) = self.events.iter().position(|x| x.depth > trace.depth) {
						trace.calls = self.events.drain(index..).collect();
					}

					self.events.push(trace);
				}
			}
			Event::Enter { depth } => {
				if let Some(event) = self.stack.last_mut() {
					event.depth = depth;
				}
			}
		}
	}
}

pub struct EvmRuntimeTracer;

impl evm_runtime::tracing::EventListener for EvmRuntimeTracer {
	fn event(&mut self, event: evm_runtime::tracing::Event) {
		tracer::with(|tracer| {
			tracer.evm_runtime_event(event);
		});
	}
}

pub struct EvmGasometerTracer;

impl evm_gasometer::tracing::EventListener for EvmGasometerTracer {
	fn event(&mut self, event: evm_gasometer::tracing::Event) {
		tracer::with(|tracer| {
			tracer.evm_gasometer_event(event);
		});
	}
}

environmental::environmental!(tracer: Tracer);

pub fn using<R, F: FnOnce() -> R>(new: &mut Tracer, f: F) -> R {
	tracer::using(new, || {
		evm_gasometer::tracing::using(&mut EvmGasometerTracer, || {
			evm_runtime::tracing::using(&mut EvmRuntimeTracer, f)
		})
	})
}

pub(crate) fn with<F: FnOnce(&mut Tracer)>(f: F) {
	tracer::with(f);
}

trait Stringify {
	fn stringify(&self) -> &str;
}

impl Stringify for ExitError {
	fn stringify(&self) -> &str {
		match self {
			ExitError::StackUnderflow => "StackUnderflow",
			ExitError::StackOverflow => "StackOverflow",
			ExitError::InvalidJump => "InvalidJump",
			ExitError::InvalidRange => "InvalidRange",
			ExitError::DesignatedInvalid => "DesignatedInvalid",
			ExitError::CallTooDeep => "CallTooDeep",
			ExitError::CreateCollision => "CreateCollision",
			ExitError::CreateContractLimit => "CreateContractLimit",
			ExitError::InvalidCode(_) => "InvalidCode",
			ExitError::OutOfOffset => "OutOfOffset",
			ExitError::OutOfGas => "OutOfGas",
			ExitError::OutOfFund => "OutOfFund",
			ExitError::PCUnderflow => "PCUnderflow",
			ExitError::CreateEmpty => "CreateEmpty",
			ExitError::Other(msg) => msg,
			ExitError::MaxNonce => "MaxNonce",
		}
	}
}

impl Stringify for ExitFatal {
	fn stringify(&self) -> &str {
		match self {
			ExitFatal::NotSupported => "NotSupported",
			ExitFatal::UnhandledInterrupt => "UnhandledInterrupt",
			ExitFatal::CallErrorAsFatal(e) => e.stringify(),
			ExitFatal::Other(msg) => msg,
		}
	}
}
