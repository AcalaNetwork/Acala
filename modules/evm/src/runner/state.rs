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

// Synchronize with https://github.com/rust-blockchain/evm/blob/master/src/executor/stack/mod.rs

use crate::{encode_revert_message, runner::StackState, StorageMeter};
use core::{cmp::min, convert::Infallible};
use frame_support::log;
use module_evm_utiltity::{
	ethereum::Log,
	evm::{
		Capture, Config, Context, CreateScheme, ExitError, ExitReason, ExitRevert, ExitSucceed, Opcode, Runtime, Stack,
		Transfer,
	},
	evm_gasometer::{self as gasometer, Gasometer},
	evm_runtime::Handler,
};
use primitive_types::{H160, H256, U256};
pub use primitives::{
	evm::{EvmAddress, Vicinity},
	ReserveIdentifier, H160_PREFIX_DEXSHARE, H160_PREFIX_TOKEN, MIRRORED_NFT_ADDRESS_START, PREDEPLOY_ADDRESS_START,
	SYSTEM_CONTRACT_ADDRESS_PREFIX,
};
use sha3::{Digest, Keccak256};
use sp_std::{rc::Rc, vec::Vec};

macro_rules! event {
	($x:expr) => {};
}

#[cfg(feature = "tracing")]
mod tracing {
	pub struct Tracer;
	impl module_evm_utiltity::evm_runtime::tracing::EventListener for Tracer {
		fn event(&mut self, event: module_evm_utiltity::evm_runtime::tracing::Event) {
			frame_support::log::debug!(
				target: "evm", "evm_runtime tracing: {:?}", event
			);
		}
	}
}

#[cfg(feature = "tracing")]
use tracing::*;

pub enum StackExitKind {
	Succeeded,
	Reverted,
	Failed,
}

pub struct StackSubstateMetadata<'config> {
	gasometer: Gasometer<'config>,
	storage_meter: StorageMeter,
	is_static: bool,
	depth: Option<usize>,
	// save the caller which called `inner_create` to get maintainer
	caller: Option<H160>,
	// save the contract to charge storage
	target: Option<H160>,
}

impl<'config> StackSubstateMetadata<'config> {
	pub fn new(gas_limit: u64, storage_limit: u32, extra_bytes: u32, config: &'config Config) -> Self {
		Self {
			gasometer: Gasometer::new(gas_limit, config),
			storage_meter: StorageMeter::new(storage_limit, extra_bytes),
			is_static: false,
			depth: None,
			caller: None,
			target: None,
		}
	}

	pub fn swallow_commit(&mut self, other: Self) -> Result<(), ExitError> {
		self.gasometer_mut().record_stipend(other.gasometer().gas())?;
		self.gasometer.record_refund(other.gasometer().refunded_gas())?;

		// merge child meter into parent meter
		self.storage_meter.merge(other.storage_meter());

		Ok(())
	}

	pub fn swallow_revert(&mut self, other: Self) -> Result<(), ExitError> {
		self.gasometer_mut().record_stipend(other.gasometer().gas())?;

		Ok(())
	}

	pub fn swallow_discard(&mut self, _other: Self) -> Result<(), ExitError> {
		Ok(())
	}

	pub fn spit_child(&self, gas_limit: u64, is_static: bool) -> Self {
		Self {
			gasometer: Gasometer::new(gas_limit, self.gasometer().config()),
			storage_meter: StorageMeter::new(
				self.storage_meter().available_storage(),
				self.storage_meter().extra_bytes(),
			),
			is_static: is_static || self.is_static,
			depth: match self.depth {
				None => Some(0),
				Some(n) => Some(n + 1),
			},
			caller: None,
			target: None,
		}
	}

	pub fn gasometer(&self) -> &Gasometer<'config> {
		&self.gasometer
	}

	pub fn gasometer_mut(&mut self) -> &mut Gasometer<'config> {
		&mut self.gasometer
	}

	pub fn storage_meter(&self) -> &StorageMeter {
		&self.storage_meter
	}

	pub fn storage_meter_mut(&mut self) -> &mut StorageMeter {
		&mut self.storage_meter
	}

	pub fn is_static(&self) -> bool {
		self.is_static
	}

	pub fn depth(&self) -> Option<usize> {
		self.depth
	}

	pub fn caller(&self) -> &Option<H160> {
		&self.caller
	}

	pub fn caller_mut(&mut self) -> &mut Option<H160> {
		&mut self.caller
	}

	pub fn target(&self) -> &Option<H160> {
		&self.target
	}

	pub fn target_mut(&mut self) -> &mut Option<H160> {
		&mut self.target
	}
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct PrecompileOutput {
	pub exit_status: ExitSucceed,
	pub cost: u64,
	pub output: Vec<u8>,
	pub logs: Vec<Log>,
}

