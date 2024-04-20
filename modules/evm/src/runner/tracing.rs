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
	evm::{Context, ExitError, ExitFatal, ExitReason, ExitSucceed, Opcode, Transfer},
	evm_gasometer, evm_runtime,
};
use sp_core::{H160, H256, U256};
use sp_std::prelude::*;

pub use primitives::evm::tracing::{CallTrace, CallType, LogTrace, OpcodeConfig, Step, TraceOutcome, TracerConfig};

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
	Log {
		address: H160,
		topics: &'a Vec<H256>,
		data: &'a Vec<u8>,
	},
}

pub struct Tracer {
	config: TracerConfig,
	calls: Vec<CallTrace>,
	stack: Vec<CallTrace>,
	depth: u32,
	steps: Vec<Step>,
	step_counter: u32,
	gas: u64,
	current_opcode: Option<Opcode>,
}

impl Tracer {
	pub fn new(config: TracerConfig) -> Self {
		Self {
			config,
			calls: Vec::new(),
			stack: Vec::new(),
			depth: 0,
			steps: Vec::new(),
			step_counter: 0,
			gas: 0,
			current_opcode: None,
		}
	}

	#[inline]
	fn trace_memory(&self) -> bool {
		matches!(
			self.config,
			TracerConfig::OpcodeTracer(OpcodeConfig {
				enable_memory: true,
				..
			})
		)
	}

	#[inline]
	fn trace_stack(&self) -> bool {
		matches!(
			self.config,
			TracerConfig::OpcodeTracer(OpcodeConfig {
				disable_stack: false,
				..
			})
		)
	}

	#[inline]
	fn trace_call(&self) -> bool {
		matches!(self.config, TracerConfig::CallTracer)
	}

	// increment step counter and check if we should record this step
	#[inline]
	fn count_step(&mut self) -> bool {
		self.step_counter += 1;
		if let TracerConfig::OpcodeTracer(OpcodeConfig { page, page_size, .. }) = self.config {
			if self.step_counter > page * page_size && self.step_counter <= (page + 1) * page_size {
				return true;
			}
		}
		false
	}

	pub fn finalize(&mut self) -> TraceOutcome {
		match self.config {
			TracerConfig::CallTracer => {
				assert!(self.stack.is_empty(), "Call stack is not empty");
				TraceOutcome::Calls(self.calls.drain(..).collect())
			}
			TracerConfig::OpcodeTracer(_) => TraceOutcome::Steps(self.steps.drain(..).collect()),
		}
	}

