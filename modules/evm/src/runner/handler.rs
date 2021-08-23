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

#![allow(clippy::type_complexity)]

use crate::{
	precompiles::Precompiles,
	runner::storage_meter::{StorageMeter, StorageMeterHandler},
	AccountInfo, AccountStorages, Accounts, AddressMapping, Codes, Config, ContractInfo, Error, Event, Log, Pallet,
	Vicinity, RESERVE_ID_DEVELOPER_DEPOSIT, RESERVE_ID_STORAGE_DEPOSIT,
};
use evm::{Capture, Context, CreateScheme, ExitError, ExitReason, Opcode, Runtime, Stack, Transfer};
use evm_gasometer::{self as gasometer, Gasometer};
use evm_runtime::{Config as EvmRuntimeConfig, Handler as HandlerT};
use frame_support::{
	log,
	traits::{BalanceStatus, Currency, ExistenceRequirement, Get, NamedReservableCurrency},
};
use primitive_types::{H160, H256, U256};
use primitives::{H160_PREFIX_DEXSHARE, H160_PREFIX_TOKEN, PREDEPLOY_ADDRESS_START, SYSTEM_CONTRACT_ADDRESS_PREFIX};
use sha3::{Digest, Keccak256};
use sp_runtime::{
	traits::{One, Saturating, UniqueSaturatedInto, Zero},
	DispatchError, DispatchResult, SaturatedConversion, TransactionOutcome,
};
use sp_std::{cmp::min, convert::Infallible, marker::PhantomData, prelude::*, rc::Rc};

/// Storage key size and storage value size.
pub const STORAGE_SIZE: u32 = 64;

pub struct Handler<'vicinity, 'config, 'meter, T: Config> {
	pub vicinity: &'vicinity Vicinity,
	pub config: &'config EvmRuntimeConfig,
	pub gasometer: Gasometer<'config>,
	pub storage_meter: StorageMeter<'meter>,
	pub is_static: bool,
	_marker: PhantomData<T>,
}

fn l64(gas: u64) -> u64 {
	gas - gas / 64
}