/// Precompiles function signature. Expected input arguments are:
///  * Address
///  * Input
///  * Context
pub type PrecompileFn = fn(H160, &[u8], Option<u64>, &Context) -> Option<Result<PrecompileOutput, ExitError>>;

/// Stack-based executor.
pub struct StackExecutor<'config, S> {
	config: &'config Config,
	precompile: PrecompileFn,
	state: S,
}

fn no_precompile(
	_address: H160,
	_input: &[u8],
	_target_gas: Option<u64>,
	_context: &Context,
) -> Option<Result<PrecompileOutput, ExitError>> {
	None
}

impl<'config, S: StackState<'config>> StackExecutor<'config, S> {
	/// Create a new stack-based executor.
	pub fn new(state: S, config: &'config Config) -> Self {
		Self::new_with_precompile(state, config, no_precompile)
	}

	/// Return a reference of the Config.
	pub fn config(&self) -> &'config Config {
		self.config
	}

	/// Create a new stack-based executor with given precompiles.
	pub fn new_with_precompile(state: S, config: &'config Config, precompile: PrecompileFn) -> Self {
		Self {
			config,
			precompile,
			state,
		}
	}

	pub fn state(&self) -> &S {
		&self.state
	}

	pub fn state_mut(&mut self) -> &mut S {
		&mut self.state
	}

	pub fn into_state(self) -> S {
		self.state
	}

	/// Create a substate executor from the current executor.
	pub fn enter_substate(&mut self, gas_limit: u64, is_static: bool) {
		self.state.enter(gas_limit, is_static);
	}

	/// Exit a substate. Panic if it results an empty substate stack.
	pub fn exit_substate(&mut self, kind: StackExitKind) -> Result<(), ExitError> {
		match kind {
			StackExitKind::Succeeded => self.state.exit_commit(),
			StackExitKind::Reverted => self.state.exit_revert(),
			StackExitKind::Failed => self.state.exit_discard(),
		}
	}

	/// Execute the runtime until it returns.
	pub fn execute(&mut self, runtime: &mut Runtime) -> ExitReason {
		match runtime.run(self) {
			Capture::Exit(s) => s,
			Capture::Trap(_) => unreachable!("Trap is Infallible"),
		}
	}

	/// Get remaining gas.
	pub fn gas(&self) -> u64 {
		self.state.metadata().gasometer().gas()
	}

	/// Execute a `CREATE` transaction.
	pub fn transact_create(
		&mut self,
		caller: H160,
		value: U256,
		init_code: Vec<u8>,
		gas_limit: u64,
		access_list: Vec<(H160, Vec<H256>)>, // See EIP-2930
	) -> ExitReason {
		let transaction_cost = gasometer::create_transaction_cost(&init_code, &access_list);
		match self
			.state
			.metadata_mut()
			.gasometer_mut()
			.record_transaction(transaction_cost)
		{
			Ok(()) => (),
			Err(e) => return e.into(),
		}

		match self.create_inner(
			caller,
			CreateScheme::Legacy { caller },
			value,
			init_code,
			Some(gas_limit),
			false,
		) {
			Capture::Exit((s, _, _)) => s,
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Execute a `CREATE2` transaction.
	pub fn transact_create2(
		&mut self,
		caller: H160,
		value: U256,
		init_code: Vec<u8>,
		salt: H256,
		gas_limit: u64,
		access_list: Vec<(H160, Vec<H256>)>, // See EIP-2930
	) -> ExitReason {
		let transaction_cost = gasometer::create_transaction_cost(&init_code, &access_list);
		match self
			.state
			.metadata_mut()
			.gasometer_mut()
			.record_transaction(transaction_cost)
		{
			Ok(()) => (),
			Err(e) => return e.into(),
		}
		let code_hash = H256::from_slice(Keccak256::digest(&init_code).as_slice());

		match self.create_inner(
			caller,
			CreateScheme::Create2 {
				caller,
				code_hash,
				salt,
			},
			value,
			init_code,
			Some(gas_limit),
			false,
		) {
			Capture::Exit((s, _, _)) => s,
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Execute a `CREATE` transaction with specific address.
	pub fn transact_create_at_address(
		&mut self,
		caller: H160,
		address: H160,
		value: U256,
		init_code: Vec<u8>,
		gas_limit: u64,
		access_list: Vec<(H160, Vec<H256>)>, // See EIP-2930
	) -> ExitReason {
		let transaction_cost = gasometer::create_transaction_cost(&init_code, &access_list);
		match self
			.state
			.metadata_mut()
			.gasometer_mut()
			.record_transaction(transaction_cost)
		{
			Ok(()) => (),
			Err(e) => return e.into(),
		}

		match self.create_inner(
			caller,
			CreateScheme::Fixed(address),
			value,
			init_code,
			Some(gas_limit),
			false,
		) {
			Capture::Exit((s, _, _)) => s,
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Execute a `CALL` transaction.
	pub fn transact_call(
		&mut self,
		caller: H160,
		address: H160,
		value: U256,
		data: Vec<u8>,
		gas_limit: u64,
		access_list: Vec<(H160, Vec<H256>)>, // See EIP-2930
	) -> (ExitReason, Vec<u8>) {
		let transaction_cost = gasometer::call_transaction_cost(&data, &access_list);
		match self
			.state
			.metadata_mut()
			.gasometer_mut()
			.record_transaction(transaction_cost)
		{
			Ok(()) => (),
			Err(e) => return (e.into(), Vec::new()),
		}

		self.state.inc_nonce(caller);

		let context = Context {
			caller,
			address,
			apparent_value: value,
		};

		match self.call_inner(
			address,
			Some(Transfer {
				source: caller,
				target: address,
				value,
			}),
			data,
			Some(gas_limit),
			false,
			false,
			false,
			context,
		) {
			Capture::Exit((s, v)) => (s, v),
			Capture::Trap(_) => unreachable!(),
		}
	}

	/// Get used gas for the current executor, given the price.
	pub fn used_gas(&self) -> u64 {
		self.state.metadata().gasometer().total_used_gas()
			- min(
				self.state.metadata().gasometer().total_used_gas() / 2,
				self.state.metadata().gasometer().refunded_gas() as u64,
			)
	}

	/// Get fee needed for the current executor, given the price.
	pub fn fee(&self, price: U256) -> U256 {
		let used_gas = self.used_gas();
		U256::from(used_gas) * price
	}

	/// Get account nonce.
	pub fn nonce(&self, address: H160) -> U256 {
		self.state.basic(address).nonce
	}

	pub fn handle_mirrored_token(&self, address: H160) -> H160 {
		log::debug!(
			target: "evm",
			"handle_mirrored_token: address: {:?}",
			address,
		);

		let addr = address.as_bytes();
		if !addr.starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX) {
			return address;
		}

		if addr.starts_with(&H160_PREFIX_TOKEN) || addr.starts_with(&H160_PREFIX_DEXSHARE) {
			// `Token` predeploy contract.
			let token_address = H160::from_low_u64_be(PREDEPLOY_ADDRESS_START);
			log::debug!(
				target: "evm",
				"handle_mirrored_token: origin address: {:?}, token address: {:?}",
				address,
				token_address
			);
			token_address
		} else {
			address
		}
	}

	/// Get the create address from given scheme.
	pub fn create_address(&self, scheme: CreateScheme) -> Result<H160, ExitError> {
		let address = match scheme {
			CreateScheme::Create2 {
				caller,
				code_hash,
				salt,
			} => {
				let mut hasher = Keccak256::new();
				hasher.update(&[0xff]);
				hasher.update(&caller[..]);
				hasher.update(&salt[..]);
				hasher.update(&code_hash[..]);
				H256::from_slice(hasher.finalize().as_slice()).into()
			}
			CreateScheme::Legacy { caller } => {
				let nonce = self.nonce(caller);
				let mut stream = rlp::RlpStream::new_list(2);
				stream.append(&caller);
				stream.append(&nonce);
				H256::from_slice(Keccak256::digest(&stream.out()).as_slice()).into()
			}
			CreateScheme::Fixed(naddress) => naddress,
		};

		match scheme {
			CreateScheme::Create2 { .. } | CreateScheme::Legacy { .. } => {
				if address.as_bytes().starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX) {
					Err(ExitError::Other("ConflictContractAddress".into()))
				} else {
					Ok(address)
				}
			}
			_ => Ok(address),
		}
	}

	fn create_inner(
		&mut self,
		caller: H160,
		scheme: CreateScheme,
		value: U256,
		init_code: Vec<u8>,
		target_gas: Option<u64>,
		take_l64: bool,
	) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Infallible> {
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit((e.into(), None, Vec::new())),
				}
			};
		}

		fn l64(gas: u64) -> u64 {
			gas - gas / 64
		}

		let address = match self.create_address(scheme) {
			Err(e) => {
				return Capture::Exit((ExitReason::Error(e), None, Vec::new()));
			}
			Ok(address) => address,
		};

		*self.state.metadata_mut().caller_mut() = Some(caller);
		*self.state.metadata_mut().target_mut() = Some(address);

		event!(Create {
			caller,
			address,
			scheme,
			value,
			init_code: &init_code,
			target_gas
		});

		if let Some(depth) = self.state.metadata().depth() {
			if depth > self.config.call_stack_limit {
				return Capture::Exit((ExitError::CallTooDeep.into(), None, Vec::new()));
			}
		}

		if self.balance(caller) < value {
			return Capture::Exit((ExitError::OutOfFund.into(), None, Vec::new()));
		}

		let after_gas = if take_l64 && self.config.call_l64_after_gas {
			if self.config.estimate {
				let initial_after_gas = self.state.metadata().gasometer().gas();
				let diff = initial_after_gas - l64(initial_after_gas);
				try_or_fail!(self.state.metadata_mut().gasometer_mut().record_cost(diff));
				self.state.metadata().gasometer().gas()
			} else {
				l64(self.state.metadata().gasometer().gas())
			}
		} else {
			self.state.metadata().gasometer().gas()
		};

		let target_gas = target_gas.unwrap_or(after_gas);

		let gas_limit = min(after_gas, target_gas);
		try_or_fail!(self.state.metadata_mut().gasometer_mut().record_cost(gas_limit));

		self.state.inc_nonce(caller);

		self.enter_substate(gas_limit, false);

		{
			if self.code_size(address) != U256::zero() {
				let _ = self.exit_substate(StackExitKind::Failed);
				return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()));
			}

			if self.nonce(address) > U256::zero() {
				let _ = self.exit_substate(StackExitKind::Failed);
				return Capture::Exit((ExitError::CreateCollision.into(), None, Vec::new()));
			}

			self.state.reset_storage(address);
		}

		let context = Context {
			address,
			caller,
			apparent_value: value,
		};
		let transfer = Transfer {
			source: caller,
			target: address,
			value,
		};
		match self.state.transfer(transfer) {
			Ok(()) => (),
			Err(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				return Capture::Exit((ExitReason::Error(e), None, Vec::new()));
			}
		}

		if self.config.create_increase_nonce {
			self.state.inc_nonce(address);
		}

		let mut runtime = Runtime::new(Rc::new(init_code), Rc::new(Vec::new()), context, self.config);

		let reason = self.execute(&mut runtime);
		log::debug!(target: "evm", "Create execution using address {}: {:?}", address, reason);

		match reason {
			ExitReason::Succeed(s) => {
				let out = runtime.machine().return_value();

				if let Some(limit) = self.config.create_contract_limit {
					if out.len() > limit {
						self.state.metadata_mut().gasometer_mut().fail();
						let _ = self.exit_substate(StackExitKind::Failed);
						return Capture::Exit((ExitError::CreateContractLimit.into(), None, Vec::new()));
					}
				}

				match self.state.metadata_mut().gasometer_mut().record_deposit(out.len()) {
					Ok(()) => {
						self.state
							.metadata_mut()
							.storage_meter_mut()
							.charge_with_extra_bytes(out.len() as u32);
						let e = self.exit_substate(StackExitKind::Succeeded);
						try_or_fail!(e);
						self.state.set_code(address, out);
						Capture::Exit((ExitReason::Succeed(s), Some(address), Vec::new()))
					}
					Err(e) => {
						let _ = self.exit_substate(StackExitKind::Failed);
						Capture::Exit((ExitReason::Error(e), None, Vec::new()))
					}
				}
			}
			ExitReason::Error(e) => {
				self.state.metadata_mut().gasometer_mut().fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Error(e), None, Vec::new()))
			}
			ExitReason::Revert(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				Capture::Exit((ExitReason::Revert(e), None, runtime.machine().return_value()))
			}
			ExitReason::Fatal(e) => {
				self.state.metadata_mut().gasometer_mut().fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Fatal(e), None, Vec::new()))
			}
		}
	}

	#[allow(clippy::too_many_arguments)]
	fn call_inner(
		&mut self,
		code_address: H160,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<u64>,
		is_static: bool,
		take_l64: bool,
		take_stipend: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Infallible> {
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit((e.into(), Vec::new())),
				}
			};
		}

		fn l64(gas: u64) -> u64 {
			gas - gas / 64
		}

		*self.state.metadata_mut().target_mut() = Some(code_address);

		event!(Call {
			code_address,
			transfer: &transfer,
			input: &input,
			target_gas,
			is_static,
			context: &context,
		});

		let after_gas = if take_l64 && self.config.call_l64_after_gas {
			if self.config.estimate {
				let initial_after_gas = self.state.metadata().gasometer().gas();
				let diff = initial_after_gas - l64(initial_after_gas);
				try_or_fail!(self.state.metadata_mut().gasometer_mut().record_cost(diff));
				self.state.metadata().gasometer().gas()
			} else {
				l64(self.state.metadata().gasometer().gas())
			}
		} else {
			self.state.metadata().gasometer().gas()
		};

		let target_gas = target_gas.unwrap_or(after_gas);
		let mut gas_limit = min(target_gas, after_gas);

		try_or_fail!(self.state.metadata_mut().gasometer_mut().record_cost(gas_limit));

		if let Some(transfer) = transfer.as_ref() {
			if take_stipend && transfer.value != U256::zero() {
				gas_limit = gas_limit.saturating_add(self.config.call_stipend);
			}
		}

		let code = self.code(code_address);

		self.enter_substate(gas_limit, is_static);
		self.state.touch(context.address);

		if let Some(depth) = self.state.metadata().depth() {
			if depth > self.config.call_stack_limit {
				let _ = self.exit_substate(StackExitKind::Reverted);
				return Capture::Exit((ExitError::CallTooDeep.into(), Vec::new()));
			}
		}

		if let Some(transfer) = transfer {
			match self.state.transfer(transfer) {
				Ok(()) => (),
				Err(e) => {
					let _ = self.exit_substate(StackExitKind::Reverted);
					return Capture::Exit((ExitReason::Error(e), Vec::new()));
				}
			}
		}

		if let Some(ret) = (self.precompile)(code_address, &input, Some(gas_limit), &context) {
			match ret {
				Ok(PrecompileOutput {
					exit_status,
					output,
					cost,
					logs,
				}) => {
					for Log { address, topics, data } in logs {
						match self.log(address, topics, data) {
							Ok(_) => continue,
							Err(error) => {
								return Capture::Exit((ExitReason::Error(error), output));
							}
						}
					}

					let _ = self.state.metadata_mut().gasometer_mut().record_cost(cost);
					let e = self.exit_substate(StackExitKind::Succeeded);
					try_or_fail!(e);
					return Capture::Exit((ExitReason::Succeed(exit_status), output));
				}
				Err(e) => {
					// return the error to contract
					let _ = self.exit_substate(StackExitKind::Reverted);
					return Capture::Exit((ExitReason::Revert(ExitRevert::Reverted), encode_revert_message(&e)));
				}
			}
		}

		let mut runtime = Runtime::new(Rc::new(code), Rc::new(input), context, self.config);

		#[cfg(not(feature = "tracing"))]
		let reason = self.execute(&mut runtime);
		#[cfg(feature = "tracing")]
		let reason = module_evm_utiltity::evm_runtime::tracing::using(&mut Tracer, || self.execute(&mut runtime));

		log::debug!(target: "evm", "Call execution using address {}: {:?}", code_address, reason);

		match reason {
			ExitReason::Succeed(s) => {
				let e = self.exit_substate(StackExitKind::Succeeded);
				try_or_fail!(e);
				Capture::Exit((ExitReason::Succeed(s), runtime.machine().return_value()))
			}
			ExitReason::Error(e) => {
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Error(e), Vec::new()))
			}
			ExitReason::Revert(e) => {
				let _ = self.exit_substate(StackExitKind::Reverted);
				Capture::Exit((ExitReason::Revert(e), runtime.machine().return_value()))
			}
			ExitReason::Fatal(e) => {
				self.state.metadata_mut().gasometer_mut().fail();
				let _ = self.exit_substate(StackExitKind::Failed);
				Capture::Exit((ExitReason::Fatal(e), Vec::new()))
			}
		}
	}
}