	#[inline]
	fn evm_runtime_event(&mut self, event: evm_runtime::tracing::Event) {
		match event {
			evm_runtime::tracing::Event::Step {
				context: _,
				opcode,
				position,
				stack,
				memory,
			} => {
				self.current_opcode = Some(opcode);
				if self.count_step() {
					self.steps.push(Step {
						op: opcode,
						pc: position.as_ref().map_or(0, |pc| *pc as u32),
						depth: self.depth,
						gas: 0,
						stack: if self.trace_stack() {
							stack
								.data()
								.iter()
								.map(|x| {
									let slice = x.as_fixed_bytes();
									// trim leading zeros
									let start = slice.iter().position(|x| *x != 0).unwrap_or(31);
									slice[start..].to_vec()
								})
								.collect()
						} else {
							Vec::new()
						},
						memory: if memory.is_empty() || !self.trace_memory() {
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
			}
			evm_runtime::tracing::Event::StepResult { .. } => {
				if let Some(step) = self.steps.last_mut() {
					step.gas = self.gas;
				}
			}
			evm_runtime::tracing::Event::SLoad { address, index, value } => {
				if !self.trace_call() {
					return;
				}
				let trace = self.stack.last_mut().expect("missing call trace");
				trace.logs.push(LogTrace::SLoad { address, index, value });
			}
			evm_runtime::tracing::Event::SStore { address, index, value } => {
				if !self.trace_call() {
					return;
				}
				let trace = self.stack.last_mut().expect("missing call trace");
				trace.logs.push(LogTrace::SStore { address, index, value });
			}
		};
	}

	#[inline]
	fn evm_gasometer_event(&mut self, event: evm_gasometer::tracing::Event) {
		match event {
			evm_gasometer::tracing::Event::RecordCost { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordDynamicCost { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordTransaction { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordRefund { snapshot, .. }
			| evm_gasometer::tracing::Event::RecordStipend { snapshot, .. } => {
				self.gas = snapshot.map_or(0, |s| s.gas());
			}
		};
	}

	#[inline]
	fn call_event(&mut self, event: Event) {
		match event {
			Event::Call {
				code_address,
				transfer,
				input,
				target_gas,
				context,
				..
			}
			| Event::PrecompileSubcall {
				code_address,
				transfer,
				input,
				target_gas,
				context,
				..
			} => {
				self.stack.push(CallTrace {
					call_type: self.current_opcode.map_or(CallType::CALL, |x| x.into()),
					from: context.caller,
					to: code_address,
					input: input.to_vec(),
					value: transfer.as_ref().map_or(U256::zero(), |x| x.value),
					gas: target_gas.unwrap_or(self.gas),
					..Default::default()
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
					..Default::default()
				});
			}
			Event::Create {
				caller,
				address,
				value,
				init_code,
				target_gas,
			} => {
				self.stack.push(CallTrace {
					call_type: CallType::CREATE,
					from: caller,
					to: address,
					input: init_code.to_vec(),
					value,
					gas: target_gas.unwrap_or(self.gas),
					..Default::default()
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
					..Default::default()
				});
			}
			Event::Suicide {
				address,
				target,
				balance,
			} => {
				let trace = self.stack.last_mut().expect("missing call trace");
				trace.calls.push(CallTrace {
					call_type: CallType::SUICIDE,
					from: address,
					to: target,
					value: balance,
					..Default::default()
				});
			}
			Event::Exit { reason, return_value } => {
				let mut trace = self.stack.pop().expect("missing call trace");
				match reason {
					ExitReason::Succeed(ExitSucceed::Returned) => {
						if !return_value.is_empty() {
							trace.output = Some(return_value.to_vec());
						}
					}
					ExitReason::Succeed(_) => {}
					ExitReason::Error(e) => trace.error = Some(e.stringify().as_bytes().to_vec()),
					ExitReason::Revert(_) => {
						if !return_value.is_empty() {
							trace.revert_reason = Some(return_value.to_vec());
						}
					}
					ExitReason::Fatal(e) => trace.error = Some(e.stringify().as_bytes().to_vec()),
				}

				trace.gas_used = trace.gas.saturating_sub(self.gas);

				if let Some(index) = self.calls.iter().position(|x| x.depth > trace.depth) {
					let mut subcalls = self.calls.drain(index..).collect::<Vec<_>>();
					if matches!(reason, ExitReason::Succeed(ExitSucceed::Suicided)) {
						let mut suicide_call = trace.calls.pop().expect("suicide call should be injected");
						suicide_call.depth = trace.depth + 1;
						subcalls.push(suicide_call);
					}
					trace.calls = subcalls;
				}

				self.calls.push(trace);
			}
			Event::Enter { depth } => {
				let trace = self.stack.last_mut().expect("missing call trace");
				trace.depth = depth;
			}
			Event::Log { address, topics, data } => {
				let trace = self.stack.last_mut().expect("missing call trace");
				trace.logs.push(LogTrace::Log {
					address,
					topics: topics.clone(),
					data: data.clone(),
				});
			}
		}
	}
}

pub trait EventListener {
	fn event(&mut self, event: Event);
}

impl EventListener for Tracer {
	fn event(&mut self, event: Event) {
		if self.trace_call() {
			self.call_event(event);
		} else {
			match event {
				Event::Exit { reason, .. } => {
					if !matches!(reason, ExitReason::Succeed(ExitSucceed::Suicided)) {
						self.depth = self.depth.saturating_sub(1);
					}
				}
				Event::Enter { depth } => {
					self.depth = depth;
				}
				_ => {}
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
