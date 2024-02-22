// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
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

pub use primitives::evm::tracing::{CallTrace, CallType, Step};

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
	Enter {
		depth: u32,
	},
}

pub struct CallTracer {
	events: Vec<CallTrace>,
	stack: Vec<CallTrace>,
	opcode: Option<Opcode>,
	snapshot: Option<evm_gasometer::Snapshot>,
}

impl CallTracer {
	pub fn new() -> Self {
		Self {
			events: Vec::new(),
			stack: Vec::new(),
			opcode: None,
			snapshot: None,
		}
	}

	pub fn finalize(&mut self) -> Vec<CallTrace> {
		self.events.drain(..).rev().collect()
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
			evm_runtime::tracing::Event::Step { opcode, .. } => {
				self.opcode = Some(opcode);
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

pub struct OpcodeTracer {
	pub steps: Vec<Step>,
}

impl OpcodeTracer {
	pub fn new() -> Self {
		Self { steps: Vec::new() }
	}
}

impl evm_runtime::tracing::EventListener for OpcodeTracer {
	fn event(&mut self, event: evm_runtime::tracing::Event) {
		match event {
			evm_runtime::tracing::Event::Step {
				context: _,
				opcode,
				position,
				stack,
				memory,
			} => self.steps.push(Step {
				op: opcode.stringify().as_bytes().to_vec(),
				pc: position.clone().unwrap_or_default() as u64,
				stack: stack.data().clone(),
				memory: memory.data().clone(),
			}),
			_ => {}
		}
	}
}

pub trait EventListener {
	fn event(&mut self, event: Event);
}

impl EventListener for CallTracer {
	fn event(&mut self, event: Event) {
		match event {
			Event::Call {
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
		call_tracer::with(|tracer| {
			tracer.evm_runtime_event(event);
		});
	}
}

pub struct EvmGasometerTracer;

impl evm_gasometer::tracing::EventListener for EvmGasometerTracer {
	fn event(&mut self, event: evm_gasometer::tracing::Event) {
		call_tracer::with(|tracer| {
			tracer.evm_gasometer_event(event);
		});
	}
}

environmental::environmental!(call_tracer: CallTracer);

pub fn call_tracer_using<R, F: FnOnce() -> R>(new: &mut CallTracer, f: F) -> R {
	call_tracer::using(new, || {
		evm_gasometer::tracing::using(&mut EvmGasometerTracer, || {
			evm_runtime::tracing::using(&mut EvmRuntimeTracer, f)
		})
	})
}

pub(crate) fn call_tracer_with<F: FnOnce(&mut CallTracer)>(f: F) {
	call_tracer::with(f);
}

pub fn opcode_tracer_using<R, F: FnOnce() -> R>(new: &mut OpcodeTracer, f: F) -> R {
	evm_runtime::tracing::using(new, f)
}

trait Stringify {
	fn stringify(&self) -> &str;
}

impl Stringify for Opcode {
	fn stringify(&self) -> &str {
		match self {
			&Opcode::STOP => "STOP",
			&Opcode::ADD => "ADD",
			&Opcode::MUL => "MUL",
			&Opcode::SUB => "SUB",
			&Opcode::DIV => "DIV",
			&Opcode::SDIV => "SDIV",
			&Opcode::MOD => "MOD",
			&Opcode::SMOD => "SMOD",
			&Opcode::ADDMOD => "ADDMOD",
			&Opcode::MULMOD => "MULMOD",
			&Opcode::EXP => "EXP",
			&Opcode::SIGNEXTEND => "SIGNEXTEND",
			&Opcode::LT => "LT",
			&Opcode::GT => "GT",
			&Opcode::SLT => "SLT",
			&Opcode::SGT => "SGT",
			&Opcode::EQ => "EQ",
			&Opcode::ISZERO => "ISZERO",
			&Opcode::AND => "AND",
			&Opcode::OR => "OR",
			&Opcode::XOR => "XOR",
			&Opcode::NOT => "NOT",
			&Opcode::BYTE => "BYTE",
			&Opcode::SHL => "SHL",
			&Opcode::SHR => "SHR",
			&Opcode::SAR => "SAR",
			&Opcode::SHA3 => "SHA3",
			&Opcode::ADDRESS => "ADDRESS",
			&Opcode::BALANCE => "BALANCE",
			&Opcode::ORIGIN => "ORIGIN",
			&Opcode::CALLER => "CALLER",
			&Opcode::CALLVALUE => "CALLVALUE",
			&Opcode::CALLDATALOAD => "CALLDATALOAD",
			&Opcode::CALLDATASIZE => "CALLDATASIZE",
			&Opcode::CALLDATACOPY => "CALLDATACOPY",
			&Opcode::CODESIZE => "CODESIZE",
			&Opcode::CODECOPY => "CODECOPY",
			&Opcode::GASPRICE => "GASPRICE",
			&Opcode::EXTCODESIZE => "EXTCODESIZE",
			&Opcode::EXTCODECOPY => "EXTCODECOPY",
			&Opcode::RETURNDATASIZE => "RETURNDATASIZE",
			&Opcode::RETURNDATACOPY => "RETURNDATACOPY",
			&Opcode::EXTCODEHASH => "EXTCODEHASH",
			&Opcode::BLOCKHASH => "BLOCKHASH",
			&Opcode::COINBASE => "COINBASE",
			&Opcode::TIMESTAMP => "TIMESTAMP",
			&Opcode::NUMBER => "NUMBER",
			&Opcode::DIFFICULTY => "DIFFICULTY",
			&Opcode::GASLIMIT => "GASLIMIT",
			&Opcode::CHAINID => "CHAINID",
			&Opcode::SELFBALANCE => "SELFBALANCE",
			&Opcode::POP => "POP",
			&Opcode::MLOAD => "MLOAD",
			&Opcode::MSTORE => "MSTORE",
			&Opcode::MSTORE8 => "MSTORE8",
			&Opcode::SLOAD => "SLOAD",
			&Opcode::SSTORE => "SSTORE",
			&Opcode::JUMP => "JUMP",
			&Opcode::JUMPI => "JUMPI",
			&Opcode::PC => "PC",
			&Opcode::MSIZE => "MSIZE",
			&Opcode::GAS => "GAS",
			&Opcode::JUMPDEST => "JUMPDEST",
			&Opcode::PUSH1 => "PUSH1",
			&Opcode::PUSH2 => "PUSH2",
			&Opcode::PUSH3 => "PUSH3",
			&Opcode::PUSH4 => "PUSH4",
			&Opcode::PUSH5 => "PUSH5",
			&Opcode::PUSH6 => "PUSH6",
			&Opcode::PUSH7 => "PUSH7",
			&Opcode::PUSH8 => "PUSH8",
			&Opcode::PUSH9 => "PUSH9",
			&Opcode::PUSH10 => "PUSH10",
			&Opcode::PUSH11 => "PUSH11",
			&Opcode::PUSH12 => "PUSH12",
			&Opcode::PUSH13 => "PUSH13",
			&Opcode::PUSH14 => "PUSH14",
			&Opcode::PUSH15 => "PUSH15",
			&Opcode::PUSH16 => "PUSH16",
			&Opcode::PUSH17 => "PUSH17",
			&Opcode::PUSH18 => "PUSH18",
			&Opcode::PUSH19 => "PUSH19",
			&Opcode::PUSH20 => "PUSH20",
			&Opcode::PUSH21 => "PUSH21",
			&Opcode::PUSH22 => "PUSH22",
			&Opcode::PUSH23 => "PUSH23",
			&Opcode::PUSH24 => "PUSH24",
			&Opcode::PUSH25 => "PUSH25",
			&Opcode::PUSH26 => "PUSH26",
			&Opcode::PUSH27 => "PUSH27",
			&Opcode::PUSH28 => "PUSH28",
			&Opcode::PUSH29 => "PUSH29",
			&Opcode::PUSH30 => "PUSH30",
			&Opcode::PUSH31 => "PUSH31",
			&Opcode::PUSH32 => "PUSH32",
			&Opcode::DUP1 => "DUP1",
			&Opcode::DUP2 => "DUP2",
			&Opcode::DUP3 => "DUP3",
			&Opcode::DUP4 => "DUP4",
			&Opcode::DUP5 => "DUP5",
			&Opcode::DUP6 => "DUP6",
			&Opcode::DUP7 => "DUP7",
			&Opcode::DUP8 => "DUP8",
			&Opcode::DUP9 => "DUP9",
			&Opcode::DUP10 => "DUP10",
			&Opcode::DUP11 => "DUP11",
			&Opcode::DUP12 => "DUP12",
			&Opcode::DUP13 => "DUP13",
			&Opcode::DUP14 => "DUP14",
			&Opcode::DUP15 => "DUP15",
			&Opcode::DUP16 => "DUP16",
			&Opcode::SWAP1 => "SWAP1",
			&Opcode::SWAP2 => "SWAP2",
			&Opcode::SWAP3 => "SWAP3",
			&Opcode::SWAP4 => "SWAP4",
			&Opcode::SWAP5 => "SWAP5",
			&Opcode::SWAP6 => "SWAP6",
			&Opcode::SWAP7 => "SWAP7",
			&Opcode::SWAP8 => "SWAP8",
			&Opcode::SWAP9 => "SWAP9",
			&Opcode::SWAP10 => "SWAP10",
			&Opcode::SWAP11 => "SWAP11",
			&Opcode::SWAP12 => "SWAP12",
			&Opcode::SWAP13 => "SWAP13",
			&Opcode::SWAP14 => "SWAP14",
			&Opcode::SWAP15 => "SWAP15",
			&Opcode::SWAP16 => "SWAP16",
			&Opcode::LOG0 => "LOG0",
			&Opcode::LOG1 => "LOG1",
			&Opcode::LOG2 => "LOG2",
			&Opcode::LOG3 => "LOG3",
			&Opcode::LOG4 => "LOG4",
			&Opcode::CREATE => "CREATE",
			&Opcode::CALL => "CALL",
			&Opcode::CALLCODE => "CALLCODE",
			&Opcode::RETURN => "RETURN",
			&Opcode::DELEGATECALL => "DELEGATECALL",
			&Opcode::STATICCALL => "STATICCALL",
			&Opcode::REVERT => "REVERT",
			&Opcode::INVALID => "INVALID",
			&Opcode::CREATE2 => "CREATE2",
			&Opcode::EOFMAGIC => "EOFMAGIC",
			&Opcode::SUICIDE => "SUICIDE",
			_ => "UNKNOWN",
		}
	}
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