impl<'vicinity, 'config, 'meter, T: Config> Handler<'vicinity, 'config, 'meter, T> {
	pub fn new(
		vicinity: &'vicinity Vicinity,
		gas_limit: u64,
		storage_meter: StorageMeter<'meter>,
		is_static: bool,
		config: &'config EvmRuntimeConfig,
	) -> Self {
		Handler::<'vicinity, 'config, '_, T> {
			vicinity,
			config,
			is_static,
			gasometer: Gasometer::new(gas_limit, config),
			storage_meter,
			_marker: PhantomData,
		}
	}

	pub fn run_transaction<R, F: FnOnce(&mut Handler<'vicinity, 'config, '_, T>) -> TransactionOutcome<R>>(
		vicinity: &'vicinity Vicinity,
		gas_limit: u64,
		storage_limit: u32,
		contract: H160,
		is_static: bool,
		config: &'config EvmRuntimeConfig,
		f: F,
	) -> Result<R, DispatchError> {
		frame_support::storage::with_transaction(|| {
			let mut storage_meter_handler = StorageMeterHandlerImpl::<T>::new(vicinity.origin);
			let storage_meter = match StorageMeter::new(&mut storage_meter_handler, contract, storage_limit) {
				Ok(x) => x,
				Err(e) => return TransactionOutcome::Rollback(Err(e)),
			};

			let mut substate = Handler::new(vicinity, gas_limit, storage_meter, is_static, config);

			match f(&mut substate) {
				TransactionOutcome::Commit(r) => match substate.storage_meter.finish() {
					Ok(_) => TransactionOutcome::Commit(Ok(r)),
					Err(e) => TransactionOutcome::Rollback(Err(e)),
				},
				TransactionOutcome::Rollback(e) => TransactionOutcome::Rollback(Ok(e)),
			}
		})
	}

	pub fn run_sub_transaction<
		'a,
		R,
		F: FnOnce(&mut Handler<'vicinity, 'config, '_, T>, &mut Gasometer<'_>) -> TransactionOutcome<R>,
	>(
		&'a mut self,
		vicinity: &'vicinity Vicinity,
		gas_limit: u64,
		contract: H160,
		is_static: bool,
		config: &'config EvmRuntimeConfig,
		f: F,
	) -> Result<R, DispatchError> {
		frame_support::storage::with_transaction(|| {
			let storage_meter = match self.storage_meter.child_meter(contract) {
				Ok(x) => x,
				Err(e) => return TransactionOutcome::Rollback(Err(e)),
			};

			let mut substate = Handler::new(vicinity, gas_limit, storage_meter, is_static, config);

			match f(&mut substate, &mut self.gasometer) {
				TransactionOutcome::Commit(r) => match substate.storage_meter.finish() {
					Ok(_) => TransactionOutcome::Commit(Ok(r)),
					Err(e) => TransactionOutcome::Rollback(Err(e)),
				},
				TransactionOutcome::Rollback(e) => TransactionOutcome::Rollback(Ok(e)),
			}
		})
	}

	/// Get used gas for the current executor, given the price.
	pub fn used_gas(&self) -> u64 {
		self.gasometer.total_used_gas()
			- min(
				self.gasometer.total_used_gas() / 2,
				self.gasometer.refunded_gas() as u64,
			)
	}

	pub fn used_storage(&self) -> i32 {
		self.storage_meter.used_storage()
	}

	pub fn execute(
		&mut self,
		caller: H160,
		address: H160,
		value: U256,
		code: Vec<u8>,
		input: Vec<u8>,
	) -> (ExitReason, Vec<u8>) {
		let context = Context {
			caller,
			address,
			apparent_value: value,
		};

		let mut runtime = Runtime::new(Rc::new(code), Rc::new(input), context, self.config);

		let reason = match runtime.run(self) {
			Capture::Exit(s) => s,
			Capture::Trap(_) => unreachable!("Trap is Infallible"),
		};

		match reason {
			ExitReason::Succeed(s) => (s.into(), runtime.machine().return_value()),
			ExitReason::Error(e) => (e.into(), Vec::new()),
			ExitReason::Revert(e) => (e.into(), runtime.machine().return_value()),
			ExitReason::Fatal(e) => {
				self.gasometer.fail();
				(e.into(), Vec::new())
			}
		}
	}

	fn transfer(transfer: Transfer) -> Result<(), ExitError> {
		let source = T::AddressMapping::get_account_id(&transfer.source);
		let target = T::AddressMapping::get_account_id(&transfer.target);

		T::Currency::transfer(
			&source,
			&target,
			transfer.value.saturated_into::<u128>().unique_saturated_into(),
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|_| ExitError::OutOfGas)
	}

	pub fn nonce(address: H160) -> U256 {
		let account = Pallet::<T>::account_basic(&address);
		account.nonce
	}

	pub fn inc_nonce(address: H160) {
		Accounts::<T>::mutate(&address, |maybe_account| {
			if let Some(account) = maybe_account.as_mut() {
				account.nonce += One::one()
			} else {
				let mut account_info = <AccountInfo<T>>::new(Default::default(), None);
				account_info.nonce += One::one();
				*maybe_account = Some(account_info);
			}
		});
	}

	pub fn create_address(scheme: CreateScheme) -> Result<H160, ExitError> {
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
				let nonce = Self::nonce(caller);
				let mut stream = rlp::RlpStream::new_list(2);
				stream.append(&caller);
				stream.append(&nonce);
				H256::from_slice(Keccak256::digest(&stream.out()).as_slice()).into()
			}
			CreateScheme::Fixed(naddress) => naddress,
		};

		if address.as_bytes().starts_with(&SYSTEM_CONTRACT_ADDRESS_PREFIX) {
			Err(ExitError::Other(
				Into::<&str>::into(Error::<T>::ConflictContractAddress).into(),
			))
		} else {
			Ok(address)
		}
	}

	pub fn can_call_contract(address: &H160, caller: &H160) -> bool {
		if let Some(AccountInfo {
			contract_info: Some(ContractInfo {
				deployed, maintainer, ..
			}),
			..
		}) = Accounts::<T>::get(address)
		{
			deployed || maintainer == *caller || Self::is_developer_or_contract(caller)
		} else {
			// contract non exist, we don't override defualt evm behaviour
			true
		}
	}

	pub fn is_developer_or_contract(caller: &H160) -> bool {
		if let Some(AccountInfo { contract_info, .. }) = Accounts::<T>::get(caller) {
			let account_id = T::AddressMapping::get_account_id(&caller);
			contract_info.is_some()
				|| !T::Currency::reserved_balance_named(&RESERVE_ID_DEVELOPER_DEPOSIT, &account_id).is_zero()
		} else {
			false
		}
	}

	fn handle_mirrored_token(address: H160) -> H160 {
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
			// Token contracts.
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
}