impl<'config, S: StackState<'config>> Handler for StackExecutor<'config, S> {
	type CreateInterrupt = Infallible;
	type CreateFeedback = Infallible;
	type CallInterrupt = Infallible;
	type CallFeedback = Infallible;

	fn balance(&self, address: H160) -> U256 {
		self.state.basic(address).balance
	}

	fn code_size(&self, address: H160) -> U256 {
		let addr = self.handle_mirrored_token(address);
		U256::from(self.state.code(addr).len())
	}

	fn code_hash(&self, address: H160) -> H256 {
		if !self.exists(address) {
			return H256::default();
		}

		let addr = self.handle_mirrored_token(address);
		H256::from_slice(Keccak256::digest(&self.state.code(addr)).as_slice())
	}

	fn code(&self, address: H160) -> Vec<u8> {
		let addr = self.handle_mirrored_token(address);
		self.state.code(addr)
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		self.state.storage(address, index)
	}

	fn original_storage(&self, address: H160, index: H256) -> H256 {
		self.state.original_storage(address, index).unwrap_or_default()
	}

	fn exists(&self, address: H160) -> bool {
		if self.config.empty_considered_exists {
			self.state.exists(address)
		} else {
			self.state.exists(address) && !self.state.is_empty(address)
		}
	}

	fn is_cold(&self, address: H160, maybe_index: Option<H256>) -> bool {
		match maybe_index {
			None => self.state.is_cold(address),
			Some(index) => self.state.is_storage_cold(address, index),
		}
	}

	fn gas_left(&self) -> U256 {
		U256::from(self.state.metadata().gasometer().gas())
	}

	fn gas_price(&self) -> U256 {
		self.state.gas_price()
	}
	fn origin(&self) -> H160 {
		self.state.origin()
	}
	fn block_hash(&self, number: U256) -> H256 {
		self.state.block_hash(number)
	}
	fn block_number(&self) -> U256 {
		self.state.block_number()
	}
	fn block_coinbase(&self) -> H160 {
		self.state.block_coinbase()
	}
	fn block_timestamp(&self) -> U256 {
		self.state.block_timestamp()
	}
	fn block_difficulty(&self) -> U256 {
		self.state.block_difficulty()
	}
	fn block_gas_limit(&self) -> U256 {
		self.state.block_gas_limit()
	}
	fn chain_id(&self) -> U256 {
		self.state.chain_id()
	}

	fn deleted(&self, address: H160) -> bool {
		self.state.deleted(address)
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) -> Result<(), ExitError> {
		self.state.set_storage(address, index, value);
		Ok(())
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
		self.state.log(address, topics, data);
		Ok(())
	}

	fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
		let balance = self.balance(address);

		event!(Suicide {
			target,
			address,
			balance,
		});

		self.state.transfer(Transfer {
			source: address,
			target,
			value: balance,
		})?;
		self.state.reset_balance(address);
		self.state.set_deleted(address);

		Ok(())
	}

	fn create(
		&mut self,
		caller: H160,
		scheme: CreateScheme,
		value: U256,
		init_code: Vec<u8>,
		target_gas: Option<u64>,
	) -> Capture<(ExitReason, Option<H160>, Vec<u8>), Self::CreateInterrupt> {
		self.create_inner(caller, scheme, value, init_code, target_gas, true)
	}

	fn call(
		&mut self,
		code_address: H160,
		transfer: Option<Transfer>,
		input: Vec<u8>,
		target_gas: Option<u64>,
		is_static: bool,
		context: Context,
	) -> Capture<(ExitReason, Vec<u8>), Self::CallInterrupt> {
		self.call_inner(
			code_address,
			transfer,
			input,
			target_gas,
			is_static,
			true,
			true,
			context,
		)
	}

	#[inline]
	fn pre_validate(&mut self, context: &Context, opcode: Opcode, stack: &Stack) -> Result<(), ExitError> {
		// log::trace!(target: "evm", "Running opcode: {:?}, Pre gas-left: {:?}", opcode,
		// gasometer().gas());

		if let Some(cost) = gasometer::static_opcode_cost(opcode) {
			self.state.metadata_mut().gasometer_mut().record_cost(cost)?;
		} else {
			let is_static = self.state.metadata().is_static();
			// TODO: EIP-2930
			let (gas_cost, _storage_target, memory_cost) =
				gasometer::dynamic_opcode_cost(context.address, opcode, stack, is_static, self.config, self)?;

			let gasometer = &mut self.state.metadata_mut().gasometer_mut();

			gasometer.record_dynamic_cost(gas_cost, memory_cost)?;
		}

		Ok(())
	}
}