/// Create `try_or_fail` and `try_or_rollback`.
macro_rules! create_try {
	( $map_err:expr ) => {
		#[allow(unused_macros)]
		macro_rules! try_or_fail {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return Capture::Exit($map_err(e)),
				}
			};
		}

		macro_rules! try_or_rollback {
			( $e:expr ) => {
				match $e {
					Ok(v) => v,
					Err(e) => return TransactionOutcome::Rollback(Capture::Exit($map_err(e))),
				}
			};
		}
	};
}

impl<'vicinity, 'config, 'meter, T: Config> HandlerT for Handler<'vicinity, 'config, 'meter, T> {
	type CreateInterrupt = Infallible;
	type CreateFeedback = Infallible;
	type CallInterrupt = Infallible;
	type CallFeedback = Infallible;

	fn balance(&self, address: H160) -> U256 {
		let account = Pallet::<T>::account_basic(&address);
		account.balance
	}

	fn code_size(&self, address: H160) -> U256 {
		let addr = Self::handle_mirrored_token(address);
		let code_hash = self.code_hash(addr);
		U256::from(Codes::<T>::decode_len(&code_hash).unwrap_or(0))
	}

	fn code_hash(&self, address: H160) -> H256 {
		let addr = Self::handle_mirrored_token(address);
		Pallet::<T>::code_hash_at_address(&addr)
	}

	fn code(&self, address: H160) -> Vec<u8> {
		let addr = Self::handle_mirrored_token(address);
		Pallet::<T>::code_at_address(&addr).into_inner()
	}

	fn storage(&self, address: H160, index: H256) -> H256 {
		AccountStorages::<T>::get(address, index)
	}

	fn original_storage(&self, _address: H160, _index: H256) -> H256 {
		// We do not have the concept of original storage in the native runner, so we
		// always return empty value. This only affects gas calculation in the current
		// EVM specification.
		H256::default()
	}

	fn gas_left(&self) -> U256 {
		U256::from(self.gasometer.gas())
	}

	fn gas_price(&self) -> U256 {
		self.vicinity.gas_price
	}

	fn origin(&self) -> H160 {
		self.vicinity.origin
	}

	fn block_hash(&self, number: U256) -> H256 {
		if number > U256::from(u32::max_value()) {
			H256::default()
		} else {
			let number = T::BlockNumber::from(number.as_u32());
			H256::from_slice(frame_system::Pallet::<T>::block_hash(number).as_ref())
		}
	}

	fn block_number(&self) -> U256 {
		let number: u128 = frame_system::Pallet::<T>::block_number().unique_saturated_into();
		U256::from(number)
	}

	fn block_coinbase(&self) -> H160 {
		H160::default()
	}

	fn block_timestamp(&self) -> U256 {
		let now: u128 = pallet_timestamp::Pallet::<T>::get().unique_saturated_into();
		U256::from(now / 1000)
	}

	fn block_difficulty(&self) -> U256 {
		U256::zero()
	}

	fn block_gas_limit(&self) -> U256 {
		U256::zero()
	}

	fn chain_id(&self) -> U256 {
		U256::from(T::ChainId::get())
	}

	fn exists(&self, _address: H160) -> bool {
		true
	}

	fn deleted(&self, _address: H160) -> bool {
		// This only affects gas calculation in the current EVM specification.
		// return true to disable suicide gas refund
		true
	}

	fn set_storage(&mut self, address: H160, index: H256, value: H256) -> Result<(), ExitError> {
		if self.is_static {
			return Err(ExitError::OutOfGas);
		}
		enum StorageChange {
			None,
			Added,
			Removed,
		}

		let mut storage_change = StorageChange::None;

		let default_value = H256::default();
		let is_prev_value_default = Pallet::<T>::account_storages(address, index) == default_value;

		if value == default_value {
			if !is_prev_value_default {
				storage_change = StorageChange::Removed;
			}

			AccountStorages::<T>::remove(address, index);
		} else {
			if is_prev_value_default {
				storage_change = StorageChange::Added;
			}

			AccountStorages::<T>::insert(address, index, value);
		}

		match storage_change {
			StorageChange::Added => {
				Pallet::<T>::update_contract_storage_size(&address, STORAGE_SIZE as i32);
				self.storage_meter.charge(STORAGE_SIZE)
			}
			StorageChange::Removed => {
				Pallet::<T>::update_contract_storage_size(&address, -(STORAGE_SIZE as i32));
				self.storage_meter.refund(STORAGE_SIZE)
			}
			StorageChange::None => Ok(()),
		}
		.map_err(|_| ExitError::OutOfGas)
	}

	fn log(&mut self, address: H160, topics: Vec<H256>, data: Vec<u8>) -> Result<(), ExitError> {
		Pallet::<T>::deposit_event(Event::<T>::Log(Log { address, topics, data }));

		Ok(())
	}

	fn mark_delete(&mut self, address: H160, target: H160) -> Result<(), ExitError> {
		if self.is_static {
			return Err(ExitError::OutOfGas);
		}

		let storage = Pallet::<T>::remove_contract(&address, &target)
			.map_err(|e| ExitError::Other(Into::<&str>::into(e).into()))?;

		self.storage_meter
			.refund(storage)
			.map_err(|e| ExitError::Other(Into::<&str>::into(e).into()))?;

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
		log::debug!(
			target: "evm",
			"handler: create: caller {:?}",
			caller,
		);

		create_try!(|e: ExitError| (e.into(), None, Vec::new()));

		if self.is_static {
			return Capture::Exit((ExitError::OutOfGas.into(), None, Vec::new()));
		}

		let mut after_gas = self.gasometer.gas();
		if self.config.call_l64_after_gas {
			after_gas = l64(after_gas);
		}
		let mut target_gas = target_gas.unwrap_or(after_gas);
		target_gas = min(target_gas, after_gas);
		try_or_fail!(self.gasometer.record_cost(target_gas));

		let maybe_address = Self::create_address(scheme);
		let address = if let Err(e) = maybe_address {
			return Capture::Exit((ExitReason::Error(e), None, Vec::new()));
		} else {
			maybe_address.unwrap()
		};
		Self::inc_nonce(caller);

		let origin = &self.vicinity.origin;

		self.run_sub_transaction(
			self.vicinity,
			target_gas,
			address,
			self.is_static,
			self.config,
			|substate, gasometer| {
				try_or_rollback!(Self::transfer(Transfer {
					source: caller,
					target: address,
					value,
				}));

				let (reason, out) = substate.execute(caller, address, value, init_code, Vec::new());

				match reason {
					ExitReason::Succeed(s) => match substate.gasometer.record_deposit(out.len()) {
						Ok(()) => {
							try_or_rollback!(gasometer.record_stipend(substate.gasometer.gas()));
							try_or_rollback!(gasometer.record_refund(substate.gasometer.refunded_gas()));

							Handler::<T>::inc_nonce(address);
							try_or_rollback!(substate
								.storage_meter
								.charge((out.len() as u32).saturating_add(T::NewContractExtraBytes::get()))
								.map_err(|_| ExitError::OutOfGas));
							match <Pallet<T>>::on_contract_initialization(&address, origin, out) {
								Ok(()) => {
									TransactionOutcome::Commit(Capture::Exit((s.into(), Some(address), Vec::new())))
								}
								Err(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new()))),
							}
						}
						Err(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new()))),
					},
					ExitReason::Revert(r) => TransactionOutcome::Rollback(Capture::Exit((r.into(), None, out))),
					ExitReason::Error(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new()))),
					ExitReason::Fatal(e) => {
						gasometer.fail();
						TransactionOutcome::Rollback(Capture::Exit((e.into(), None, Vec::new())))
					}
				}
			},
		)
		.unwrap_or_else(|x| {
			Capture::Exit((
				ExitReason::Error(ExitError::Other(Into::<&'static str>::into(x).into())),
				None,
				Vec::new(),
			))
		})
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
		log::debug!(
			target: "evm",
			"handler: call: source {:?} code_address {:?} input: {:?} target_gas {:?} gas_left {:?}",
			context.caller,
			code_address,
			input,
			target_gas,
			self.gas_left()
		);

		create_try!(|e: ExitError| (e.into(), Vec::new()));

		if self.is_static && transfer.is_some() {
			return Capture::Exit((ExitError::OutOfGas.into(), Vec::new()));
		}

		let mut after_gas = self.gasometer.gas();
		if self.config.call_l64_after_gas {
			after_gas = l64(after_gas);
		}
		let mut target_gas = target_gas.unwrap_or(after_gas);
		target_gas = min(target_gas, after_gas);

		if let Some(transfer) = transfer.as_ref() {
			if !transfer.value.is_zero() {
				target_gas = target_gas.saturating_add(self.config.call_stipend);
			}
		}

		let code = self.code(code_address);

		self.run_sub_transaction(
			self.vicinity,
			target_gas,
			context.address,
			self.is_static || is_static,
			self.config,
			|substate, gasometer| {
				if let Some(transfer) = transfer {
					try_or_rollback!(Self::transfer(transfer));
				}

				try_or_rollback!(gasometer.record_cost(target_gas));

				if let Some(ret) = T::Precompiles::execute(code_address, &input, Some(target_gas), &context) {
					log::debug!(
						target: "evm",
						"handler: call-result: precompile result {:?}",
						ret
					);

					return match ret {
						Ok((s, out, cost)) => {
							// TODO: write some test to make sure following 3 lines is correct
							try_or_rollback!(substate.gasometer.record_cost(cost));
							try_or_rollback!(gasometer.record_stipend(substate.gasometer.gas()));
							try_or_rollback!(gasometer.record_refund(substate.gasometer.refunded_gas()));
							// precompile contract cost 0
							// try_or_rollback!(self.storage_meter.record_cost(0));
							TransactionOutcome::Commit(Capture::Exit((s.into(), out)))
						}
						Err(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new()))),
					};
				}

				let (reason, out) = substate.execute(
					context.caller,
					context.address,
					context.apparent_value,
					code.clone(),
					input,
				);

				log::debug!(
					target: "evm",
					"handler: call-result: reason {:?} out {:?} gas_left {:?}",
					reason, out, substate.gas_left()
				);

				match reason {
					ExitReason::Succeed(s) => {
						try_or_rollback!(gasometer.record_stipend(substate.gasometer.gas()));
						try_or_rollback!(gasometer.record_refund(substate.gasometer.refunded_gas()));
						TransactionOutcome::Commit(Capture::Exit((s.into(), out)))
					}
					ExitReason::Revert(r) => TransactionOutcome::Rollback(Capture::Exit((r.into(), out))),
					ExitReason::Error(e) => TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new()))),
					ExitReason::Fatal(e) => {
						gasometer.fail();
						TransactionOutcome::Rollback(Capture::Exit((e.into(), Vec::new())))
					}
				}
			},
		)
		.unwrap_or_else(|x| {
			Capture::Exit((
				ExitReason::Error(ExitError::Other(Into::<&'static str>::into(x).into())),
				Vec::new(),
			))
		})
	}

	fn pre_validate(&mut self, context: &Context, opcode: Opcode, stack: &Stack) -> Result<(), ExitError> {
		if let Some(cost) = gasometer::static_opcode_cost(opcode) {
			self.gasometer.record_cost(cost)?;
		} else {
			let (gas_cost, memory_cost) =
				gasometer::dynamic_opcode_cost(context.address, opcode, stack, self.is_static, &self.config, self)?;

			self.gasometer.record_dynamic_cost(gas_cost, memory_cost)?;
		}
		Ok(())
	}
}

pub struct StorageMeterHandlerImpl<T: Config> {
	origin: H160,
	_marker: PhantomData<T>,
}

impl<T: Config> StorageMeterHandlerImpl<T> {
	pub fn new(origin: H160) -> Self {
		Self {
			origin,
			_marker: Default::default(),
		}
	}
}

impl<T: Config> StorageMeterHandler for StorageMeterHandlerImpl<T> {
	fn reserve_storage(&mut self, limit: u32) -> DispatchResult {
		if limit.is_zero() {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"reserve_storage: from {:?} limit {:?}",
			self.origin, limit,
		);

		let user = T::AddressMapping::get_account_id(&self.origin);

		let amount = T::StorageDepositPerByte::get().saturating_mul(limit.into());

		T::Currency::reserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount)
	}

	fn unreserve_storage(&mut self, limit: u32, used: u32, refunded: u32) -> DispatchResult {
		let total = limit.saturating_add(refunded);
		let unused = total.saturating_sub(used);
		if unused.is_zero() {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"unreserve_storage: from {:?} used {:?} refunded {:?} unused {:?}",
			self.origin, used, refunded, unused
		);

		let user = T::AddressMapping::get_account_id(&self.origin);
		let amount = T::StorageDepositPerByte::get().saturating_mul(unused.into());

		// should always be able to unreserve the amount
		// but otherwise we will just ignore the issue here.
		let err_amount = T::Currency::unreserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount);
		debug_assert!(err_amount.is_zero());
		Ok(())
	}

	fn charge_storage(&mut self, contract: &H160, used: u32, refunded: u32) -> DispatchResult {
		if used == refunded {
			return Ok(());
		}

		log::debug!(
			target: "evm",
			"charge_storage: from {:?} contract {:?} used {:?} refunded {:?}",
			&self.origin, contract, used, refunded
		);

		let user = T::AddressMapping::get_account_id(&self.origin);
		let contract_acc = T::AddressMapping::get_account_id(contract);

		if used > refunded {
			let storage = used - refunded;
			let amount = T::StorageDepositPerByte::get().saturating_mul(storage.into());

			// `repatriate_reserved` requires beneficiary is an existing account but
			// contract_acc could be a new account so we need to do
			// unreserve/transfer/reserve.
			// should always be able to unreserve the amount
			// but otherwise we will just ignore the issue here.
			let err_amount = T::Currency::unreserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &user, amount);
			debug_assert!(err_amount.is_zero());
			T::Currency::transfer(&user, &contract_acc, amount, ExistenceRequirement::AllowDeath)?;
			T::Currency::reserve_named(&RESERVE_ID_STORAGE_DEPOSIT, &contract_acc, amount)?;
		} else {
			let storage = refunded - used;
			let amount = T::StorageDepositPerByte::get().saturating_mul(storage.into());

			// user can't be a dead account
			let val = T::Currency::repatriate_reserved_named(
				&RESERVE_ID_STORAGE_DEPOSIT,
				&contract_acc,
				&user,
				amount,
				BalanceStatus::Reserved,
			)?;
			debug_assert!(val.is_zero());
		};

		Ok(())
	}

	fn out_of_storage_error(&self) -> DispatchError {
		Error::<T>::OutOfStorage.into()
	}
}
